// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Persistance des appels LLM meta de l'agent Kanban (compose / analyze).
//!
//! Chaque interaction correspond a un tool_loop execute pendant une operation
//! meta (composition ou analyse d'une carte). Stocke prompt, iterations
//! (reasoning + tool calls), tokens et cout pour affichage dans le
//! `KanbanCardReportViewer`.

use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type d'operation meta sur une carte Kanban.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InteractionKind {
    /// Composition initiale : description utilisateur -> KanbanCardCreate.
    Compose,
    /// Analyse post-execution : rapport workflow -> verdict (approve/reject/needs_improvement).
    Analyze,
}

/// Appel d'un outil (local ou MCP) effectue pendant une iteration LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractionToolCall {
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_server: Option<String>,
    /// JSON-stringifie pour stabilite SCHEMAFULL (objets dynamiques).
    pub input_json: String,
    pub output_json: String,
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default = "default_true")]
    pub success: bool,
}

fn default_true() -> bool {
    true
}

/// Une iteration du tool_loop : appel LLM + ses tool calls + tokens / cout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InteractionIteration {
    pub iteration_index: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_content: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<InteractionToolCall>,
    #[serde(default)]
    pub tokens_input: u64,
    #[serde(default)]
    pub tokens_output: u64,
    #[serde(default)]
    pub cached_tokens: u64,
    #[serde(default)]
    pub cost_usd: f64,
    #[serde(default)]
    pub duration_ms: u64,
}

/// Une interaction meta complete : une operation compose ou analyze.
///
/// Persistee dans la table `kanban_card_interaction` apres chaque execution
/// reussie du tool_loop par `compose_card.rs` ou `kanban_analyzer.rs`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanCardInteraction {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    pub card_id: String,
    pub kind: InteractionKind,
    pub kanban_agent_id: String,
    pub provider: String,
    pub model_id_used: String,
    pub task_input: String,
    #[serde(default)]
    pub iterations: Vec<InteractionIteration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_payload_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_response_text: Option<String>,
    #[serde(default)]
    pub total_tokens_input: u64,
    #[serde(default)]
    pub total_tokens_output: u64,
    #[serde(default)]
    pub total_cost_usd: f64,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_serializes_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&InteractionKind::Compose).unwrap(),
            "\"compose\""
        );
        assert_eq!(
            serde_json::to_string(&InteractionKind::Analyze).unwrap(),
            "\"analyze\""
        );
    }

    #[test]
    fn kind_deserializes_from_lowercase() {
        let parsed: InteractionKind = serde_json::from_str("\"analyze\"").unwrap();
        assert_eq!(parsed, InteractionKind::Analyze);
    }

    #[test]
    fn tool_call_defaults_success_true_when_absent() {
        let json = r#"{
            "tool_name": "ListAgents",
            "input_json": "{}",
            "output_json": "[]"
        }"#;
        let call: InteractionToolCall = serde_json::from_str(json).unwrap();
        assert!(call.success);
        assert_eq!(call.duration_ms, 0);
        assert!(call.mcp_server.is_none());
    }

    #[test]
    fn iteration_defaults_numeric_fields_to_zero() {
        let json = r#"{"iteration_index": 0}"#;
        let it: InteractionIteration = serde_json::from_str(json).unwrap();
        assert_eq!(it.tokens_input, 0);
        assert_eq!(it.tokens_output, 0);
        assert_eq!(it.cost_usd, 0.0);
        assert!(it.tool_calls.is_empty());
        assert!(it.reasoning.is_none());
    }

    #[test]
    fn interaction_roundtrip_preserves_data() {
        let original = KanbanCardInteraction {
            id: "abc-123".to_string(),
            card_id: "card-456".to_string(),
            kind: InteractionKind::Compose,
            kanban_agent_id: "agent-1".to_string(),
            provider: "mistral".to_string(),
            model_id_used: "mistral-medium-2505".to_string(),
            task_input: "Compose a card for X".to_string(),
            iterations: vec![InteractionIteration {
                iteration_index: 0,
                reasoning: Some("thinking".to_string()),
                response_content: Some("done".to_string()),
                tool_calls: vec![InteractionToolCall {
                    tool_name: "SubmitComposedCard".to_string(),
                    mcp_server: None,
                    input_json: "{\"title\":\"x\"}".to_string(),
                    output_json: "{\"success\":true}".to_string(),
                    duration_ms: 42,
                    success: true,
                }],
                tokens_input: 100,
                tokens_output: 50,
                cached_tokens: 10,
                cost_usd: 0.0015,
                duration_ms: 1200,
            }],
            final_payload_summary: Some("title: X".to_string()),
            final_response_text: Some("rationale".to_string()),
            total_tokens_input: 100,
            total_tokens_output: 50,
            total_cost_usd: 0.0015,
            created_at: chrono::DateTime::parse_from_rfc3339("2026-05-21T10:00:00Z")
                .unwrap()
                .with_timezone(&Utc),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: KanbanCardInteraction = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, original.id);
        assert_eq!(parsed.kind, original.kind);
        assert_eq!(parsed.iterations.len(), 1);
        assert_eq!(
            parsed.iterations[0].tool_calls[0].tool_name,
            "SubmitComposedCard"
        );
        assert_eq!(parsed.total_cost_usd, original.total_cost_usd);
    }
}
