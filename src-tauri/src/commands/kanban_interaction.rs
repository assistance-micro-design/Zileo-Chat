// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Lecture des interactions meta de l'agent Kanban pour affichage historique.
//!
//! Une "interaction" est un tool_loop execute par compose_card.rs ou
//! kanban_analyzer.rs. Les writes sont realises dans ces modules ; cette
//! commande expose uniquement la lecture pour le viewer.

use crate::agents::core::agent::Report;
use crate::agents::execution::tool_loop::PricingCache;
use crate::db::DBClient;
use crate::models::kanban_card_interaction::{
    InteractionIteration, InteractionKind, InteractionToolCall, KanbanCardInteraction,
};
use crate::models::LLMConfig;
use crate::security::validate_uuid_field;
use crate::AppState;
use serde_json::json;
use std::sync::Arc;
use tauri::State;
use tracing::{instrument, warn};

/// Champs SELECT canoniques pour reconstruire un `KanbanCardInteraction`.
///
/// Garder synchronise avec la struct (PAT_PERSISTED_FIELD_RUST_STRUCT_SYNC) :
/// si on ajoute un champ persiste sur la table, l'ajouter ici sinon il sera
/// silencieusement omis a la lecture.
const INTERACTION_FIELDS: &str = "meta::id(id) AS id, card_id, kind, kanban_agent_id, \
    provider, model_id_used, task_input, iterations, final_payload_summary, \
    final_response_text, total_tokens_input, total_tokens_output, total_cost_usd, \
    created_at";

const MAX_INTERACTIONS_PER_CARD: usize = 100;

/// Charge les interactions meta d'une carte (compose + analyze, dans l'ordre).
///
/// Retourne une liste vide si la carte n'a aucune interaction historisee
/// (cas standard pour les anciennes cartes anterieures a la feature).
///
/// Pattern PAT_RUST_015 : la commande Tauri delegue ici pour pouvoir tester
/// la logique sans instancier `tauri::State`.
pub async fn load_card_interactions_core(
    db: &Arc<DBClient>,
    card_id: &str,
) -> Result<Vec<KanbanCardInteraction>, String> {
    let validated = validate_uuid_field(card_id, "card_id")?;
    let query = format!(
        "SELECT {INTERACTION_FIELDS} FROM kanban_card_interaction \
         WHERE card_id = $card_id \
         ORDER BY created_at ASC \
         LIMIT {MAX_INTERACTIONS_PER_CARD}"
    );
    let rows = db
        .query_json_with_params(&query, vec![("card_id".to_string(), json!(validated))])
        .await
        .map_err(|e| format!("Failed to query kanban_card_interaction: {}", e))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        match serde_json::from_value::<KanbanCardInteraction>(row.clone()) {
            Ok(interaction) => out.push(interaction),
            Err(e) => {
                warn!(
                    card_id = %validated,
                    error = %e,
                    raw = %row,
                    "Skipping malformed kanban_card_interaction row"
                );
            }
        }
    }
    Ok(out)
}

