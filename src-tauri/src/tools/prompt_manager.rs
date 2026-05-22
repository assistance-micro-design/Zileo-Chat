// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! PromptManagerTool — allows agents to list/get/create/update prompts.
//!
//! No delete, no restore. Updates require an `edit_summary` (validated at the
//! tool level). All snapshots are taken by the underlying command core.

use crate::commands::prompt_version::snapshot_prompt_version_core;
use crate::db::DBClient;
use crate::models::prompt::{
    Prompt, PromptCategory, MAX_PROMPT_CONTENT_LEN, MAX_PROMPT_DESCRIPTION_LEN, MAX_PROMPT_NAME_LEN,
};
use crate::security::{
    serialize_for_query, validate_edit_summary as validate_edit_summary_core, validate_uuid_field,
};
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info};

fn parse_prompt_category(s: &str) -> ToolResult<PromptCategory> {
    serde_json::from_value(json!(s))
        .map_err(|_| ToolError::ValidationFailed(format!("Unknown category: {}", s)))
}

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| ToolDefinition {
    id: "PromptManagerTool".to_string(),
    name: "PromptManager".to_string(),
    summary: "List, read, create or update prompt templates (no delete)".to_string(),
    description: ToolDescriptionBuilder::new(
        "Manages the prompt library: list, read, create, update. No delete.",
    )
    .use_when(&[
        "The user asks to write a new prompt or improve an existing one",
        "You need to read an existing prompt template to compose a card",
        "You want to enrich the library with a new reusable template",
    ])
    .do_not_use(&[
        "Operating on skills (use SkillManagerTool)",
        "Running a workflow (use WorkflowManagerTool / DelegateTaskTool)",
    ])
    .operations(&[
        (
            "list_prompts",
            "List prompts, optional `query` and `category` filters",
        ),
        ("get_prompt", "Get full prompt by `prompt_id`"),
        (
            "create_prompt",
            "Create with {name, description, category, content}",
        ),
        (
            "update_prompt",
            "Update by prompt_id, requires `edit_summary` (free text, max 500 chars)",
        ),
    ])
    .examples(&[
        json!({"operation": "list_prompts", "category": "coding"}),
        json!({"operation": "get_prompt", "prompt_id": "<uuid>"}),
        json!({
            "operation": "create_prompt",
            "name": "Summarize code",
            "description": "Concise PR summary",
            "category": "coding",
            "content": "Summarize {{diff}}"
        }),
        json!({
            "operation": "update_prompt",
            "prompt_id": "<uuid>",
            "content": "Refined prompt",
            "edit_summary": "Clarified the instruction to focus on edge cases"
        }),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["list_prompts", "get_prompt", "create_prompt", "update_prompt"],
            },
            "prompt_id": {"type": "string"},
            "query": {"type": "string"},
            "category": {"type": "string"},
            "name": {"type": "string"},
            "description": {"type": "string"},
            "content": {"type": "string"},
            "edit_summary": {"type": "string"},
        },
        "required": ["operation"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
});

/// Manages prompts on behalf of an agent. The agent ID is woven into version
/// snapshots so the history shows who changed what.
pub struct PromptManagerTool {
    db: Arc<DBClient>,
    agent_id: String,
}

impl PromptManagerTool {
    pub fn new(db: Arc<DBClient>, agent_id: String) -> Self {
        Self { db, agent_id }
    }

    fn edited_by(&self) -> String {
        format!("agent:{}", self.agent_id)
    }

    fn validate_name(n: &str) -> ToolResult<String> {
        let t = n.trim();
        if t.is_empty() {
            return Err(ToolError::ValidationFailed("name is empty".to_string()));
        }
        if t.len() > MAX_PROMPT_NAME_LEN {
            return Err(ToolError::ValidationFailed(format!(
                "name exceeds {} chars",
                MAX_PROMPT_NAME_LEN
            )));
        }
        Ok(t.to_string())
    }

