// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Compose-card flow: turn a free-text description into a fully-formed
//! `KanbanCardCreate` payload by asking the Kanban-kind agent to fill it.
//!
//! Runs the Kanban agent through `tool_loop::execute_with_tools` with a pair
//! of privately-instantiated tools injected via `extra_tools`:
//! - `ListAgentsTool` so the model can discover the `target_agent_id`
//! - `SubmitComposedCardTool` which captures the final payload into an
//!   `Arc<Mutex<_>>` slot owned by this module.
//!
//! The agent's own `tools`, `mcp_servers` and `skills` are honoured (it can
//! call PromptManager, Skills, etc. — those writes are governed by
//! `manager_write_gate`). The flow is now ASYNC and detached:
//! `start_compose_card` reserves a global slot (cap `MAX_CONCURRENT_COMPOSE`) and
//! spawns a task that runs the tool-loop, persists the result as a `proposed`
//! card (via `create_kanban_card_core`, M-2) and emits `kanban:compose_ready` /
//! `kanban:compose_failed`. The user then validates (`approve_proposed_card` →
//! `ready`) or rejects (`delete_kanban_card`). The meta interaction is persisted
//! to `kanban_card_interaction` for the history viewer.

use crate::agents::core::agent::Task;
use crate::agents::execution::tool_loop::{self, PricingCache, ToolLoopContext};
use crate::commands::agent::hydrate_llm_from_model;
use crate::commands::kanban_card::create_kanban_card_core;
use crate::commands::kanban_interaction::persist_interaction;
use crate::commands::settings_kanban::{effective_compose_timeout, load_kanban_settings};
use crate::db::DBClient;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::models::kanban_card_interaction::InteractionKind;
use crate::models::{AgentConfig, KanbanCardCreate, KanbanCardStatus};
use crate::security::validate_uuid_field;
use crate::tools::list_agents::ListAgentsTool;
use crate::tools::submit_composed_card::SubmitComposedCardTool;
use crate::tools::{Tool, ToolFactory};
use crate::AppState;
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tracing::{info, instrument, warn};

/// Returned immediately by `start_compose_card` after the slot is reserved and
/// the detached task is spawned — the card itself arrives later via the
/// `kanban:compose_ready` event.
#[derive(Debug, Serialize)]
pub struct ComposeStartResponse {
    /// Tracking id, also the future card id (`compose_id == card_id`, DP-4).
    pub card_id: String,
}

/// Cap on the description input length (sanity, prompt budget protection).
const MAX_DESCRIPTION_LEN: usize = 4_000;

