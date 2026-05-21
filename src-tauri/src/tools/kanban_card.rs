// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! KanbanCardTool — list/get/create/update kanban cards from a Kanban-kind agent.
//!
//! The Kanban agent is the author of every card it creates. Update is intentionally
//! limited: it does not touch `workflow_id`, `status` or `column` — those are
//! managed by the scheduler and the feedback loop.

use crate::commands::kanban_card::{
    create_kanban_card_core, get_kanban_card_core, list_kanban_cards_core, update_kanban_card_core,
};
use crate::db::DBClient;
use crate::models::prompt::PromptVariable;
use crate::models::{KanbanCardCreate, KanbanCardUpdate};
use crate::security::validate_uuid_field;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info};

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| ToolDefinition {
    id: "KanbanCardTool".to_string(),
    name: "KanbanCard".to_string(),
    summary: "List, read, create or update kanban cards (no delete, no column move)".to_string(),
    description: ToolDescriptionBuilder::new(
        "Manages kanban cards. You are the Kanban agent: cards you create are authored by you.",
    )
    .use_when(&[
        "The user gives you a demand and you have composed a card with title + description + reusable prompt",
        "You want to read an existing card to inspect its variables and target agent",
        "You want to refine an existing card (title, description, variables, target folder)",
    ])
    .do_not_use(&[
        "Starting the workflow yourself — the scheduler does it when the card moves to Ready",
        "Moving a card between columns — that's a user gesture (drag & drop)",
        "Deleting a card — not exposed",
    ])
    .operations(&[
        ("list_kanban_cards", "List cards; optional `kanban_agent_id` filter"),
        ("get_kanban_card", "Get one card by `card_id`"),
        (
            "create_kanban_card",
            "Create with {title, description, target_agent_id, (prompt_id XOR inline_prompt), variables, target_folder_id?}",
        ),
        (
            "update_kanban_card",
            "Update {title?, description?, prompt_id? (or null), inline_prompt? (or null), variables?, target_folder_id?}",
        ),
    ])
    .examples(&[
        json!({"operation": "list_kanban_cards"}),
        json!({
            "operation": "create_kanban_card",
            "title": "Weekly digest",
            "description": "Summarize the week's PRs",
            "target_agent_id": "<agent-uuid>",
            "prompt_id": "<prompt-uuid>",
            "variables": {"week": "21"}
        }),
        json!({
            "operation": "update_kanban_card",
            "card_id": "<uuid>",
            "variables": {"week": "22"}
        }),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "operation": {"type": "string", "enum": ["list_kanban_cards", "get_kanban_card", "create_kanban_card", "update_kanban_card"]},
            "card_id": {"type": "string"},
            "kanban_agent_id": {"type": "string"},
            "title": {"type": "string"},
            "description": {"type": "string"},
            "target_agent_id": {"type": "string"},
            "prompt_id": {"type": ["string", "null"]},
            "inline_prompt": {"type": ["string", "null"]},
            "variables": {"type": ["object", "string"]},
            "target_folder_id": {"type": ["string", "null"]},
        },
        "required": ["operation"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
});

pub struct KanbanCardTool {
    db: Arc<DBClient>,
    kanban_agent_id: String,
}

impl KanbanCardTool {
    pub fn new(db: Arc<DBClient>, kanban_agent_id: String) -> Self {
        Self {
            db,
            kanban_agent_id,
        }
    }

    /// Reads the prompt and asserts that every variable without `default_value`
    /// is provided in `supplied`. Returns the names that are missing.
    async fn check_prompt_variables(
        &self,
        prompt_id: &str,
        supplied: &serde_json::Map<String, Value>,
    ) -> ToolResult<()> {
        let pid = validate_uuid_field(prompt_id, "prompt_id").map_err(ToolError::InvalidInput)?;
        let q = format!("SELECT variables FROM prompt:`{}`", pid);
        let rows = self
            .db
            .query_json(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Prompt {}", pid)))?;
        let vars: Vec<PromptVariable> =
            serde_json::from_value(row["variables"].clone()).unwrap_or_default();
        let missing: Vec<&str> = vars
            .iter()
            .filter(|v| v.default_value.is_none() && !supplied.contains_key(&v.name))
            .map(|v| v.name.as_str())
            .collect();
        if !missing.is_empty() {
            return Err(ToolError::ValidationFailed(format!(
                "Missing prompt variables (no default_value provided): {}",
                missing.join(", ")
            )));
        }
        Ok(())
    }

    /// Normalises the `variables` input which can be either an object (preferred)
    /// or a pre-stringified JSON object (for round-tripping).
    fn variables_to_json_string(input: &Value) -> ToolResult<(String, serde_json::Map<String, Value>)> {
        if input.is_null() {
            return Ok(("{}".to_string(), serde_json::Map::new()));
        }
        if let Some(s) = input.as_str() {
            let parsed: Value = serde_json::from_str(s)
                .map_err(|e| ToolError::InvalidInput(format!("variables string not valid JSON: {}", e)))?;
            let map = parsed
                .as_object()
                .ok_or_else(|| ToolError::InvalidInput("variables must be an object".to_string()))?
                .clone();
            return Ok((s.to_string(), map));
        }
        let map = input
            .as_object()
            .ok_or_else(|| ToolError::InvalidInput("variables must be an object".to_string()))?
            .clone();
        let s = serde_json::to_string(&map)
            .map_err(|e| ToolError::ExecutionFailed(format!("variables serialize: {}", e)))?;
        Ok((s, map))
    }

    async fn list_cards(&self, kanban_agent_id: Option<&str>) -> ToolResult<Value> {
        let filter = kanban_agent_id
            .filter(|s| !s.trim().is_empty())
            .map(String::from);
        let cards = list_kanban_cards_core(&self.db, filter)
            .await
            .map_err(ToolError::DatabaseError)?;
        Ok(json!({"success": true, "cards": cards}))
    }

    async fn get_card(&self, card_id: &str) -> ToolResult<Value> {
        let card = get_kanban_card_core(&self.db, card_id)
            .await
            .map_err(|e| {
                if e.contains("not found") || e.contains("Not found") {
                    ToolError::NotFound(format!("Card {}", card_id))
                } else {
                    ToolError::DatabaseError(e)
                }
            })?;
        Ok(json!({"success": true, "card": card}))
    }

    async fn create_card(&self, input: &Value) -> ToolResult<Value> {
        let title = input["title"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("title required".to_string()))?
            .to_string();
        let description = input["description"].as_str().unwrap_or("").to_string();
        let target_agent_id = input["target_agent_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("target_agent_id required".to_string()))?
            .to_string();
        let prompt_id = input["prompt_id"].as_str().map(String::from);
        let inline_prompt = input["inline_prompt"].as_str().map(String::from);
        let target_folder_id = input["target_folder_id"].as_str().map(String::from);
        let (variables_json, supplied_map) = Self::variables_to_json_string(&input["variables"])?;

        // Variable completeness check — only when a stored prompt is referenced.
        if let Some(ref pid) = prompt_id {
            self.check_prompt_variables(pid, &supplied_map).await?;
        }

        let card = create_kanban_card_core(
            &self.db,
            KanbanCardCreate {
                title,
                description,
                kanban_agent_id: self.kanban_agent_id.clone(),
                target_agent_id,
                prompt_id,
                inline_prompt,
                variables: variables_json,
                target_folder_id,
            },
        )
        .await
        .map_err(|e| {
            if e.starts_with("title") || e.starts_with("description") || e.starts_with("variables")
                || e.contains("mutually exclusive") || e.contains("required")
            {
                ToolError::ValidationFailed(e)
            } else {
                ToolError::DatabaseError(e)
            }
        })?;
        info!(card_id = %card.id, "Kanban card created");
        Ok(json!({"success": true, "card": card}))
    }

    async fn update_card(&self, input: &Value) -> ToolResult<Value> {
        let card_id = input["card_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("card_id required".to_string()))?;
        let mut update = KanbanCardUpdate::default();
        if let Some(t) = input["title"].as_str() {
            update.title = Some(t.to_string());
        }
        if let Some(d) = input["description"].as_str() {
            update.description = Some(d.to_string());
        }
        if input.get("prompt_id").is_some() {
            update.prompt_id = Some(input["prompt_id"].as_str().map(String::from));
        }
        if input.get("inline_prompt").is_some() {
            update.inline_prompt = Some(input["inline_prompt"].as_str().map(String::from));
        }
        if input.get("variables").is_some() {
            let (s, _) = Self::variables_to_json_string(&input["variables"])?;
            update.variables = Some(s);
        }
        if input.get("target_folder_id").is_some() {
            update.target_folder_id = Some(input["target_folder_id"].as_str().map(String::from));
        }

        let card = update_kanban_card_core(&self.db, card_id, update)
            .await
            .map_err(ToolError::DatabaseError)?;
        Ok(json!({"success": true, "card": card}))
    }
}

#[async_trait]
impl Tool for KanbanCardTool {
    fn id(&self) -> &str {
        "KanbanCardTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let op = input["operation"].as_str().unwrap_or("");
        debug!(operation = %op, "KanbanCardTool execute");
        match op {
            "list_kanban_cards" => self.list_cards(input["kanban_agent_id"].as_str()).await,
            "get_kanban_card" => {
                let id = input["card_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("card_id required".to_string()))?;
                self.get_card(id).await
            }
            "create_kanban_card" => self.create_card(&input).await,
            "update_kanban_card" => self.update_card(&input).await,
            other => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                other
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        let op = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("operation required".to_string()))?;
        match op {
            "list_kanban_cards"
            | "get_kanban_card"
            | "create_kanban_card"
            | "update_kanban_card" => Ok(()),
            other => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                other
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    async fn seed_prompt(db: &Arc<DBClient>, vars: Value) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE prompt:`{}` CONTENT {{
                name: 'p', description: '', category: 'custom', content: 'x',
                variables: $vars, created_at: time::now(), updated_at: time::now()
            }}",
            id
        );
        db.execute_with_params(&q, vec![("vars".to_string(), vars)])
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn test_create_card_rejects_missing_variable() {
        let (state, _g) = setup_test_state().await;
        let prompt_id = seed_prompt(
            &state.db,
            json!([{"name": "topic"}, {"name": "audience", "defaultValue": "all"}]),
        )
        .await;
        let kanban_agent_id = uuid::Uuid::new_v4().to_string();
        let target_agent_id = uuid::Uuid::new_v4().to_string();
        let tool = KanbanCardTool::new(state.db.clone(), kanban_agent_id);
        let err = tool
            .execute(json!({
                "operation": "create_kanban_card",
                "title": "x",
                "description": "",
                "target_agent_id": target_agent_id,
                "prompt_id": prompt_id,
                "variables": {}
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
        let msg = err.to_string();
        assert!(msg.contains("topic"), "expected `topic` in error: {}", msg);
        assert!(!msg.contains("audience"), "default-valued var should not be flagged");
    }

    #[tokio::test]
    async fn test_create_card_accepts_when_all_vars_provided() {
        let (state, _g) = setup_test_state().await;
        let prompt_id = seed_prompt(&state.db, json!([{"name": "topic"}])).await;
        let kanban_agent_id = uuid::Uuid::new_v4().to_string();
        let target_agent_id = uuid::Uuid::new_v4().to_string();
        let tool = KanbanCardTool::new(state.db.clone(), kanban_agent_id);
        let res = tool
            .execute(json!({
                "operation": "create_kanban_card",
                "title": "x",
                "description": "",
                "target_agent_id": target_agent_id,
                "prompt_id": prompt_id,
                "variables": {"topic": "rust"}
            }))
            .await
            .unwrap();
        assert_eq!(res["success"], true);
    }

    #[test]
    fn test_variables_object_to_json() {
        let (s, m) = KanbanCardTool::variables_to_json_string(&json!({"a": "b"})).unwrap();
        assert_eq!(s, "{\"a\":\"b\"}");
        assert_eq!(m.get("a").unwrap(), &json!("b"));
    }

    #[test]
    fn test_variables_string_passthrough() {
        let (s, m) =
            KanbanCardTool::variables_to_json_string(&json!("{\"a\":\"b\"}")).unwrap();
        assert_eq!(s, "{\"a\":\"b\"}");
        assert_eq!(m.get("a").unwrap(), &json!("b"));
    }

    #[test]
    fn test_variables_string_rejects_non_object() {
        assert!(KanbanCardTool::variables_to_json_string(&json!("[1,2]")).is_err());
    }
}