    fn validate_description(d: &str) -> ToolResult<String> {
        let t = d.trim();
        if t.len() > MAX_PROMPT_DESCRIPTION_LEN {
            return Err(ToolError::ValidationFailed(format!(
                "description exceeds {} chars",
                MAX_PROMPT_DESCRIPTION_LEN
            )));
        }
        Ok(t.to_string())
    }

    fn validate_content(c: &str) -> ToolResult<String> {
        if c.is_empty() {
            return Err(ToolError::ValidationFailed("content is empty".to_string()));
        }
        if c.len() > MAX_PROMPT_CONTENT_LEN {
            return Err(ToolError::ValidationFailed(format!(
                "content exceeds {} chars",
                MAX_PROMPT_CONTENT_LEN
            )));
        }
        Ok(c.to_string())
    }

    /// Thin wrapper around the shared validator. Agents always tag updates,
    /// so `edit_summary` is mandatory here.
    fn validate_edit_summary(s: &str) -> ToolResult<String> {
        validate_edit_summary_core(Some(s), true)
            .map_err(ToolError::ValidationFailed)?
            .ok_or_else(|| ToolError::ValidationFailed("edit_summary is required".to_string()))
    }

    async fn list_prompts(&self, query: Option<&str>, category: Option<&str>) -> ToolResult<Value> {
        let mut conds: Vec<&str> = Vec::new();
        let mut params: Vec<(String, Value)> = Vec::new();
        if let Some(q) = query.filter(|q| !q.trim().is_empty()) {
            conds.push("(string::lowercase(name) CONTAINS $q OR string::lowercase(description) CONTAINS $q)");
            params.push(("q".to_string(), json!(q.trim().to_lowercase())));
        }
        if let Some(c) = category.filter(|c| !c.trim().is_empty()) {
            conds.push("category = $cat");
            params.push(("cat".to_string(), json!(c.trim())));
        }
        let where_clause = if conds.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conds.join(" AND "))
        };
        let q = format!(
            "SELECT meta::id(id) AS id, name, description, category, \
             array::len(variables ?? []) AS variables_count, updated_at FROM prompt {} \
             ORDER BY updated_at DESC LIMIT 100",
            where_clause
        );
        let rows = self
            .db
            .query_json_with_params(&q, params)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_prompts: {}", e)))?;
        Ok(json!({"success": true, "prompts": rows}))
    }

    async fn get_prompt(&self, prompt_id: &str) -> ToolResult<Value> {
        let id = validate_uuid_field(prompt_id, "prompt_id").map_err(ToolError::InvalidInput)?;
        let q = format!(
            "SELECT meta::id(id) AS id, name, description, category, content, variables, \
             created_at, updated_at FROM prompt:`{}`",
            id
        );
        let rows = self
            .db
            .query_json(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("get_prompt: {}", e)))?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Prompt {}", id)))?;
        Ok(json!({"success": true, "prompt": row}))
    }

    async fn create_prompt(&self, input: &Value) -> ToolResult<Value> {
        let name = Self::validate_name(
            input["name"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("name required".to_string()))?,
        )?;
        let description = Self::validate_description(input["description"].as_str().unwrap_or(""))?;
        let content = Self::validate_content(
            input["content"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("content required".to_string()))?,
        )?;
        let category = parse_prompt_category(input["category"].as_str().unwrap_or("custom"))?;

        let id = uuid::Uuid::new_v4().to_string();
        let variables = Prompt::detect_variables(&content);
        let q = format!(
            "CREATE prompt:`{}` CONTENT {{
                name: $name, description: $description, category: $category,
                content: $content, variables: $variables,
                created_at: time::now(), updated_at: time::now()
            }}",
            id
        );
        self.db
            .execute_with_params(
                &q,
                vec![
                    ("name".to_string(), json!(name)),
                    ("description".to_string(), json!(description)),
                    ("category".to_string(), json!(category.to_string())),
                    ("content".to_string(), json!(content)),
                    ("variables".to_string(), json!(variables)),
                ],
            )
            .await
            .map_err(|e| ToolError::DatabaseError(format!("create_prompt: {}", e)))?;
        info!(prompt_id = %id, "Prompt created by agent");
        Ok(json!({"success": true, "prompt_id": id}))
    }

    async fn update_prompt(&self, input: &Value) -> ToolResult<Value> {
        let prompt_id = validate_uuid_field(
            input["prompt_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("prompt_id required".to_string()))?,
            "prompt_id",
        )
        .map_err(ToolError::InvalidInput)?;

        let edit_summary =
            Self::validate_edit_summary(input["edit_summary"].as_str().unwrap_or(""))?;

        let mut sets: Vec<String> = Vec::new();
        if let Some(n) = input["name"].as_str() {
            let v = Self::validate_name(n)?;
            sets.push(format!(
                "name = {}",
                serialize_for_query(&v, "name").map_err(ToolError::ExecutionFailed)?
            ));
        }
        if let Some(d) = input["description"].as_str() {
            let v = Self::validate_description(d)?;
            sets.push(format!(
                "description = {}",
                serialize_for_query(&v, "description").map_err(ToolError::ExecutionFailed)?
            ));
        }
        if let Some(c) = input["category"].as_str() {
            let cat = parse_prompt_category(c)?;
            sets.push(format!(
                "category = {}",
                serialize_for_query(&cat.to_string(), "category")
                    .map_err(ToolError::ExecutionFailed)?
            ));
        }
        if let Some(content) = input["content"].as_str() {
            let v = Self::validate_content(content)?;
            let variables = Prompt::detect_variables(&v);
            sets.push(format!(
                "content = {}",
                serialize_for_query(&v, "content").map_err(ToolError::ExecutionFailed)?
            ));
            sets.push(format!(
                "variables = {}",
                serialize_for_query(&variables, "variables").map_err(ToolError::ExecutionFailed)?
            ));
        }
        if sets.is_empty() {
            return Err(ToolError::InvalidInput(
                "At least one field to update is required".to_string(),
            ));
        }

        // Snapshot BEFORE update — uses the agent-flavoured edited_by so the
        // history shows who authored the change.
        snapshot_prompt_version_core(&self.db, &prompt_id, &self.edited_by(), Some(edit_summary))
            .await
            .map_err(ToolError::DatabaseError)?;

        sets.push("updated_at = time::now()".to_string());
        let q = format!("UPDATE prompt:`{}` SET {}", prompt_id, sets.join(", "));
        self.db
            .execute(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("update_prompt: {}", e)))?;
        Ok(json!({"success": true, "prompt_id": prompt_id}))
    }
}