/// Composes a kanban card from a short user description.
///
/// `kanban_agent_id` identifies the meta-agent doing the composition; its
/// `llm` field selects the provider/model. The agent's system_prompt is
/// reused and prepended to the compose-mode instructions + the Submit
/// contract.
#[allow(clippy::too_many_arguments)]
pub async fn compose_card_from_description_core(
    db: &Arc<DBClient>,
    tool_factory: &Arc<ToolFactory>,
    mcp_manager: &Arc<MCPManager>,
    provider_manager: &Arc<ProviderManager>,
    kanban_agent_id: &str,
    description: &str,
    locale: &str,
    card_id: &str,
) -> Result<KanbanCardCreate, String> {
    let kanban_agent_id = validate_uuid_field(kanban_agent_id, "kanban_agent_id")?;
    // The card id is imposed by the caller (`start_compose_card` generates it
    // before reserving the slot) so the tracking id, the persisted compose
    // interaction and the eventual card row all share one UUID (DP-4).
    let pre_generated_card_id = validate_uuid_field(card_id, "card_id")?;
    let trimmed_desc = description.trim();
    if trimmed_desc.is_empty() {
        return Err("description cannot be empty".to_string());
    }
    if trimmed_desc.len() > MAX_DESCRIPTION_LEN {
        return Err(format!("description exceeds {} chars", MAX_DESCRIPTION_LEN));
    }

    let mut config = load_kanban_agent_config(db, &kanban_agent_id).await?;
    config.system_prompt = build_compose_system_prompt(&config.system_prompt);

    let capture: Arc<Mutex<Option<KanbanCardCreate>>> = Arc::new(Mutex::new(None));
    let extra_tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(ListAgentsTool::new(db.clone())),
        Arc::new(SubmitComposedCardTool::new(
            capture.clone(),
            kanban_agent_id.clone(),
            db.clone(),
        )),
    ];

    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: build_compose_user_prompt(trimmed_desc),
        // Compose in the UI language so the card title/description match the
        // user's language. Empty locale → tool loop falls back to default.
        context: if locale.trim().is_empty() {
            serde_json::json!({})
        } else {
            serde_json::json!({ "locale": locale })
        },
    };

    let pricing_cache = PricingCache::load(db, &config).await;

    let ctx = ToolLoopContext {
        config: &config,
        provider_manager,
        tool_factory: Some(tool_factory),
        agent_context: None,
        // Compose-card runs unattended: enforce the MCP tool allowlist.
        is_detached: true,
        // Direct detached run, not a delegate → delegation flag N/A.
        is_delegated: false,
    };
    let report = tool_loop::execute_with_tools(
        ctx,
        task,
        Some(mcp_manager.clone()),
        None,
        extra_tools,
        // Same root cause as the analyze flow: force a tool call on the opening
        // turn so the model engages SubmitComposedCard instead of finishing
        // with an empty capture slot. Auto afterwards so the loop can stop.
        crate::models::function_calling::ToolChoiceMode::Required,
    )
    .await
    .map_err(|e| format!("Compose tool_loop failed: {}", e))?;

    let mut card = capture.lock().await.take().ok_or_else(|| {
        "Agent did not call SubmitComposedCardTool. Review your system prompt or model choice."
            .to_string()
    })?;

    // Stamp the pre-generated card id on the returned proposal so the
    // subsequent create_kanban_card call uses the same UUID as the persisted
    // compose interaction.
    card.id = Some(pre_generated_card_id.clone());

    let summary = format!("title: {}", card.title);
    if let Err(e) = persist_interaction(
        db,
        &pre_generated_card_id,
        InteractionKind::Compose,
        &kanban_agent_id,
        &config.llm,
        trimmed_desc,
        &report,
        Some(&summary),
        &pricing_cache,
    )
    .await
    {
        warn!(error = %e, "Failed to persist compose interaction (non-fatal)");
    }

    info!(
        agent_id = %kanban_agent_id,
        target_agent_id = %card.target_agent_id,
        card_id = %pre_generated_card_id,
        "Compose-card produced KanbanCardCreate"
    );
    Ok(card)
}

