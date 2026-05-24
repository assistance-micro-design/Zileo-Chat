// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! CRUD operations for `SkillManagerTool` : list, read, create, update.

use super::validators::parse_skill_category;
use super::SkillManagerTool;
use crate::commands::skill_version::snapshot_skill_version_core;
use crate::models::agent::AgentKind;
use crate::models::skill::{
    validate_skill_content, validate_skill_description, validate_skill_name,
};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use tracing::info;

impl SkillManagerTool {
    pub(super) async fn list_skills(
        &self,
        category: Option<&str>,
        kind: Option<&str>,
    ) -> ToolResult<Value> {
        self.ensure_kanban()?;
        let mut conds: Vec<String> = vec!["enabled = true".to_string()];
        let mut params: Vec<(String, Value)> = Vec::new();
        if let Some(c) = category.filter(|c| !c.trim().is_empty()) {
            conds.push("category = $cat".to_string());
            params.push(("cat".to_string(), json!(c.trim())));
        }
        match kind.map(str::trim) {
            Some("standard") => conds.push("kind IS NONE".to_string()),
            Some("kanban") => {
                conds.push("kind = $kind".to_string());
                params.push(("kind".to_string(), json!("kanban")));
            }
            _ => {}
        }
        let q = format!(
            "SELECT meta::id(id) AS id, name, description, category, kind, \
             string::len(content) AS content_length, updated_at \
             FROM skill WHERE {} ORDER BY name ASC LIMIT 200",
            conds.join(" AND ")
        );
        let rows: Vec<Value> = self
            .db
            .query_json_with_params(&q, params)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_skills: {}", e)))?;
        Ok(json!({"success": true, "skills": rows}))
    }

    pub(super) async fn read_skill(&self, name: &str) -> ToolResult<Value> {
        self.ensure_kanban()?;
        let q = "SELECT meta::id(id) AS id, name, description, category, content, enabled, kind, \
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

    /// Looks up the target agent's `kind` field. Returns `None` for a
    /// standard agent (`kind` field missing or null) and `Some(AgentKind)`
    /// otherwise. Errors with `NotFound` if the target does not exist.
    async fn lookup_target_agent_kind(
        &self,
        target_agent_id: &str,
    ) -> ToolResult<Option<AgentKind>> {
        let q = format!("SELECT kind FROM agent:`{}`", target_agent_id);
        let rows = self
            .db
            .query_json(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("lookup_target_agent: {}", e)))?;
        let row = rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Agent {}", target_agent_id)))?;
        match row["kind"].as_str() {
            Some("kanban") => Ok(Some(AgentKind::Kanban)),
            _ => Ok(None),
        }
    }

    pub(super) async fn create_skill(&self, input: &Value) -> ToolResult<Value> {
        self.ensure_kanban()?;
        let target_agent_id = validate_uuid_field(
            input["target_agent_id"].as_str().ok_or_else(|| {
                ToolError::InvalidInput(
                    "target_agent_id is required (the agent who receives the skill)".to_string(),
                )
            })?,
            "target_agent_id",
        )
        .map_err(ToolError::InvalidInput)?;

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

        // Reject collisions early so we never end up with two `skill` rows
        // sharing a name (read_skill resolves by name and would otherwise
        // become non-deterministic).
        self.ensure_name_unique(&name, None).await?;

        // Derive the skill's kind from the target agent. The tool never lets
        // the LLM pass `kind` directly — keeps the invariant
        // `skill.kind == target.kind` watertight.
        let target_kind = self.lookup_target_agent_kind(&target_agent_id).await?;
        let kind_value: Value = match target_kind {
            Some(AgentKind::Kanban) => json!("kanban"),
            None => Value::Null,
        };

        let id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE skill:`{}` CONTENT {{
                name: $name, description: $description, category: $category,
                content: $content, enabled: true, kind: $kind,
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
                    ("kind".to_string(), kind_value),
                ],
            )
            .await
            .map_err(|e| ToolError::DatabaseError(format!("create_skill: {}", e)))?;

        // Baseline v1 snapshot so the initial state is recoverable via
        // restore_skill_version (any later update creates v2, v3, ...).
        snapshot_skill_version_core(
            &self.db,
            &id,
            &self.edited_by(),
            Some("Initial version".to_string()),
        )
        .await
        .map_err(ToolError::DatabaseError)?;

        // Auto-grant: add the skill name to the *target* agent's `skills`
        // array (NOT the caller). Both standard and Kanban targets are
        // supported.
        self.grant_skill_name_raw(&target_agent_id, &name).await?;

        info!(
            skill_id = %id,
            skill_name = %name,
            target_agent_id = %target_agent_id,
            target_kind = ?target_kind,
            "Skill created by Kanban agent"
        );
        Ok(json!({
            "success": true,
            "skill_id": id,
            "name": name,
            "target_agent_id": target_agent_id,
            "kind": target_kind.map(|_| "kanban").unwrap_or("standard")
        }))
    }

    pub(super) async fn update_skill(&self, input: &Value) -> ToolResult<Value> {
        self.ensure_kanban()?;
        Self::reject_immutable_fields_on_update(input)?;
        let skill_id = validate_uuid_field(
            input["skill_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("skill_id required".to_string()))?,
            "skill_id",
        )
        .map_err(ToolError::InvalidInput)?;
        let edit_summary =
            Self::validate_edit_summary(input["edit_summary"].as_str().unwrap_or(""))?;

        // Verify the skill exists before snapshotting / updating.
        let name_q = format!("SELECT name FROM skill:`{}`", skill_id);
        let name_rows = self
            .db
            .query_json(&name_q)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let old_name = name_rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Skill {}", skill_id)))?["name"]
            .as_str()
            .unwrap_or("")
            .to_string();

        let mut sets: Vec<String> = Vec::new();
        let mut new_name: Option<String> = None;
        if let Some(n) = input["name"].as_str() {
            let v = validate_skill_name(n).map_err(ToolError::ValidationFailed)?;
            if v != old_name {
                // A rename must not collide with another existing skill.
                self.ensure_name_unique(&v, Some(&skill_id)).await?;
            }
            sets.push(format!(
                "name = {}",
                serialize_for_query(&v, "name").map_err(ToolError::ExecutionFailed)?
            ));
            new_name = Some(v);
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

        // Cascade rename through every agent's allowlist after the row
        // update succeeded — `agent.skills` stores names, not ids.
        if let Some(ref n) = new_name {
            self.cascade_skill_rename(&old_name, n).await?;
        }

        Ok(json!({"success": true, "skill_id": skill_id}))
    }
}