#[async_trait]
impl Tool for PromptManagerTool {
    fn id(&self) -> &str {
        "PromptManagerTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let op = input["operation"].as_str().unwrap_or("");
        debug!(operation = %op, "PromptManagerTool execute");
        match op {
            "list_prompts" => {
                self.list_prompts(input["query"].as_str(), input["category"].as_str())
                    .await
            }
            "get_prompt" => {
                let id = input["prompt_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("prompt_id required".to_string()))?;
                self.get_prompt(id).await
            }
            "create_prompt" => self.create_prompt(&input).await,
            "update_prompt" => self.update_prompt(&input).await,
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
            "list_prompts" | "get_prompt" | "create_prompt" | "update_prompt" => Ok(()),
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

    #[test]
    fn test_edit_summary_required() {
        assert!(PromptManagerTool::validate_edit_summary("").is_err());
        assert!(PromptManagerTool::validate_edit_summary("   ").is_err());
    }

    #[test]
    fn test_edit_summary_too_long() {
        let big = "x".repeat(501);
        assert!(PromptManagerTool::validate_edit_summary(&big).is_err());
    }

    #[test]
    fn test_edit_summary_rejects_control_chars() {
        assert!(PromptManagerTool::validate_edit_summary("ok\nthere").is_err());
    }

    #[test]
    fn test_edit_summary_accepts_valid() {
        assert_eq!(
            PromptManagerTool::validate_edit_summary("Clarified instruction").unwrap(),
            "Clarified instruction"
        );
    }

    #[test]
    fn test_name_validation() {
        assert!(PromptManagerTool::validate_name("").is_err());
        assert!(PromptManagerTool::validate_name("good").is_ok());
    }
}
