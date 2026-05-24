// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Kanban report analyzer: after a workflow finishes, ask the parent
//! Kanban-kind agent to grade the report and decide what to do with
//! the card.
//!
//! Trigger: workflow_complete listener in main.rs, only when the card's
//! Kanban agent has `auto_analyze_reports = true`. Skipped otherwise.
//!
//! Runs the agent through `tool_loop::execute_with_tools` with a private
//! `SubmitAnalysisTool` (capturing the final verdict via `Arc<Mutex<_>>`)
//! and `ListAgentsTool` (for context discovery). The agent honours its own
//! `tools`, `mcp_servers` and `skills` so the verdict can be informed by
//! knowledge tools.
//!
//! Verdicts:
//! - `approve` -> card moves to column='done', status='done'
//! - `reject` -> card stays in column='review' with `error_summary` filled
//! - `needs_improvement` -> card stays in review; emits
//!   `kanban:needs_improvement` so the frontend can pre-open the prompt
//!   improvement modal with the suggested edit
//! - `skipped` -> agent had the flag disabled (handled before LLM call)

use crate::agents::core::agent::Task;
use crate::agents::execution::tool_loop::{self, PricingCache, ToolLoopContext};
use crate::commands::agent::hydrate_llm_from_model;
use crate::commands::kanban_card::get_kanban_card_core;
use crate::commands::kanban_interaction::persist_interaction;
use crate::db::DBClient;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::models::function_calling::ToolChoiceMode;
use crate::models::kanban_card_interaction::InteractionKind;
use crate::models::AgentConfig;
use crate::security::validate_uuid_field;
use crate::tools::list_agents::ListAgentsTool;
use crate::tools::submit_analysis::SubmitAnalysisTool;
use crate::tools::{Tool, ToolFactory};
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

/// Hard cap on report length passed to the analyzer LLM. Prevents a
/// runaway report from blowing the context window and the cost budget.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalyzeVerdict {
    Approve,
    Reject,
    NeedsImprovement,
    /// The Kanban agent has `auto_analyze_reports = false`. No LLM call,
    /// no card update — the caller can treat this as a no-op.
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeReport {
    pub verdict: AnalyzeVerdict,
    pub reasoning: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_prompt_edit: Option<String>,
}