#[tauri::command]
#[instrument(name = "load_card_interactions", skip(state), fields(card_id = %card_id))]
pub async fn load_card_interactions(
    card_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<KanbanCardInteraction>, String> {
    load_card_interactions_core(&state.db, &card_id).await
}

/// Hard cap on the `task_input` column. The input is a snapshot of the user
/// demand (compose) or the workflow report (analyze). The latter is already
/// `safe_truncate`d upstream but we re-cap defensively to keep DB rows bounded.
const MAX_TASK_INPUT_CHARS: usize = 16_000;

/// Hard cap on `final_response_text` (the LLM's trailing rationale).
const MAX_FINAL_RESPONSE_CHARS: usize = 8_000;

/// Hard cap on a single tool call's serialized input or output JSON.
const MAX_TOOL_JSON_CHARS: usize = 16_000;

fn truncate_chars(s: &str, max: usize) -> String {
    crate::tools::utils::safe_truncate(s, max, true)
}

/// Maps a `Report` produced by `tool_loop::execute_with_tools` into the
/// per-iteration shape persisted on `kanban_card_interaction`.
///
/// Tool executions are grouped by their `iteration` field (set by
/// `iteration.rs`). Reasoning steps from `ReasoningSource::ModelThinking` are
/// attached to the iteration whose tool-execution sequence range covers the
/// step's `sequence`; steps that fall past the last tool execution land on the
/// last iteration (typically the final tool-less turn).
fn report_to_iterations(report: &Report, pricing: &PricingCache) -> Vec<InteractionIteration> {
    use crate::agents::core::agent::ReasoningSource;

    let metrics = &report.metrics;
    let mut iterations: Vec<InteractionIteration> = metrics
        .iteration_metrics
        .iter()
        .map(|m| {
            let cost_usd = pricing
                .compute_iteration_local_cost(
                    m.tokens_input,
                    m.tokens_output,
                    m.cached_tokens,
                    m.cache_write_tokens,
                )
                .unwrap_or(0.0);
            InteractionIteration {
                iteration_index: m.iteration,
                reasoning: None,
                response_content: None,
                tool_calls: Vec::new(),
                tokens_input: m.tokens_input as u64,
                tokens_output: m.tokens_output as u64,
                cached_tokens: m.cached_tokens.unwrap_or(0) as u64,
                cost_usd,
                duration_ms: m.duration_ms,
            }
        })
        .collect();

    // Group tool_executions by iteration.
    for te in &metrics.tool_executions {
        if let Some(slot) = iterations
            .iter_mut()
            .find(|it| it.iteration_index == te.iteration)
        {
            slot.tool_calls.push(InteractionToolCall {
                tool_name: te.tool_name.clone(),
                mcp_server: te.server_name.clone(),
                input_json: truncate_chars(&te.input_params.to_string(), MAX_TOOL_JSON_CHARS),
                output_json: truncate_chars(&te.output_result.to_string(), MAX_TOOL_JSON_CHARS),
                duration_ms: te.duration_ms,
                success: te.success,
            });
        }
    }

    // Compute the upper sequence bound per iteration from its tool executions
    // (None if the iteration had no tool calls).
    let bounds: Vec<Option<u32>> = iterations
        .iter()
        .map(|it| {
            metrics
                .tool_executions
                .iter()
                .filter(|te| te.iteration == it.iteration_index)
                .map(|te| te.sequence)
                .max()
        })
        .collect();

    // Assign each ModelThinking reasoning step to the first iteration whose
    // tool-execution max-sequence is >= step.sequence; fallback to the last.
    let mut per_iter_reasoning: Vec<Vec<String>> = vec![Vec::new(); iterations.len()];
    for step in &metrics.reasoning_steps {
        if step.source != ReasoningSource::ModelThinking {
            continue;
        }
        let target = bounds
            .iter()
            .position(|b| b.is_some_and(|max| step.sequence <= max))
            .unwrap_or(per_iter_reasoning.len().saturating_sub(1));
        if let Some(bucket) = per_iter_reasoning.get_mut(target) {
            bucket.push(step.content.clone());
        }
    }
    for (slot, bucket) in iterations.iter_mut().zip(per_iter_reasoning) {
        if !bucket.is_empty() {
            slot.reasoning = Some(bucket.join("\n\n"));
        }
    }

    // Attach the final LLM response (assistant text) to the last iteration as
    // `response_content` so the viewer can render it next to the tool calls.
    if let Some(last) = iterations.last_mut() {
        let resp = report.response.trim();
        if !resp.is_empty() {
            last.response_content = Some(truncate_chars(resp, MAX_FINAL_RESPONSE_CHARS));
        }
    }

    iterations
}

/// Persists a single meta interaction (compose or analyze) on a Kanban card.
///
/// Converts the `Report` produced by `tool_loop::execute_with_tools` into a
/// `KanbanCardInteraction` row. Returns the inserted row id.
///
/// PAT_PERSISTED_FIELD_RUST_STRUCT_SYNC: any new field must be mirrored in
/// the schema, the struct, the SELECT in `load_card_interactions_core`, and
/// the INSERT below.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn persist_interaction(
    db: &Arc<DBClient>,
    card_id: &str,
    kind: InteractionKind,
    kanban_agent_id: &str,
    llm: &LLMConfig,
    task_input: &str,
    report: &Report,
    final_payload_summary: Option<&str>,
    pricing_cache: &PricingCache,
) -> Result<String, String> {
    let validated_card_id = validate_uuid_field(card_id, "card_id")?;
    let iterations = report_to_iterations(report, pricing_cache);

    let total_tokens_input: u64 = iterations.iter().map(|i| i.tokens_input).sum();
    let total_tokens_output: u64 = iterations.iter().map(|i| i.tokens_output).sum();
    let total_cost_usd: f64 = iterations.iter().map(|i| i.cost_usd).sum();

    let interaction_id = uuid::Uuid::new_v4().to_string();
    let kind_str = match kind {
        InteractionKind::Compose => "compose",
        InteractionKind::Analyze => "analyze",
    };

    let iterations_json = serde_json::to_value(&iterations)
        .map_err(|e| format!("Failed to serialize iterations: {}", e))?;
    let task_input_capped = truncate_chars(task_input, MAX_TASK_INPUT_CHARS);
    let final_response_text = iterations.last().and_then(|i| i.response_content.clone());

    let query = format!(
        "CREATE kanban_card_interaction:`{interaction_id}` CONTENT {{ \
            id: $id, \
            card_id: $card_id, \
            kind: $kind, \
            kanban_agent_id: $kanban_agent_id, \
            provider: $provider, \
            model_id_used: $model_id_used, \
            task_input: $task_input, \
            iterations: $iterations, \
            final_payload_summary: $final_payload_summary, \
            final_response_text: $final_response_text, \
            total_tokens_input: $total_tokens_input, \
            total_tokens_output: $total_tokens_output, \
            total_cost_usd: $total_cost_usd, \
            created_at: time::now() \
        }}"
    );

    let params: Vec<(String, serde_json::Value)> = vec![
        ("id".to_string(), json!(interaction_id.clone())),
        ("card_id".to_string(), json!(validated_card_id)),
        ("kind".to_string(), json!(kind_str)),
        (
            "kanban_agent_id".to_string(),
            json!(kanban_agent_id.to_string()),
        ),
        ("provider".to_string(), json!(llm.provider.clone())),
        ("model_id_used".to_string(), json!(llm.model.clone())),
        ("task_input".to_string(), json!(task_input_capped)),
        ("iterations".to_string(), iterations_json),
        (
            "final_payload_summary".to_string(),
            json!(final_payload_summary.map(|s| s.to_string())),
        ),
        (
            "final_response_text".to_string(),
            json!(final_response_text),
        ),
        (
            "total_tokens_input".to_string(),
            json!(total_tokens_input as i64),
        ),
        (
            "total_tokens_output".to_string(),
            json!(total_tokens_output as i64),
        ),
        ("total_cost_usd".to_string(), json!(total_cost_usd)),
    ];

    db.execute_with_params(&query, params)
        .await
        .map_err(|e| format!("Failed to insert kanban_card_interaction: {}", e))?;

    Ok(interaction_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    /// Inserts a minimal interaction row via raw query for the load test.
    /// Lives in tests because production writes happen inside compose_card and
    /// kanban_analyzer ; this command is read-only and has no own write path.
    async fn insert_dummy_interaction(
        db: &Arc<DBClient>,
        id: &str,
        card_id: &str,
        kind: &str,
    ) -> Result<(), String> {
        let query = format!(
            "CREATE kanban_card_interaction:`{id}` CONTENT {{ \
                id: '{id}', \
                card_id: '{card_id}', \
                kind: '{kind}', \
                kanban_agent_id: 'agent-1', \
                provider: 'mistral', \
                model_id_used: 'mistral-medium-2505', \
                task_input: 'demo', \
                iterations: [], \
                total_tokens_input: 0, \
                total_tokens_output: 0, \
                total_cost_usd: 0.0, \
                created_at: time::now() \
            }}"
        );
        db.execute(&query)
            .await
            .map_err(|e| format!("insert failed: {}", e))
    }

    #[tokio::test]
    async fn load_returns_empty_for_unknown_card() {
        let (state, _guard) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let out = load_card_interactions_core(&state.db, &card_id)
            .await
            .unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn load_returns_inserted_row() {
        let (state, _guard) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let interaction_id = uuid::Uuid::new_v4().to_string();
        insert_dummy_interaction(&state.db, &interaction_id, &card_id, "compose")
            .await
            .unwrap();

        let out = load_card_interactions_core(&state.db, &card_id)
            .await
            .unwrap();
        assert_eq!(out.len(), 1);
        let it = &out[0];
        assert_eq!(it.card_id, card_id);
        assert_eq!(
            it.kind,
            crate::models::kanban_card_interaction::InteractionKind::Compose
        );
        assert_eq!(it.kanban_agent_id, "agent-1");
        assert!(it.iterations.is_empty());
    }

    #[tokio::test]
    async fn load_orders_by_created_at_ascending() {
        let (state, _guard) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let first = uuid::Uuid::new_v4().to_string();
        let second = uuid::Uuid::new_v4().to_string();
        insert_dummy_interaction(&state.db, &first, &card_id, "compose")
            .await
            .unwrap();
        // Force chronological gap so SurrealDB orders them deterministically.
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        insert_dummy_interaction(&state.db, &second, &card_id, "analyze")
            .await
            .unwrap();

        let out = load_card_interactions_core(&state.db, &card_id)
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(
            out[0].kind,
            crate::models::kanban_card_interaction::InteractionKind::Compose
        );
        assert_eq!(
            out[1].kind,
            crate::models::kanban_card_interaction::InteractionKind::Analyze
        );
    }

    #[tokio::test]
    async fn load_rejects_invalid_uuid() {
        let (state, _guard) = setup_test_state().await;
        let err = load_card_interactions_core(&state.db, "not-a-uuid")
            .await
            .unwrap_err();
        assert!(err.contains("card_id"), "got: {}", err);
    }
}
