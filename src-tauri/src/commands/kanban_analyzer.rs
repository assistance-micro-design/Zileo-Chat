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
//! Verdicts:
//! - `approve` → card moves to column='done', status='done'
//! - `reject` → card stays in column='review' with `error_summary` filled
//! - `needs_improvement` → card stays in review; emits
//!   `kanban:needs_improvement` so the frontend can pre-open the prompt
//!   improvement modal with the suggested edit
//! - `skipped` → agent had the flag disabled (handled before LLM call)

use crate::commands::agent::hydrate_llm_from_model;
use crate::commands::compose_card::extract_json_payload;
use crate::commands::kanban_card::get_kanban_card_core;
use crate::db::DBClient;
use crate::llm::{CompletionParams, ProviderManager, ProviderType};
use crate::models::agent::ReasoningEffort;
use crate::models::LLMConfig;
use crate::security::validate_uuid_field;
use crate::tools::utils::safe_truncate;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tracing::{debug, info, instrument, warn};

/// Hard cap on report length passed to the analyzer LLM. Prevents a
/// runaway report from blowing the context window and the cost budget.
const MAX_REPORT_CHARS_FOR_ANALYSIS: usize = 12_000;

/// Sampling temperature for the analyzer — low because we want a
/// structured verdict, not creative writing.
const ANALYZE_TEMPERATURE: f64 = 0.2;

/// Cap on the LLM response: the JSON verdict is small (a few hundred
/// chars) plus an optional prompt edit. 4000 tokens leaves room for the
/// reasoning text.
const ANALYZE_MAX_OUTPUT_TOKENS: usize = 4_000;

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

struct AnalysisAgent {
    system_prompt: String,
    llm: LLMConfig,
    reasoning_effort: Option<ReasoningEffort>,
    auto_analyze_reports: bool,
}

/// Core analyzer entry point. Returns the verdict so callers (manual
/// trigger via Tauri command, listener spawn) can act on it.
#[instrument(skip(db, llm_manager, app_handle), fields(card_id = %card_id))]
pub async fn analyze_card_report_core(
    db: &Arc<DBClient>,
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

    let agent = load_kanban_agent_for_analysis(db, &card.kanban_agent_id).await?;
    if !agent.auto_analyze_reports {
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

    let report = load_workflow_report(db, &workflow_id).await?;

    let system_prompt = build_analyze_system_prompt(&agent.system_prompt);
    let user_prompt = build_analyze_user_prompt(&card.title, &card.description, &report);

    let provider = ProviderType::from_str(&agent.llm.provider)
        .map_err(|e| format!("Invalid provider '{}': {}", agent.llm.provider, e))?;
    let reasoning_effort = if agent.llm.is_reasoning {
        agent.reasoning_effort.clone()
    } else {
        None
    };
    let params = CompletionParams {
        prompt: user_prompt,
        system_prompt: Some(system_prompt),
        model: Some(agent.llm.model.clone()),
        temperature: ANALYZE_TEMPERATURE,
        max_tokens: agent.llm.max_tokens.min(ANALYZE_MAX_OUTPUT_TOKENS),
        reasoning_effort,
        context_window: agent.llm.context_window,
    };
    let response = llm_manager
        .complete_with_provider(provider, params)
        .await
        .map_err(|e| format!("Analyzer LLM completion failed: {}", e))?;
    debug!(
        provider = %agent.llm.provider,
        model = %agent.llm.model,
        tokens_in = response.tokens_input,
        tokens_out = response.tokens_output,
        "Analyzer LLM completion done"
    );

    let analysis = parse_analyze_response(&response.content)?;
    apply_verdict(db, &validated_card_id, &analysis).await?;
    emit_verdict_event(app_handle, &validated_card_id, &analysis);
    info!(
        card_id = %validated_card_id,
        verdict = ?analysis.verdict,
        "Kanban auto-analyze finished"
    );
    Ok(analysis)
}

async fn load_kanban_agent_for_analysis(
    db: &Arc<DBClient>,
    agent_id: &str,
) -> Result<AnalysisAgent, String> {
    let q = format!(
        "SELECT system_prompt, kind, llm, reasoning_effort, auto_analyze_reports FROM agent:`{}`",
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
    let system_prompt = row["system_prompt"].as_str().unwrap_or("").to_string();
    let auto_analyze_reports = row["auto_analyze_reports"].as_bool().unwrap_or(false);

    let llm_value = row
        .get("llm")
        .cloned()
        .ok_or_else(|| "Kanban agent has no llm config".to_string())?;
    let mut llm: LLMConfig = serde_json::from_value(llm_value)
        .map_err(|e| format!("Failed to deserialize agent llm config: {}", e))?;
    if llm.model.trim().is_empty() {
        return Err("Kanban agent has no LLM model configured".to_string());
    }
    hydrate_llm_from_model(db, &mut llm).await?;

    let reasoning_effort: Option<ReasoningEffort> = row
        .get("reasoning_effort")
        .filter(|v| !v.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("Failed to deserialize reasoning_effort: {}", e))?;

    Ok(AnalysisAgent {
        system_prompt,
        llm,
        reasoning_effort,
        auto_analyze_reports,
    })
}

async fn load_workflow_report(db: &Arc<DBClient>, workflow_id: &str) -> Result<String, String> {
    let validated_wf = validate_uuid_field(workflow_id, "workflow_id")?;
    let q = "SELECT content, created_at FROM message \
             WHERE workflow_id = $wid AND role = 'assistant' \
             ORDER BY created_at DESC LIMIT 1";
    let rows: Vec<serde_json::Value> = db
        .query_with_params(q, vec![("wid".to_string(), json!(validated_wf))])
        .await
        .map_err(|e| format!("Failed to load workflow report: {}", e))?;
    let content = rows
        .into_iter()
        .next()
        .and_then(|r| r["content"].as_str().map(String::from))
        .ok_or_else(|| format!("No assistant message found for workflow {}", validated_wf))?;
    Ok(safe_truncate(&content, MAX_REPORT_CHARS_FOR_ANALYSIS, true))
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
         worker's final report, then return a structured verdict.\n\n\
         ## Possible verdicts\n\n\
         - `approve` — The report fulfils the demand correctly and is ready \
           for the user. The card will be auto-moved to Done.\n\
         - `reject` — The report is wrong, incomplete, or off-topic and cannot \
           be salvaged by editing the prompt. The card stays in Review and \
           your `reasoning` is shown to the user as the rejection rationale.\n\
         - `needs_improvement` — The report has issues but they likely come \
           from the prompt itself. Provide a `suggested_prompt_edit` with the \
           full new prompt text; the user will be invited to apply it.\n\n\
         ## Output contract (STRICT)\n\n\
         Reply with ONE JSON object only, no prose, no markdown fences:\n\
         - `verdict` (string, one of: \"approve\" | \"reject\" | \"needs_improvement\")\n\
         - `reasoning` (string, 20..=2000 chars, in the user's language)\n\
         - `suggested_prompt_edit` (string, only for `needs_improvement`; the FULL \
           replacement prompt text, not a diff)\n",
    );
    s
}

