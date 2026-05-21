// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! SkillManagerTool — list/read/create/update skills.
//!
//! Allowlist semantics:
//! - empty allowlist → unrestricted access (meta-agent)
//! - non-empty → list/read/update restricted to that set
//! - `create_skill` is always allowed AND auto-extends `agent.skills` so the
//!   creating agent can immediately read what it just wrote.

use crate::commands::skill_version::snapshot_skill_version_core;
use crate::db::DBClient;
use crate::models::skill::{
    validate_skill_content, validate_skill_description, validate_skill_name, SkillCategory,
};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock, Mutex};
use tracing::{debug, info};

fn parse_skill_category(s: &str) -> ToolResult<SkillCategory> {
    serde_json::from_value(json!(s))
        .map_err(|_| ToolError::ValidationFailed(format!("Unknown category: {}", s)))
}

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| ToolDefinition {
    id: "SkillManagerTool".to_string(),
    name: "SkillManager".to_string(),
    summary: "List, read, create or update skill documents (no delete)".to_string(),
    description: ToolDescriptionBuilder::new(
        "Manages skills (reusable markdown instructions): list, read, create, update.",
    )
    .use_when(&[
        "The user asks to author or refine a skill document",
        "You need to read a skill that is in your assigned list",
    ])
    .do_not_use(&[
        "Reading a skill not in your assigned list (you will receive a permission error)",
        "Operating on prompts (use PromptManagerTool)",
    ])
    .operations(&[
        ("list_skills", "List available skills, optional `category` filter"),
        ("read_skill", "Read a skill by name (`name` field)"),
        (
            "create_skill",
            "Create with {name, description, content, category?}; auto-grants read access",
        ),
        (
            "update_skill",
            "Update by skill_id; requires `edit_summary` (max 500 chars)",
        ),
    ])
    .examples(&[
        json!({"operation": "list_skills"}),
        json!({"operation": "read_skill", "name": "coding-standards"}),
        json!({
            "operation": "create_skill",
            "name": "review-checklist",
            "description": "Pre-merge gate",
            "content": "# Checklist\n- ...",
            "category": "workflow"
        }),
        json!({
            "operation": "update_skill",
            "skill_id": "<uuid>",
            "content": "...",
            "edit_summary": "Added section on perf budget"
        }),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "operation": {"type": "string", "enum": ["list_skills", "read_skill", "create_skill", "update_skill"]},
            "skill_id": {"type": "string"},
            "name": {"type": "string"},
            "description": {"type": "string"},
            "content": {"type": "string"},
            "category": {"type": "string"},
            "edit_summary": {"type": "string"},
        },
        "required": ["operation"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
});

pub struct SkillManagerTool {
    db: Arc<DBClient>,
    agent_id: String,
    /// Allowlist of skill *names*. Empty = unrestricted.
    /// Mutexed because `create_skill` extends it at runtime.
    allowed_skills: Mutex<Vec<String>>,
}

impl SkillManagerTool {
    pub fn new(db: Arc<DBClient>, agent_id: String, allowed_skills: Vec<String>) -> Self {
        Self {
            db,
            agent_id,
            allowed_skills: Mutex::new(allowed_skills),
        }
    }

    fn edited_by(&self) -> String {
        format!("agent:{}", self.agent_id)
    }

    fn is_allowed(&self, name: &str) -> bool {
        let lock = self.allowed_skills.lock().expect("allowlist lock");
        lock.is_empty() || lock.iter().any(|s| s == name)
    }

    fn validate_edit_summary(s: &str) -> ToolResult<String> {
        let t = s.trim();
        if t.is_empty() {
            return Err(ToolError::ValidationFailed(
                "edit_summary is required".to_string(),
            ));
        }
        if t.len() > 500 {
            return Err(ToolError::ValidationFailed(
                "edit_summary exceeds 500 chars".to_string(),
            ));
        }
        if t.chars().any(|c| c.is_control() && c != ' ') {
            return Err(ToolError::ValidationFailed(
                "edit_summary contains control characters".to_string(),
            ));
        }
        Ok(t.to_string())
    }

    async fn list_skills(&self, category: Option<&str>) -> ToolResult<Value> {
        let mut conds: Vec<&str> = vec!["enabled = true"];
        let mut params: Vec<(String, Value)> = Vec::new();
        if let Some(c) = category.filter(|c| !c.trim().is_empty()) {
            conds.push("category = $cat");
            params.push(("cat".to_string(), json!(c.trim())));
        }
        let q = format!(
            "SELECT meta::id(id) AS id, name, description, category, \
             string::len(content) AS content_length, updated_at \
             FROM skill WHERE {} ORDER BY name ASC LIMIT 200",
            conds.join(" AND ")
        );
        let rows: Vec<Value> = self
            .db
            .query_json_with_params(&q, params)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_skills: {}", e)))?;

        // Filter to allowlist when non-empty.
        let filtered: Vec<Value> = {
            let lock = self.allowed_skills.lock().expect("allowlist lock");
            if lock.is_empty() {
                rows
            } else {
                rows.into_iter()
                    .filter(|r| {
                        r["name"]
                            .as_str()
                            .map(|n| lock.iter().any(|s| s == n))
                            .unwrap_or(false)
                    })
                    .collect()
            }
        };
        Ok(json!({"success": true, "skills": filtered}))
    }

    async fn read_skill(&self, name: &str) -> ToolResult<Value> {
        if !self.is_allowed(name) {
            return Err(ToolError::PermissionDenied(format!(
                "Skill '{}' not in your allowlist",
                name
            )));
        }
        let q = "SELECT meta::id(id) AS id, name, description, category, content, enabled, \
                 updated_at FROM skill WHERE name = $n AND enabled = true";
        let rows = self
            .db
            .query_json_with_params(q, vec![("n".to_string(), json!(name))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("read_skill: {}", e)))?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Skill '{}'", name)))?;
        Ok(json!({"success": true, "skill": row}))
    }

    async fn create_skill(&self, input: &Value) -> ToolResult<Value> {
        let name = validate_skill_name(
            input["name"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("name required".to_string()))?,
        )
        .map_err(ToolError::ValidationFailed)?;
        let description = validate_skill_description(input["description"].as_str().unwrap_or(""))
            .map_err(ToolError::ValidationFailed)?;
        let content = validate_skill_content(
            input["content"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("content required".to_string()))?,
        )
        .map_err(ToolError::ValidationFailed)?;
        let category = parse_skill_category(input["category"].as_str().unwrap_or("custom"))?;

        let id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE skill:`{}` CONTENT {{
                name: $name, description: $description, category: $category,
                content: $content, enabled: true,
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
                ],
            )
            .await
            .map_err(|e| ToolError::DatabaseError(format!("create_skill: {}", e)))?;

        // Auto-grant: extend the agent's skills array AND the in-memory allowlist
        // so the calling agent can immediately read what it created.
        self.grant_skill_to_agent(&name).await?;
        {
            let mut lock = self.allowed_skills.lock().expect("allowlist lock");
            if !lock.iter().any(|s| s == &name) {
                lock.push(name.clone());
            }
        }

        info!(skill_id = %id, skill_name = %name, "Skill created by agent");
        Ok(json!({"success": true, "skill_id": id, "name": name}))
    }

    async fn grant_skill_to_agent(&self, skill_name: &str) -> ToolResult<()> {
        let agent_id = validate_uuid_field(&self.agent_id, "agent_id")
            .map_err(ToolError::ExecutionFailed)?;
        let q = format!(
            "UPDATE agent:`{}` SET skills = array::union(skills ?? [], [$name])",
            agent_id
        );
        self.db
            .execute_with_params(&q, vec![("name".to_string(), json!(skill_name))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("grant_skill_to_agent: {}", e)))?;
        Ok(())
    }

    async fn update_skill(&self, input: &Value) -> ToolResult<Value> {
        let skill_id = validate_uuid_field(
            input["skill_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("skill_id required".to_string()))?,
            "skill_id",
        )
        .map_err(ToolError::InvalidInput)?;
        let edit_summary = Self::validate_edit_summary(input["edit_summary"].as_str().unwrap_or(""))?;

        // Allowlist check: load the skill name from DB.
        let name_q = format!("SELECT name FROM skill:`{}`", skill_id);
        let name_rows = self
            .db
            .query_json(&name_q)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let current_name = name_rows
            .into_iter()
            .next()
            .and_then(|r| r["name"].as_str().map(String::from))
            .ok_or_else(|| ToolError::NotFound(format!("Skill {}", skill_id)))?;
        if !self.is_allowed(&current_name) {
            return Err(ToolError::PermissionDenied(format!(
                "Skill '{}' not in your allowlist",
                current_name
            )));
        }

        let mut sets: Vec<String> = Vec::new();
        if let Some(n) = input["name"].as_str() {
            let v = validate_skill_name(n).map_err(ToolError::ValidationFailed)?;
            sets.push(format!(
                "name = {}",
                serialize_for_query(&v, "name").map_err(ToolError::ExecutionFailed)?
            ));
        }
        if let Some(d) = input["description"].as_str() {
            let v = validate_skill_description(d).map_err(ToolError::ValidationFailed)?;
            sets.push(format!(
                "description = {}",
                serialize_for_query(&v, "description").map_err(ToolError::ExecutionFailed)?
            ));
        }
        if let Some(c) = input["category"].as_str() {
            let cat = parse_skill_category(c)?;
            sets.push(format!(
                "category = {}",
                serialize_for_query(&cat.to_string(), "category")
                    .map_err(ToolError::ExecutionFailed)?
            ));
        }
        if let Some(content) = input["content"].as_str() {
            let v = validate_skill_content(content).map_err(ToolError::ValidationFailed)?;
            sets.push(format!(
                "content = {}",
                serialize_for_query(&v, "content").map_err(ToolError::ExecutionFailed)?
            ));
        }
        if sets.is_empty() {
            return Err(ToolError::InvalidInput(
                "At least one field to update is required".to_string(),
            ));
        }

        snapshot_skill_version_core(&self.db, &skill_id, &self.edited_by(), Some(edit_summary))
            .await
            .map_err(ToolError::DatabaseError)?;

        sets.push("updated_at = time::now()".to_string());
        let q = format!("UPDATE skill:`{}` SET {}", skill_id, sets.join(", "));
        self.db
            .execute(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("update_skill: {}", e)))?;
        Ok(json!({"success": true, "skill_id": skill_id}))
    }
}

#[async_trait]
impl Tool for SkillManagerTool {
    fn id(&self) -> &str {
        "SkillManagerTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let op = input["operation"].as_str().unwrap_or("");
        debug!(operation = %op, "SkillManagerTool execute");
        match op {
            "list_skills" => self.list_skills(input["category"].as_str()).await,
            "read_skill" => {
                let name = input["name"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("name required".to_string()))?;
                self.read_skill(name).await
            }
            "create_skill" => self.create_skill(&input).await,
            "update_skill" => self.update_skill(&input).await,
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
            "list_skills" | "read_skill" | "create_skill" | "update_skill" => Ok(()),
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

    fn make_tool(db: Arc<DBClient>, agent_id: &str, allowed: Vec<&str>) -> SkillManagerTool {
        SkillManagerTool::new(
            db,
            agent_id.to_string(),
            allowed.into_iter().map(String::from).collect(),
        )
    }

    #[tokio::test]
    async fn test_allowlist_empty_allows_all() {
        let (state, _g) = setup_test_state().await;
        let tool = make_tool(state.db.clone(), "agent-x", vec![]);
        assert!(tool.is_allowed("anything"));
    }

    #[test]
    fn test_edit_summary_required() {
        assert!(SkillManagerTool::validate_edit_summary("").is_err());
        assert!(SkillManagerTool::validate_edit_summary("ok").is_ok());
    }

    #[tokio::test]
    async fn test_create_skill_auto_adds_to_allowlist() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        // Create an agent so the UPDATE has a target.
        let cq = format!(
            "CREATE agent:`{}` CONTENT {{
                name: 'AgentX', description: '', system_prompt: 's',
                model_id: 'm', tools: [], skills: [], folders: [],
                created_at: time::now(), updated_at: time::now()
            }}",
            agent_id
        );
        state.db.execute(&cq).await.unwrap();

        let tool = make_tool(state.db.clone(), &agent_id, vec!["existing"]);
        let res = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "new-skill",
                "description": "auto-added",
                "content": "# x",
                "category": "custom"
            }))
            .await
            .unwrap();
        assert_eq!(res["success"], true);
        // Allowlist now includes the new skill.
        assert!(tool.is_allowed("new-skill"));
        // Existing entry preserved.
        assert!(tool.is_allowed("existing"));
    }

    #[tokio::test]
    async fn test_read_skill_respects_allowlist() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let tool = make_tool(state.db.clone(), &agent_id, vec!["only-this"]);
        let err = tool
            .execute(json!({"operation": "read_skill", "name": "forbidden"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }
}