/// Core analyzer entry point. Returns the verdict so callers (manual
/// trigger via Tauri command, listener spawn) can act on it.
#[instrument(skip(db, tool_factory, mcp_manager, llm_manager, app_handle), fields(card_id = %card_id))]
pub async fn analyze_card_report_core(
    db: &Arc<DBClient>,
    tool_factory: &Arc<ToolFactory>,
    mcp_manager: &Arc<MCPManager>,
    llm_manager: &Arc<ProviderManager>,
    app_handle: &AppHandle,
    card_id: &str,
) -> Result<AnalyzeReport, String> {
    let validated_card_id = validate_uuid_field(card_id, "card_id")?;

    let card = get_kanban_card_core(db, &validated_card_id).await?;
    let workflow_id = card.workflow_id.clone().ok_or_else(|| {
        format!(
            "Card {} has no workflow_id linked — cannot analyze report",
            validated_card_id
        )
    })?;

    let mut config = load_kanban_agent_for_analysis(db, &card.kanban_agent_id).await?;
    if !config.auto_analyze_reports {
        debug!(
            kanban_agent_id = %card.kanban_agent_id,
            "Kanban agent has auto_analyze_reports=false — skipping"
        );
        return Ok(AnalyzeReport {
            verdict: AnalyzeVerdict::Skipped,
            reasoning: "auto_analyze_reports is disabled on this Kanban agent".to_string(),
            suggested_prompt_edit: None,
        });
    }

    // Signal to the frontend that the Kanban agent is now finalizing this
    // card's report. The matching "done" signal is the existing
    // `kanban:auto_analyzed` / `kanban:needs_improvement` event emitted at
    // the end of this function (or the on-error fallback in the caller).
    let _ = app_handle.emit(
        "kanban:analyzing",
        json!({ "card_id": validated_card_id.clone() }),
    );

    let report_text = load_workflow_report(db, &workflow_id).await?;
    // Inherit the language the worker workflow ran in (stamped at execution),
    // so the verdict is produced in the user's language without a frontend
    // round-trip. Absent on legacy workflows → tool loop falls back to default.
    let locale = load_workflow_locale(db, &workflow_id).await;
    config.system_prompt = build_analyze_system_prompt(&config.system_prompt);

    let capture: Arc<Mutex<Option<AnalyzeReport>>> = Arc::new(Mutex::new(None));
    let extra_tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(ListAgentsTool::new(db.clone())),
        Arc::new(SubmitAnalysisTool::new(capture.clone())),
    ];

    let user_prompt =
        build_analyze_user_prompt(&card.title, &card.description, &workflow_id, &report_text);
    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: user_prompt.clone(),
        context: match &locale {
            Some(l) => json!({ "locale": l }),
            None => json!({}),
        },
    };

    let pricing_cache = PricingCache::load(db, &config).await;

    let ctx = ToolLoopContext {
        config: &config,
        provider_manager: llm_manager,
        tool_factory: Some(tool_factory),
        agent_context: None,
    };
    let exec_report = tool_loop::execute_with_tools(
        ctx,
        task,
        Some(mcp_manager.clone()),
        None,
        extra_tools,
        // Force a tool call on the opening turn so the model engages
        // SubmitAnalysis instead of writing prose and finishing with an empty
        // capture slot (the silent-failure root cause). Subsequent turns revert
        // to Auto so the loop can terminate once the verdict is submitted.
        ToolChoiceMode::Required,
    )
    .await
    .map_err(|e| format!("Analyze tool_loop failed: {}", e))?;

    let analysis = capture.lock().await.take().ok_or_else(|| {
        "Agent did not call SubmitAnalysisTool. Review your system prompt or model choice."
            .to_string()
    })?;

    let summary = format!("verdict: {:?}", analysis.verdict);
    if let Err(e) = persist_interaction(
        db,
        &validated_card_id,
        InteractionKind::Analyze,
        &card.kanban_agent_id,
        &config.llm,
        &user_prompt,
        &exec_report,
        Some(&summary),
        &pricing_cache,
    )
    .await
    {
        warn!(error = %e, "Failed to persist analyze interaction (non-fatal)");
    }

    apply_verdict(db, &validated_card_id, &analysis).await?;
    emit_verdict_event(app_handle, &validated_card_id, &analysis);
    info!(
        card_id = %validated_card_id,
        verdict = ?analysis.verdict,
        "Kanban auto-analyze finished"
    );
    Ok(analysis)
}

/// Boot-time catch-up: re-run the analyzer for every card that finished into
/// `review` but was never analyzed.
///
/// A card lands here when the app was closed between the worker workflow
/// completing and the `workflow_complete` listener firing, or when an earlier
/// auto-analyze failed silently (pre-`tool_choice=Required`). Without this
/// pass such cards stay in `review` forever with no analyze interaction.
///
/// The victim set is `column='review'` AND `status='done'` (success path) AND
/// `workflow_id` present AND no `analyze` interaction yet. `reject` /
/// `needs_improvement` cards are excluded by construction — they already have
/// an analyze interaction. Approved cards left `review` (moved to `done`).
///
/// Each analysis is best-effort and gated by `auto_analyze_reports` inside
/// `analyze_card_report_core` (returns `Skipped` when the agent disabled it).
/// Returns the number of cards for which a verdict was produced.
pub async fn catchup_unanalyzed_review_cards_core(
    db: &Arc<DBClient>,
    tool_factory: &Arc<ToolFactory>,
    mcp_manager: &Arc<MCPManager>,
    llm_manager: &Arc<ProviderManager>,
    app_handle: &AppHandle,
) -> Result<usize, String> {
    let q = "SELECT meta::id(id) AS id FROM kanban_card \
             WHERE `column` = 'review' \
               AND status = 'done' \
               AND workflow_id != NONE \
               AND meta::id(id) NOT IN \
                   (SELECT VALUE card_id FROM kanban_card_interaction WHERE kind = 'analyze')";
    let rows = db
        .query_json(q)
        .await
        .map_err(|e| format!("Failed to pick un-analyzed review cards: {}", e))?;
    let card_ids: Vec<String> = rows
        .iter()
        .filter_map(|r| r["id"].as_str().map(String::from))
        .collect();
    if card_ids.is_empty() {
        return Ok(0);
    }
    info!(
        count = card_ids.len(),
        "Kanban catch-up: re-analyzing un-analyzed review cards"
    );

    let mut analyzed = 0usize;
    for card_id in card_ids {
        match analyze_card_report_core(
            db,
            tool_factory,
            mcp_manager,
            llm_manager,
            app_handle,
            &card_id,
        )
        .await
        {
            Ok(report) if report.verdict != AnalyzeVerdict::Skipped => analyzed += 1,
            Ok(_) => {} // Skipped: agent has auto_analyze_reports disabled
            Err(e) => {
                warn!(card_id = %card_id, error = %e, "Kanban catch-up: analyze failed (non-fatal)");
            }
        }
    }
    Ok(analyzed)
}