fn build_analyze_user_prompt(title: &str, description: &str, report: &str) -> String {
    format!(
        "## Card title\n{}\n\n\
         ## Original demand\n{}\n\n\
         ## Worker's final report\n{}\n\n\
         Analyse this report against the demand and emit your verdict JSON.",
        title.trim(),
        description.trim(),
        report
    )
}

fn parse_analyze_response(content: &str) -> Result<AnalyzeReport, String> {
    let payload = extract_json_payload(content).ok_or_else(|| {
        format!(
            "Analyzer response did not contain a JSON object: {}",
            content
        )
    })?;

    let verdict_str = payload["verdict"]
        .as_str()
        .ok_or_else(|| "Missing verdict field".to_string())?
        .trim()
        .to_lowercase();
    let verdict = match verdict_str.as_str() {
        "approve" => AnalyzeVerdict::Approve,
        "reject" => AnalyzeVerdict::Reject,
        "needs_improvement" => AnalyzeVerdict::NeedsImprovement,
        other => return Err(format!("Unknown verdict: {}", other)),
    };

    let reasoning = payload["reasoning"]
        .as_str()
        .ok_or_else(|| "Missing reasoning field".to_string())?
        .trim()
        .to_string();
    if reasoning.len() < 5 {
        return Err("Reasoning is too short".to_string());
    }
    if reasoning.len() > 4_000 {
        return Err("Reasoning exceeds 4000 chars".to_string());
    }

    let suggested_prompt_edit = if matches!(verdict, AnalyzeVerdict::NeedsImprovement) {
        let raw = payload["suggested_prompt_edit"]
            .as_str()
            .ok_or_else(|| "needs_improvement verdict requires suggested_prompt_edit".to_string())?
            .trim()
            .to_string();
        if raw.is_empty() {
            return Err("suggested_prompt_edit is empty".to_string());
        }
        Some(raw)
    } else {
        None
    };

    Ok(AnalyzeReport {
        verdict,
        reasoning,
        suggested_prompt_edit,
    })
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
    analyze_card_report_core(&state.db, &state.llm_manager, &app_handle, &card_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_approve_verdict() {
        let raw = r#"{"verdict":"approve","reasoning":"Report fully addresses the user's demand with accurate facts."}"#;
        let r = parse_analyze_response(raw).unwrap();
        assert_eq!(r.verdict, AnalyzeVerdict::Approve);
        assert!(r.suggested_prompt_edit.is_none());
    }

    #[test]
    fn parse_reject_verdict() {
        let raw = r#"{"verdict":"reject","reasoning":"Report is off-topic, talks about unrelated company."}"#;
        let r = parse_analyze_response(raw).unwrap();
        assert_eq!(r.verdict, AnalyzeVerdict::Reject);
    }

    #[test]
    fn parse_needs_improvement_requires_edit() {
        let raw = r#"{"verdict":"needs_improvement","reasoning":"Prompt was too vague to constrain output."}"#;
        assert!(parse_analyze_response(raw).is_err());
    }

    #[test]
    fn parse_needs_improvement_with_edit() {
        let raw = r#"{"verdict":"needs_improvement","reasoning":"Prompt was too vague to constrain output.","suggested_prompt_edit":"Search Mistral AI's official blog for the latest 5 announcements published in the last 30 days. Return them as a bullet list with dates."}"#;
        let r = parse_analyze_response(raw).unwrap();
        assert_eq!(r.verdict, AnalyzeVerdict::NeedsImprovement);
        assert!(r.suggested_prompt_edit.is_some());
    }

    #[test]
    fn parse_unknown_verdict_fails() {
        let raw = r#"{"verdict":"maybe","reasoning":"unclear."}"#;
        assert!(parse_analyze_response(raw).is_err());
    }

    #[test]
    fn parse_garbage_fails() {
        assert!(parse_analyze_response("no json here").is_err());
    }

    #[test]
    fn parse_fenced_response() {
        let raw =
            "```json\n{\"verdict\":\"approve\",\"reasoning\":\"All good and complete.\"}\n```";
        let r = parse_analyze_response(raw).unwrap();
        assert_eq!(r.verdict, AnalyzeVerdict::Approve);
    }
}
