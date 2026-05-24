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
//! call PromptManager, Skills, etc.). The returned payload is NOT persisted
//! here — the caller reviews the proposal and (optionally) calls
//! `create_kanban_card_core`. The meta interaction itself is persisted to
//! `kanban_card_interaction` for the history viewer.

use crate::agents::core::agent::Task;
use crate::agents::execution::tool_loop::{self, PricingCache, ToolLoopContext};
use crate::commands::agent::hydrate_llm_from_model;
use crate::commands::kanban_interaction::persist_interaction;
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::kanban_card_interaction::InteractionKind;
use crate::models::{AgentConfig, KanbanCardCreate};
use crate::security::validate_uuid_field;
use crate::tools::list_agents::ListAgentsTool;
use crate::tools::submit_composed_card::SubmitComposedCardTool;
use crate::tools::{Tool, ToolFactory};
use crate::AppState;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use tracing::{info, instrument, warn};

/// Cap on the description input length (sanity, prompt budget protection).
const MAX_DESCRIPTION_LEN: usize = 4_000;

/// Composes a kanban card from a short user description.
///
/// `kanban_agent_id` identifies the meta-agent doing the composition; its
/// `llm` field selects the provider/model. The agent's system_prompt is
/// reused and prepended to the compose-mode instructions + the Submit
/// contract.
pub async fn compose_card_from_description_core(
    db: &Arc<DBClient>,
    tool_factory: &Arc<ToolFactory>,
    mcp_manager: &Arc<MCPManager>,
    provider_manager: &Arc<crate::llm::ProviderManager>,
    kanban_agent_id: &str,
    description: &str,
    locale: &str,
) -> Result<KanbanCardCreate, String> {
    let kanban_agent_id = validate_uuid_field(kanban_agent_id, "kanban_agent_id")?;
    let trimmed_desc = description.trim();
    if trimmed_desc.is_empty() {
        return Err("description cannot be empty".to_string());
    }
    if trimmed_desc.len() > MAX_DESCRIPTION_LEN {
        return Err(format!("description exceeds {} chars", MAX_DESCRIPTION_LEN));
    }

    let mut config = load_kanban_agent_config(db, &kanban_agent_id).await?;
    config.system_prompt = build_compose_system_prompt(&config.system_prompt);

    // Pre-generate the card id so the compose interaction can be persisted
    // and linked to the card BEFORE the user reviews and validates the
    // proposal. `create_kanban_card_core` honours the `id` field when
    // present (KanbanCardCreate.id is None for legacy callers).
    let pre_generated_card_id = uuid::Uuid::new_v4().to_string();

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
         reasoning_effort, kind, auto_analyze_reports \
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

/// Tauri command — composes a card proposal from a free-text description.
#[tauri::command]
#[instrument(name = "compose_card_from_description", skip(state, description))]
pub async fn compose_card_from_description(
    kanban_agent_id: String,
    description: String,
    locale: String,
    state: State<'_, AppState>,
) -> Result<KanbanCardCreate, String> {
    let result = compose_card_from_description_core(
        &state.db,
        &state.tool_factory,
        &state.mcp_manager,
        &state.llm_manager,
        &kanban_agent_id,
        &description,
        &locale,
    )
    .await;
    if let Err(ref e) = result {
        warn!(error = %e, "compose_card_from_description failed");
    }
    result
}