/// Loads the full `AgentConfig` for a Kanban-kind agent, hydrating its
/// LLMConfig from the current `llm_model` row.
async fn load_kanban_agent_config(db: &Arc<DBClient>, id: &str) -> Result<AgentConfig, String> {
    let q = format!(
        "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, skills, \
         folders, require_file_confirmation, system_prompt, max_tool_iterations, \
         reasoning_effort, kind, auto_analyze_reports, mcp_tool_allowlist \
         FROM agent:`{}`",
        id
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to load Kanban agent: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Kanban agent not found: {}", id))?;
    if row["kind"].as_str() != Some("kanban") {
        return Err(format!(
            "Agent {} is not a Kanban-kind agent (kind={:?})",
            id,
            row["kind"].as_str()
        ));
    }
    let mut config: AgentConfig = serde_json::from_value(row)
        .map_err(|e| format!("Failed to deserialize Kanban agent config: {}", e))?;
    if config.llm.model.trim().is_empty() {
        return Err("Kanban agent has no LLM model configured".to_string());
    }
    // Re-sync model-owned fields (is_reasoning, context_window) so a stale
    // agent snapshot can't shadow Settings edits (ERR_LLM_010).
    hydrate_llm_from_model(db, &mut config.llm).await?;
    Ok(config)
}

fn build_compose_system_prompt(agent_system_prompt: &str) -> String {
    let mut s = String::new();
    if !agent_system_prompt.trim().is_empty() {
        s.push_str(agent_system_prompt.trim());
        s.push_str("\n\n");
    }
    s.push_str(
        "# Compose-card mode\n\n\
         Your task: read the user demand below and produce a kanban card proposal. \
         The card will be executed by a permanent worker agent that you must pick.\n\n\
         ## Workflow\n\
         1. Use ListAgents to discover available worker agents (target_agent_id candidates).\n\
         2. If a prompt library is available via tools (PromptManager), call \
            PromptManager.list_prompts then PromptManager.get_prompt on the one you pick. \
            The returned `variables` array tells you EXACTLY which keys to populate. \
            Otherwise compose an inline_prompt yourself.\n\
         3. Call SubmitComposedCard exactly ONCE with the final payload.\n\
         4. End your response with a brief rationale (2-3 sentences) explaining your choice.\n\n\
         ## Submit contract\n\
         You MUST call SubmitComposedCard exactly once before ending your turn. The card is \
         NOT persisted by this tool — the user reviews the proposal afterwards. Required: \
         title, target_agent_id, AND exactly one of (prompt_id, inline_prompt). \
         If you cannot decide a target_agent_id, prefer calling SubmitComposedCard with your \
         best guess plus an explanation in the description over not calling it at all.\n\n\
         ## Variables contract (STRICT)\n\
         When you pick a `prompt_id`, you MUST supply EVERY variable declared by that \
         prompt in the `variables` object — keys must match the names returned by \
         PromptManager.get_prompt. Missing keys are rejected and you will have to resubmit. \
         When you write an `inline_prompt`, use `{{name}}` placeholders and mirror each \
         placeholder name as a key in `variables`. Use `{}` only when there is genuinely no \
         variable to fill.\n",
    );
    s
}

fn build_compose_user_prompt(description: &str) -> String {
    format!(
        "User demand:\n\n{}\n\n\
         Compose the kanban card by calling SubmitComposedCard.",
        description
    )
}

/// Re-checks that the Kanban agent still exists (and is still Kanban-kind) just
/// before persisting a proposed card (M3). The agent could have been deleted
/// during the (potentially long) tool-loop; persisting a card pointing at a dead
/// `kanban_agent_id` would break the report viewer / re-analyze paths.
async fn kanban_agent_exists(db: &Arc<DBClient>, agent_id: &str) -> Result<bool, String> {
    let validated = validate_uuid_field(agent_id, "kanban_agent_id")?;
    let q = format!(
        "SELECT meta::id(id) AS id FROM agent:`{}` WHERE kind = 'kanban'",
        validated
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to re-check Kanban agent: {}", e))?;
    Ok(!rows.is_empty())
}

/// Emits `kanban:compose_ready` (best-effort — M4: no `AppHandle` is tolerated).
fn emit_compose_ready(app_handle: Option<&AppHandle>, card_id: &str, title: &str) {
    if let Some(handle) = app_handle {
        if let Err(e) = handle.emit(
            "kanban:compose_ready",
            json!({ "card_id": card_id, "title": title }),
        ) {
            warn!(error = %e, "Failed to emit kanban:compose_ready");
        }
    }
}

/// Emits `kanban:compose_failed` with an already-cleaned error message (I-1: the
/// `error` payload is a `String` produced by `map_err`/`?`, carrying no secret).
fn emit_compose_failed(app_handle: Option<&AppHandle>, card_id: &str, error: &str) {
    if let Some(handle) = app_handle {
        if let Err(e) = handle.emit(
            "kanban:compose_failed",
            json!({ "card_id": card_id, "error": error }),
        ) {
            warn!(error = %e, "Failed to emit kanban:compose_failed");
        }
    }
}

/// Detached compose body: runs the (timeout-bounded) tool-loop, re-validates the
/// agent, persists the proposed card and emits the lifecycle event. The compose
/// slot guard lives in the caller's spawned task and releases on ANY exit.
#[allow(clippy::too_many_arguments)]
async fn run_compose_task(
    db: Arc<DBClient>,
    tool_factory: Arc<ToolFactory>,
    mcp_manager: Arc<MCPManager>,
    llm_manager: Arc<ProviderManager>,
    app_handle: Option<AppHandle>,
    kanban_agent_id: String,
    description: String,
    locale: String,
    card_id: String,
) {
    // M-3: bound the whole compose so a pathological tool-loop cannot pin its
    // slot indefinitely. On timeout the slot is freed by the guard (in the
    // spawned task) and a `compose_failed` is emitted below. The ceiling is the
    // user-configurable `settings:kanban.compose_timeout_secs` (read fresh at
    // each run so a setting change applies without restart). The clamp + the
    // default-on-error fallback live in `effective_compose_timeout` so the
    // integration logic is unit-tested by construction.
    let timeout_secs = effective_compose_timeout(load_kanban_settings(&db).await);

    let outcome = tokio::time::timeout(
        Duration::from_secs(timeout_secs),
        compose_card_from_description_core(
            &db,
            &tool_factory,
            &mcp_manager,
            &llm_manager,
            &kanban_agent_id,
            &description,
            &locale,
            &card_id,
        ),
    )
    .await;

    let composed: Result<KanbanCardCreate, String> = match outcome {
        Ok(r) => r,
        Err(_) => Err(format!("Compose timed out after {}s", timeout_secs)),
    };

    let card = match composed {
        Ok(card) => card,
        Err(e) => {
            warn!(card_id = %card_id, error = %e, "Compose failed");
            emit_compose_failed(app_handle.as_ref(), &card_id, &e);
            return;
        }
    };

    // M3: re-validate the Kanban agent before persisting — abort with a clear
    // failure (no orphan card) if it was deleted during the run.
    match kanban_agent_exists(&db, &kanban_agent_id).await {
        Ok(true) => {}
        Ok(false) => {
            let msg = "The Kanban agent was deleted during generation".to_string();
            warn!(card_id = %card_id, "{}", msg);
            emit_compose_failed(app_handle.as_ref(), &card_id, &msg);
            return;
        }
        Err(e) => {
            emit_compose_failed(app_handle.as_ref(), &card_id, &e);
            return;
        }
    }

    // Persist as `proposed` via the single validated+bound create path (M-2).
    // `card.id` is already stamped with `card_id` by the core.
    match create_kanban_card_core(&db, card.clone(), KanbanCardStatus::Proposed).await {
        Ok(persisted) => {
            info!(card_id = %card_id, "Compose persisted as proposed card");
            emit_compose_ready(app_handle.as_ref(), &card_id, &persisted.title);
        }
        Err(e) => {
            warn!(card_id = %card_id, error = %e, "Failed to persist proposed card");
            emit_compose_failed(app_handle.as_ref(), &card_id, &e);
        }
    }
}

/// Tauri command — starts an async, detached card composition.
///
/// Validates the inputs, reserves a global compose slot (cap
/// `MAX_CONCURRENT_COMPOSE`, fail-closed when full) and spawns the detached
/// `run_compose_task`, returning the tracking `card_id` immediately. The card is
/// persisted (as `proposed`) ONLY on success; the frontend learns the outcome
/// via `kanban:compose_ready` / `kanban:compose_failed`.
#[tauri::command]
#[instrument(name = "start_compose_card", skip(state, description))]
pub async fn start_compose_card(
    kanban_agent_id: String,
    description: String,
    locale: String,
    state: State<'_, AppState>,
) -> Result<ComposeStartResponse, String> {
    // Fail-fast validation BEFORE reserving a slot / spawning.
    let kanban_agent_id = validate_uuid_field(&kanban_agent_id, "kanban_agent_id")?;
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Err("description cannot be empty".to_string());
    }
    if trimmed.len() > MAX_DESCRIPTION_LEN {
        return Err(format!("description exceeds {} chars", MAX_DESCRIPTION_LEN));
    }

    let card_id = uuid::Uuid::new_v4().to_string();

    // Atomic test-and-set reservation; the guard releases the slot on ANY task
    // exit (success, error, panic, timeout) once moved into the spawned task.
    let guard = state.try_reserve_compose_slot(&card_id)?;

    // Clone the Arcs / app_handle for the detached task. The app_handle is read
    // best-effort (None tolerated, M4) — it is normally set by the setup hook
    // well before any compose can be triggered from the UI.
    let db = state.db.clone();
    let tool_factory = state.tool_factory.clone();
    let mcp_manager = state.mcp_manager.clone();
    let llm_manager = state.llm_manager.clone();
    let app_handle = state.app_handle.read().ok().and_then(|g| g.clone());

    let card_id_for_task = card_id.clone();
    tokio::spawn(async move {
        // Holding the guard for the whole task lifetime releases the slot on drop
        // (covers panic/unwind and timeout, H-2).
        let _slot = guard;
        run_compose_task(
            db,
            tool_factory,
            mcp_manager,
            llm_manager,
            app_handle,
            kanban_agent_id,
            description,
            locale,
            card_id_for_task,
        )
        .await;
    });

    Ok(ComposeStartResponse { card_id })
}

/// Atomically validates a generated (`proposed`) card, flipping it to `ready`.
///
/// L-1: a SINGLE atomic conditional UPDATE (`WHERE status='proposed' RETURN
/// AFTER`) — never a pre-SELECT then UPDATE (TOCTOU). Zero rows (already
/// validated / rejected / not proposed) returns a clear error, not a false
/// success. `column` stays `todo`, so the scheduler promotes it like any ready
/// card. Extracted (`_core`) so the transition + guard are testable
/// without an `AppHandle`; the command wrapper adds the immediate promotion.
pub async fn approve_proposed_card_core(
    db: &DBClient,
    card_id: &str,
) -> Result<crate::models::KanbanCard, String> {
    let validated_id = validate_uuid_field(card_id, "card_id")?;
    // RETURN a JSON-safe projection (`meta::id(id)`), NOT `RETURN AFTER` — the
    // raw record `id` is a Thing enum that `query_json` cannot serialise
    // The full updated row is re-read via get_kanban_card_core.
    let q = format!(
        "UPDATE kanban_card:`{}` SET status = 'ready', updated_at = time::now() \
         WHERE status = 'proposed' RETURN meta::id(id) AS id",
        validated_id
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to approve proposed card: {}", e))?;
    if rows.is_empty() {
        return Err(
            "Card is not awaiting validation (already validated, rejected, or not found)"
                .to_string(),
        );
    }
    crate::commands::kanban_card::get_kanban_card_core(db, &validated_id).await
}

/// Tauri command — validates a generated (`proposed`) card, promoting it into the
/// scheduler queue. Kicks `start_next_pending_card_core` so a validated card
/// is promoted immediately instead of waiting for the next scheduler tick.
#[tauri::command]
#[instrument(name = "approve_proposed_card", skip(state), fields(card_id = %card_id))]
pub async fn approve_proposed_card(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<crate::models::KanbanCard, String> {
    let card = approve_proposed_card_core(&state.db, &card_id).await?;

    // Promote immediately (parity with create_kanban_card). Best-effort —
    // the scheduler tick would otherwise pick it up within 60s.
    let app_handle_opt = state.app_handle.read().ok().and_then(|g| g.clone());
    if let Some(handle) = app_handle_opt {
        if let Err(e) =
            crate::commands::scheduler::start_next_pending_card_core(&state.db, &handle).await
        {
            warn!(error = %e, "Failed to promote freshly approved card immediately");
        }
    }

    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    /// Seeds a kanban_card in the given status with column='todo'.
    async fn seed_card(db: &Arc<DBClient>, card_id: &str, status: &str) {
        let agent = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 't', description: '',
                kanban_agent_id: '{agent}', target_agent_id: '{agent}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: '{status}', `column`: 'todo',
                `column_order`: 0, workflow_id: NONE, review_chat_workflow_id: NONE,
                error_summary: NONE, created_at: time::now(), updated_at: time::now()
            }}"
        );
        db.execute(&q).await.unwrap();
    }

    /// L-1: approving a `proposed` card flips it to `ready` (column stays `todo`),
    /// returning the updated row so the scheduler can promote it.
    #[tokio::test]
    async fn approve_proposed_card_core_flips_to_ready() {
        let (state, _g) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        seed_card(&state.db, &card_id, "proposed").await;

        let card = approve_proposed_card_core(&state.db, &card_id)
            .await
            .expect("approving a proposed card succeeds");
        assert_eq!(card.status, KanbanCardStatus::Ready);
        assert!(matches!(card.column, crate::models::KanbanColumn::Todo));
    }

    /// L-1 guard: a card that is NOT proposed (e.g. already `ready`) cannot be
    /// approved — the conditional UPDATE matches 0 rows and returns a clear error
    /// instead of a false success.
    #[tokio::test]
    async fn approve_proposed_card_core_rejects_non_proposed() {
        let (state, _g) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        seed_card(&state.db, &card_id, "ready").await;

        let err = approve_proposed_card_core(&state.db, &card_id)
            .await
            .expect_err("a non-proposed card must not be approvable");
        assert!(err.contains("not awaiting validation"), "got: {err}");
    }

    /// L-1: a second approval of the same card (now `ready`) is refused — guards a
    /// double-click TOCTOU into a false success.
    #[tokio::test]
    async fn approve_proposed_card_core_second_approval_fails() {
        let (state, _g) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        seed_card(&state.db, &card_id, "proposed").await;

        approve_proposed_card_core(&state.db, &card_id)
            .await
            .expect("first approval succeeds");
        assert!(
            approve_proposed_card_core(&state.db, &card_id)
                .await
                .is_err(),
            "second approval must fail (card is already ready)"
        );
    }
}