async fn load_kanban_agent_for_analysis(
    db: &Arc<DBClient>,
    agent_id: &str,
) -> Result<AgentConfig, String> {
    let q = format!(
        "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, skills, \
         folders, require_file_confirmation, system_prompt, max_tool_iterations, \
         reasoning_effort, kind, auto_analyze_reports \
         FROM agent:`{}`",
        agent_id
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to load Kanban agent: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Kanban agent not found: {}", agent_id))?;
    if row["kind"].as_str() != Some("kanban") {
        return Err(format!("Agent {} is not Kanban-kind", agent_id));
    }
    let mut config: AgentConfig = serde_json::from_value(row)
        .map_err(|e| format!("Failed to deserialize Kanban agent config: {}", e))?;
    if config.llm.model.trim().is_empty() {
        return Err("Kanban agent has no LLM model configured".to_string());
    }
    hydrate_llm_from_model(db, &mut config.llm).await?;
    Ok(config)
}

async fn load_workflow_report(db: &Arc<DBClient>, workflow_id: &str) -> Result<String, String> {
    let validated_wf = validate_uuid_field(workflow_id, "workflow_id")?;
    let q = "SELECT content, timestamp FROM message \
             WHERE workflow_id = $wid AND role = 'assistant' \
             ORDER BY timestamp DESC LIMIT 1";
    let rows: Vec<serde_json::Value> = db
        .query_with_params(q, vec![("wid".to_string(), json!(validated_wf))])
        .await
        .map_err(|e| format!("Failed to load workflow report: {}", e))?;
    let content = rows
        .into_iter()
        .next()
        .and_then(|r| r["content"].as_str().map(String::from))
        .ok_or_else(|| format!("No assistant message found for workflow {}", validated_wf))?;
    // The full report is fed to the analyzer verbatim — never truncated.
    // A partial report could hide the very issue the verdict must catch.
    Ok(content)
}

/// Reads the UI language stamped on a workflow at execution time.
///
/// Returns `None` when the workflow predates the `locale` field or the lookup
/// fails — the analyze tool loop then falls back to its default language.
/// Best-effort: a DB error is swallowed (the analysis must still run).
async fn load_workflow_locale(db: &Arc<DBClient>, workflow_id: &str) -> Option<String> {
    let validated_wf = validate_uuid_field(workflow_id, "workflow_id").ok()?;
    let q = "SELECT locale FROM workflow WHERE id = $wid LIMIT 1";
    let rows: Vec<serde_json::Value> = db
        .query_with_params(q, vec![("wid".to_string(), json!(validated_wf))])
        .await
        .ok()?;
    rows.into_iter()
        .next()
        .and_then(|r| r["locale"].as_str().map(String::from))
        .filter(|s| !s.trim().is_empty())
}

fn build_analyze_system_prompt(agent_sp: &str) -> String {
    let mut s = String::new();
    if !agent_sp.trim().is_empty() {
        s.push_str(agent_sp.trim());
        s.push_str("\n\n");
    }
    s.push_str(
        "# Report analysis mode\n\n\
         You are reviewing the output of a worker agent you orchestrated. \
         Read the user's original demand (title + description) and the \
         worker's final report, then return a structured verdict by calling \
         SubmitAnalysis exactly once.\n\n\
         ## Possible verdicts\n\n\
         - `approve` — The report fulfils the demand correctly and is ready \
           for the user. The card will be auto-moved to Done.\n\
         - `reject` — The report is wrong, incomplete, or off-topic and cannot \
           be salvaged by editing the prompt. The card stays in Review and \
           your `reasoning` is shown to the user as the rejection rationale.\n\
         - `needs_improvement` — The report has issues but they likely come \
           from the prompt itself. Provide a `suggested_prompt_edit` with the \
           full new prompt text; the user will be invited to apply it.\n\n\
         ## Submit contract\n\n\
         You MUST call SubmitAnalysis exactly once before ending your turn. \
         You may use any of your other assigned tools (skills, memory, MCP) \
         to inform your verdict.\n\n\
         ## Going deeper (optional)\n\n\
         If your tool set includes WorkflowManager, you can use it to inspect the workflow \
         behind the report on the workflow id provided in the user prompt:\n\
         - `WorkflowManager.read_workflow({workflow_id, include_messages: true})` returns \
           the last user/assistant turns, total cost, completion time — useful when the \
           summary report alone is ambiguous.\n\
         - `WorkflowManager.list_workflow_errors({workflow_id})` lists failed tool calls \
           with iteration, agent_id and duration so you can tell whether failures came \
           from the orchestrator or a sub-agent.\n\
         - `WorkflowManager.list_workflow_sub_agents({workflow_id})` lists every sub-agent \
           execution (successes included), with sub_agent_id, sub_agent_name, status, \
           duration and cost. Useful to know exactly which permanent agents participated \
           and pick the right target_agent_id when you want to revise that agent's skills.\n\
         Skip these when the report is clear; they cost extra tokens.\n",
    );
    s
}

fn build_analyze_user_prompt(
    title: &str,
    description: &str,
    workflow_id: &str,
    report: &str,
) -> String {
    format!(
        "## Card title\n{}\n\n\
         ## Original demand\n{}\n\n\
         ## Workflow id\n{}\n\n\
         ## Worker's final report\n{}\n\n\
         Analyse this report against the demand and call SubmitAnalysis with your verdict. \
         If you have WorkflowManager available, you may call WorkflowManager.read_workflow \
         (with `include_messages: true` to see intermediate turns) or \
         WorkflowManager.list_workflow_errors on the workflow id above to dig deeper before \
         deciding.",
        title.trim(),
        description.trim(),
        workflow_id,
        report
    )
}

async fn apply_verdict(
    db: &Arc<DBClient>,
    card_id: &str,
    analysis: &AnalyzeReport,
) -> Result<(), String> {
    match analysis.verdict {
        AnalyzeVerdict::Approve => {
            let q = format!(
                "UPDATE kanban_card:`{}` SET `column` = 'done', status = 'done', \
                 error_summary = NONE, updated_at = time::now()",
                card_id
            );
            db.execute(&q)
                .await
                .map_err(|e| format!("Failed to approve card: {}", e))?;
        }
        AnalyzeVerdict::Reject => {
            let reasoning_json = serde_json::to_string(&analysis.reasoning)
                .map_err(|e| format!("Failed to serialize reasoning: {}", e))?;
            let q = format!(
                "UPDATE kanban_card:`{}` SET error_summary = {}, updated_at = time::now()",
                card_id, reasoning_json
            );
            db.execute(&q)
                .await
                .map_err(|e| format!("Failed to reject card: {}", e))?;
        }
        AnalyzeVerdict::NeedsImprovement | AnalyzeVerdict::Skipped => {
            // No DB mutation. The frontend reacts to the event payload.
        }
    }
    Ok(())
}

fn emit_verdict_event(app_handle: &AppHandle, card_id: &str, analysis: &AnalyzeReport) {
    let event = match analysis.verdict {
        AnalyzeVerdict::NeedsImprovement => "kanban:needs_improvement",
        _ => "kanban:auto_analyzed",
    };
    let payload = json!({
        "card_id": card_id,
        "verdict": analysis.verdict,
        "reasoning": analysis.reasoning,
        "suggested_prompt_edit": analysis.suggested_prompt_edit,
    });
    if let Err(e) = app_handle.emit(event, payload) {
        warn!(error = %e, event = event, "Failed to emit auto-analyze event");
    }
}

/// Tauri command — manual trigger from the UI (e.g. a "Re-analyze" button).
#[tauri::command]
#[instrument(name = "analyze_card_report", skip(state, app_handle), fields(card_id = %card_id))]
pub async fn analyze_card_report(
    card_id: String,
    state: State<'_, AppState>,
    app_handle: AppHandle,
) -> Result<AnalyzeReport, String> {
    info!("Manually analyzing card report");
    analyze_card_report_core(
        &state.db,
        &state.tool_factory,
        &state.mcp_manager,
        &state.llm_manager,
        &app_handle,
        &card_id,
    )
    .await
}
