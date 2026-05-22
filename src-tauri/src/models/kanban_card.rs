// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Kanban card model: a unit of delegated work tracked through 4 columns.

use super::agent::deserialize_explicit_option;
use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// UI column for kanban cards.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KanbanColumn {
    Todo,
    Doing,
    Review,
    Done,
}

impl KanbanColumn {
    pub fn as_str(&self) -> &'static str {
        match self {
            KanbanColumn::Todo => "todo",
            KanbanColumn::Doing => "doing",
            KanbanColumn::Review => "review",
            KanbanColumn::Done => "done",
        }
    }
}

/// Business status of a kanban card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KanbanCardStatus {
    Todo,
    Ready,
    Doing,
    Review,
    Done,
    Failed,
}

/// A kanban card persisted in DB.
///
/// `prompt_id` and `inline_prompt` are mutually exclusive (XOR).
/// `kanban_agent_id` references the Kanban-kind agent that composed the card.
/// `target_agent_id` references the permanent agent that will run the workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanCard {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    pub title: String,
    pub description: String,
    pub kanban_agent_id: String,
    pub target_agent_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_prompt: Option<String>,
    /// JSON-stringified `Record<String, String>` (ERR_SURREAL_001 dynamic keys).
    pub variables: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_folder_id: Option<String>,
    pub status: KanbanCardStatus,
    pub column: KanbanColumn,
    #[serde(default)]
    pub column_order: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_summary: Option<String>,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

/// Payload to create a new kanban card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanCardCreate {
    /// Optional pre-generated card id. Set by `compose_card` so the
    /// associated `kanban_card_interaction` row can be linked to the card
    /// before it is actually persisted. When `None`, `create_kanban_card_core`
    /// generates a fresh UUID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub kanban_agent_id: String,
    pub target_agent_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inline_prompt: Option<String>,
    /// JSON-stringified variables map. Empty `{}` when no variables.
    #[serde(default = "default_empty_object_json")]
    pub variables: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_folder_id: Option<String>,
}

fn default_empty_object_json() -> String {
    "{}".to_string()
}

/// PATCH payload (all fields optional; tri-state where clearing is meaningful).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KanbanCardUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Required field — cannot be cleared, only swapped to another agent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_agent_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_explicit_option"
    )]
    pub prompt_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_explicit_option"
    )]
    pub inline_prompt: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_explicit_option"
    )]
    pub target_folder_id: Option<Option<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_serialization() {
        assert_eq!(
            serde_json::to_string(&KanbanColumn::Doing).unwrap(),
            "\"doing\""
        );
    }

    #[test]
    fn test_status_serialization() {
        assert_eq!(
            serde_json::to_string(&KanbanCardStatus::Ready).unwrap(),
            "\"ready\""
        );
    }

    #[test]
    fn test_card_create_default_variables() {
        let json = r#"{"title":"T","kanban_agent_id":"k","target_agent_id":"t"}"#;
        let create: KanbanCardCreate = serde_json::from_str(json).unwrap();
        assert_eq!(create.variables, "{}");
    }

    #[test]
    fn test_update_tri_state_target_folder_clear() {
        let json = r#"{"target_folder_id":null}"#;
        let update: KanbanCardUpdate = serde_json::from_str(json).unwrap();
        // tri-state: explicit null -> Some(None)
        assert!(matches!(update.target_folder_id, Some(None)));
    }

    #[test]
    fn test_update_tri_state_target_folder_absent() {
        let json = r#"{}"#;
        let update: KanbanCardUpdate = serde_json::from_str(json).unwrap();
        // absent -> None
        assert!(update.target_folder_id.is_none());
    }

    #[test]
    fn test_update_tri_state_target_folder_set() {
        let json = r#"{"target_folder_id":"fld_1"}"#;
        let update: KanbanCardUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.target_folder_id, Some(Some("fld_1".to_string())));
    }
}
