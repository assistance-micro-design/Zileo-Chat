// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! SkillManagerTool — list/read/create/update skills.
//!
//! Reserved to Kanban-kind agents. Standard agents cannot author or modify
//! skills via tools (they keep `ReadSkillTool` for their assigned skills).
//!
//! `create_skill` requires a `target_agent_id` (mandatory): the new skill's
//! `kind` is derived from the target agent's `kind`, and the skill name is
//! auto-granted to the target agent's `skills` list. The target can be the
//! calling Kanban agent itself, another Kanban agent, or a standard agent.

use crate::commands::skill_version::{
    list_skill_versions_core, restore_skill_version_core, snapshot_skill_version_core,
};
use crate::db::DBClient;
use crate::models::agent::AgentKind;
use crate::models::skill::{
    validate_skill_content, validate_skill_description, validate_skill_name, SkillCategory,
};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info, warn};

fn parse_skill_category(s: &str) -> ToolResult<SkillCategory> {
    serde_json::from_value(json!(s))
        .map_err(|_| ToolError::ValidationFailed(format!("Unknown category: {}", s)))
}

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| ToolDefinition {
    id: "SkillManagerTool".to_string(),
    name: "SkillManager".to_string(),
    summary: "List, read, create or update skill documents (Kanban agents only)".to_string(),
    description: ToolDescriptionBuilder::new(
        "Manages skills (reusable markdown instructions). Reserved to Kanban-kind agents. \
         When creating a skill you must designate a `target_agent_id` (any existing agent — \
         standard or Kanban). The skill's kind is derived from the target's kind and the \
         skill is auto-granted to the target's skill list. Versioning is automatic: \
         create_skill stores a baseline v1, update_skill snapshots the previous state \
         before applying changes, restore_skill_version rolls back to a previous version \
         while preserving history.",
    )
    .use_when(&[
        "You are a Kanban agent composing or refining a skill document",
        "You need to read any skill (full content, regardless of allowlist)",
        "You want to assign a new skill to a specific agent (yourself or another)",
        "You want to rollback a skill to a previous version after a bad edit",
        "You want to revoke a skill from an agent's allowlist (without deleting the skill)",
    ])
    .do_not_use(&[
        "You are NOT a Kanban agent (every operation returns PermissionDenied)",
        "Operating on prompts (use PromptManagerTool)",
        "Deleting skills (UI only — revoke from agents instead)",
    ])
    .operations(&[
        (
            "list_skills",
            "List skills, optional `category` filter, optional `kind` filter (standard | kanban)",
        ),
        ("read_skill", "Read a skill by name (`name` field)"),
        (
            "create_skill",
            "Create with {name, description, content, category?, target_agent_id}; \
             kind is derived from the target agent. Rejects duplicate names. \
             Stores a baseline v1 snapshot automatically.",
        ),
        (
            "update_skill",
            "Update by skill_id; requires `edit_summary` (max 500 chars). \
             Renaming via `name` cascades to every agent's allowlist. \
             `kind` and `target_agent_id` are not accepted (rejected as invalid input).",
        ),
        (
            "list_skill_versions",
            "List all snapshots for a skill (by `skill_id`), ordered version DESC. \
             Each entry carries version, edited_by, edit_summary, edited_at.",
        ),
        (
            "restore_skill_version",
            "Restore a skill (by `skill_id`) to a previous version (by `version_id`). \
             A new snapshot of the current state is created before applying the restore, \
             so history is never destroyed. Cascades rename if the restored name differs.",
        ),
        (
            "revoke_skill_from_agent",
            "Remove `skill_name` from `target_agent_id`'s allowlist. \
             The skill row is preserved (other agents and history remain intact). \
             Idempotent: succeeds even if the name was not granted.",
        ),
    ])
    .examples(&[
        json!({"operation": "list_skills"}),
        json!({"operation": "list_skills", "kind": "kanban"}),
        json!({"operation": "read_skill", "name": "coding-standards"}),
        json!({
            "operation": "create_skill",
            "name": "review-checklist",
            "description": "Pre-merge gate",
            "content": "# Checklist\n- ...",
            "category": "workflow",
            "target_agent_id": "<uuid-of-an-agent>"
        }),
        json!({
            "operation": "update_skill",
            "skill_id": "<uuid>",
            "content": "...",
            "edit_summary": "Added section on perf budget"
        }),
        json!({"operation": "list_skill_versions", "skill_id": "<uuid>"}),
        json!({
            "operation": "restore_skill_version",
            "skill_id": "<uuid>",
            "version_id": "<version-uuid>"
        }),
        json!({
            "operation": "revoke_skill_from_agent",
            "target_agent_id": "<uuid>",
            "skill_name": "outdated-skill"
        }),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": [
                    "list_skills", "read_skill", "create_skill", "update_skill",
                    "list_skill_versions", "restore_skill_version", "revoke_skill_from_agent"
                ]
            },
            "skill_id": {"type": "string", "description": "UUID of the skill row (update / list_skill_versions / restore_skill_version)."},
            "version_id": {"type": "string", "description": "UUID of a skill_version row (restore_skill_version only)."},
            "name": {"type": "string", "description": "Skill identifier (create_skill, read_skill, update_skill rename)."},
            "skill_name": {"type": "string", "description": "Name to revoke from an agent's allowlist (revoke_skill_from_agent)."},
            "description": {"type": "string"},
            "content": {"type": "string"},
            "category": {"type": "string"},
            "kind": {"type": "string", "enum": ["standard", "kanban"], "description": "list_skills filter only. NOT accepted on update_skill (rejected)."},
            "target_agent_id": {"type": "string", "description": "Required on create_skill and revoke_skill_from_agent. NOT accepted on update_skill (rejected)."},
            "edit_summary": {"type": "string", "description": "Required on update_skill, max 500 chars, no control chars."},
        },
        "required": ["operation"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
});

pub struct SkillManagerTool {
    db: Arc<DBClient>,
    agent_id: String,
    /// Whether the calling agent is a Kanban-kind agent. Non-Kanban callers
    /// receive `PermissionDenied` on every operation.
    is_kanban: bool,
}

impl SkillManagerTool {
    pub fn new(db: Arc<DBClient>, agent_id: String, agent_kind: Option<AgentKind>) -> Self {
        let is_kanban = matches!(agent_kind, Some(AgentKind::Kanban));
        Self {
            db,
            agent_id,
            is_kanban,
        }
    }

    fn edited_by(&self) -> String {
        format!("agent:{}", self.agent_id)
    }

    fn ensure_kanban(&self) -> ToolResult<()> {
        if self.is_kanban {
            Ok(())
        } else {
            warn!(
                agent_id = %self.agent_id,
                "Non-Kanban agent attempted to use SkillManagerTool"
            );
            Err(ToolError::PermissionDenied(
                "SkillManagerTool is reserved to Kanban-kind agents".to_string(),
            ))
        }
    }

    /// Reject `kind` and `target_agent_id` on update_skill payloads. Both
    /// fields are immutable once the skill is created — silently ignoring
    /// them would surprise the LLM (changes vanish without error).
    fn reject_immutable_fields_on_update(input: &Value) -> ToolResult<()> {
        for field in ["kind", "target_agent_id"] {
            if let Some(v) = input.get(field) {
                if !v.is_null() {
                    return Err(ToolError::InvalidInput(format!(
                        "'{}' cannot be modified via update_skill (immutable once set)",
                        field
                    )));
                }
            }
        }
        Ok(())
    }

    /// Returns `Err(ValidationFailed)` if a skill row already has this name
    /// (excluding `exclude_id` so the caller can re-use the helper for
    /// rename checks on update). Backend stays the source of truth — the
    /// schema does not enforce uniqueness.
    async fn ensure_name_unique(&self, name: &str, exclude_id: Option<&str>) -> ToolResult<()> {
        let q = "SELECT meta::id(id) AS id FROM skill WHERE name = $n";
        let rows = self
            .db
            .query_json_with_params(q, vec![("n".to_string(), json!(name))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("ensure_name_unique: {}", e)))?;
        let collides = rows.into_iter().any(|r| r["id"].as_str() != exclude_id);
        if collides {
            return Err(ToolError::ValidationFailed(format!(
                "A skill named '{}' already exists",
                name
            )));
        }
        Ok(())
    }

    /// Rewrites every agent's `skills` allowlist to replace `old_name` with
    /// `new_name`. Skips no-op renames. Necessary because the `agent.skills`
    /// array stores skill names (not ids), so a rename without cascade
    /// leaves dangling references in every agent's allowlist.
    async fn cascade_skill_rename(&self, old_name: &str, new_name: &str) -> ToolResult<()> {
        if old_name == new_name || old_name.is_empty() {
            return Ok(());
        }
        let q = "UPDATE agent SET skills = \
                 array::union(array::difference(skills ?? [], [$old]), [$new]) \
                 WHERE skills CONTAINS $old";
        self.db
            .execute_with_params(
                q,
                vec![
                    ("old".to_string(), json!(old_name)),
                    ("new".to_string(), json!(new_name)),
                ],
            )
            .await
            .map_err(|e| ToolError::DatabaseError(format!("cascade_skill_rename: {}", e)))?;
        Ok(())
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

    async fn list_skills(&self, category: Option<&str>, kind: Option<&str>) -> ToolResult<Value> {
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

    async fn read_skill(&self, name: &str) -> ToolResult<Value> {
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

    async fn create_skill(&self, input: &Value) -> ToolResult<Value> {
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
        self.grant_skill_to_agent(&target_agent_id, &name).await?;

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

    async fn grant_skill_to_agent(
        &self,
        target_agent_id: &str,
        skill_name: &str,
    ) -> ToolResult<()> {
        let q = format!(
            "UPDATE agent:`{}` SET skills = array::union(skills ?? [], [$name])",
            target_agent_id
        );
        self.db
            .execute_with_params(&q, vec![("name".to_string(), json!(skill_name))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("grant_skill_to_agent: {}", e)))?;
        Ok(())
    }

    async fn update_skill(&self, input: &Value) -> ToolResult<Value> {
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

    async fn list_skill_versions(&self, input: &Value) -> ToolResult<Value> {
        self.ensure_kanban()?;
        let skill_id = validate_uuid_field(
            input["skill_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("skill_id required".to_string()))?,
            "skill_id",
        )
        .map_err(ToolError::InvalidInput)?;
        let versions = list_skill_versions_core(&self.db, &skill_id)
            .await
            .map_err(ToolError::DatabaseError)?;
        Ok(json!({"success": true, "versions": versions}))
    }

    async fn restore_skill_version(&self, input: &Value) -> ToolResult<Value> {
        self.ensure_kanban()?;
        let skill_id = validate_uuid_field(
            input["skill_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("skill_id required".to_string()))?,
            "skill_id",
        )
        .map_err(ToolError::InvalidInput)?;
        let version_id = validate_uuid_field(
            input["version_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("version_id required".to_string()))?,
            "version_id",
        )
        .map_err(ToolError::InvalidInput)?;

        // Capture current name to detect (and cascade) a rollback rename.
        let cur_q = format!("SELECT name FROM skill:`{}`", skill_id);
        let cur_rows = self
            .db
            .query_json(&cur_q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("restore_skill_version: {}", e)))?;
        let old_name = cur_rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Skill {}", skill_id)))?["name"]
            .as_str()
            .unwrap_or("")
            .to_string();

        restore_skill_version_core(&self.db, &skill_id, &version_id, &self.edited_by())
            .await
            .map_err(ToolError::DatabaseError)?;

        let new_q = format!("SELECT name FROM skill:`{}`", skill_id);
        let new_rows = self
            .db
            .query_json(&new_q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("restore_skill_version: {}", e)))?;
        let new_name = new_rows
            .into_iter()
            .next()
            .and_then(|r| r["name"].as_str().map(String::from))
            .unwrap_or_default();
        if !new_name.is_empty() && new_name != old_name {
            self.cascade_skill_rename(&old_name, &new_name).await?;
        }

        info!(
            skill_id = %skill_id,
            version_id = %version_id,
            "Skill restored to previous version"
        );
        Ok(json!({
            "success": true,
            "skill_id": skill_id,
            "version_id": version_id
        }))
    }

    async fn revoke_skill_from_agent(&self, input: &Value) -> ToolResult<Value> {
        self.ensure_kanban()?;
        let target_agent_id = validate_uuid_field(
            input["target_agent_id"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("target_agent_id required".to_string()))?,
            "target_agent_id",
        )
        .map_err(ToolError::InvalidInput)?;
        let skill_name = validate_skill_name(
            input["skill_name"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidInput("skill_name required".to_string()))?,
        )
        .map_err(ToolError::ValidationFailed)?;

        // Confirm the agent exists; revoke on a phantom id should not
        // silently succeed.
        let chk_q = format!("SELECT meta::id(id) AS id FROM agent:`{}`", target_agent_id);
        let chk_rows = self
            .db
            .query_json(&chk_q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("revoke_skill_from_agent: {}", e)))?;
        if chk_rows.is_empty() {
            return Err(ToolError::NotFound(format!("Agent {}", target_agent_id)));
        }

        let q = format!(
            "UPDATE agent:`{}` SET skills = array::difference(skills ?? [], [$name])",
            target_agent_id
        );
        self.db
            .execute_with_params(&q, vec![("name".to_string(), json!(skill_name))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("revoke_skill_from_agent: {}", e)))?;

        info!(
            target_agent_id = %target_agent_id,
            skill_name = %skill_name,
            "Skill revoked from agent"
        );
        Ok(json!({
            "success": true,
            "target_agent_id": target_agent_id,
            "skill_name": skill_name
        }))
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
            "list_skills" => {
                self.list_skills(input["category"].as_str(), input["kind"].as_str())
                    .await
            }
            "read_skill" => {
                let name = input["name"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("name required".to_string()))?;
                self.read_skill(name).await
            }
            "create_skill" => self.create_skill(&input).await,
            "update_skill" => self.update_skill(&input).await,
            "list_skill_versions" => self.list_skill_versions(&input).await,
            "restore_skill_version" => self.restore_skill_version(&input).await,
            "revoke_skill_from_agent" => self.revoke_skill_from_agent(&input).await,
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
            "list_skills"
            | "read_skill"
            | "create_skill"
            | "update_skill"
            | "list_skill_versions"
            | "restore_skill_version"
            | "revoke_skill_from_agent" => Ok(()),
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

    fn kanban_tool(db: Arc<DBClient>, agent_id: &str) -> SkillManagerTool {
        SkillManagerTool::new(db, agent_id.to_string(), Some(AgentKind::Kanban))
    }

    fn standard_tool(db: Arc<DBClient>, agent_id: &str) -> SkillManagerTool {
        SkillManagerTool::new(db, agent_id.to_string(), None)
    }

    async fn insert_agent(db: &Arc<DBClient>, id: &str, kind: Option<&str>) {
        let kind_clause = match kind {
            Some(k) => format!(", kind: '{}'", k),
            None => String::new(),
        };
        let q = format!(
            "CREATE agent:`{}` CONTENT {{
                name: 'A_{}', lifecycle: 'permanent', system_prompt: 's',
                llm: {{ provider: 'mistral', model: 'm', temperature: 0.7, max_tokens: 4096 }},
                tools: [], mcp_servers: [], skills: [], folders: [],
                max_tool_iterations: 50,
                auto_analyze_reports: false{},
                created_at: time::now(), updated_at: time::now()
            }}",
            id,
            &id[..8],
            kind_clause
        );
        db.execute(&q).await.unwrap();
    }

    #[test]
    fn test_edit_summary_required() {
        assert!(SkillManagerTool::validate_edit_summary("").is_err());
        assert!(SkillManagerTool::validate_edit_summary("ok").is_ok());
    }

    #[tokio::test]
    async fn test_standard_agent_denied_on_list() {
        let (state, _g) = setup_test_state().await;
        let tool = standard_tool(state.db.clone(), "agent-x");
        let err = tool
            .execute(json!({"operation": "list_skills"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn test_standard_agent_denied_on_create() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &agent_id, None).await;
        let tool = standard_tool(state.db.clone(), &agent_id);
        let err = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "new-skill",
                "description": "x",
                "content": "# c",
                "target_agent_id": agent_id
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }

    #[tokio::test]
    async fn test_kanban_create_for_standard_agent_yields_standard_kind() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;

        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let res = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "for-standard",
                "description": "Pour un agent standard",
                "content": "# Standard skill",
                "category": "custom",
                "target_agent_id": target_id
            }))
            .await
            .unwrap();
        assert_eq!(res["success"], true);
        assert_eq!(res["kind"], "standard");
        assert_eq!(res["target_agent_id"], target_id);

        // Target agent now has the skill in its allowlist.
        let q = format!("SELECT skills FROM agent:`{}`", target_id);
        let rows = state.db.query_json(&q).await.unwrap();
        let skills = rows[0]["skills"].as_array().unwrap();
        assert!(skills.iter().any(|v| v.as_str() == Some("for-standard")));

        // Skill row has kind = NONE (returned as null from query_json).
        let sq = "SELECT kind FROM skill WHERE name = $n";
        let srows = state
            .db
            .query_json_with_params(sq, vec![("n".to_string(), json!("for-standard"))])
            .await
            .unwrap();
        assert!(srows[0]["kind"].is_null());
    }

    #[tokio::test]
    async fn test_kanban_create_for_kanban_target_yields_kanban_kind() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, Some("kanban")).await;

        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let res = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "for-kanban",
                "description": "Pour un agent kanban",
                "content": "# Kanban skill",
                "category": "workflow",
                "target_agent_id": target_id
            }))
            .await
            .unwrap();
        assert_eq!(res["kind"], "kanban");

        let sq = "SELECT kind FROM skill WHERE name = $n";
        let srows = state
            .db
            .query_json_with_params(sq, vec![("n".to_string(), json!("for-kanban"))])
            .await
            .unwrap();
        assert_eq!(srows[0]["kind"], "kanban");
    }

    #[tokio::test]
    async fn test_kanban_create_rejects_missing_target() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let err = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "x", "description": "x", "content": "# x"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    async fn create_skill_for_target(
        tool: &SkillManagerTool,
        name: &str,
        target_id: &str,
    ) -> String {
        let res = tool
            .execute(json!({
                "operation": "create_skill",
                "name": name,
                "description": "seed desc",
                "content": "# Seed content",
                "target_agent_id": target_id
            }))
            .await
            .unwrap();
        res["skill_id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn test_create_skill_records_baseline_version() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let skill_id = create_skill_for_target(&tool, "baseline-skill", &target_id).await;

        let res = tool
            .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
            .await
            .unwrap();
        let versions = res["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 1);
        assert_eq!(versions[0]["version"], 1);
        assert_eq!(versions[0]["edit_summary"], "Initial version");
    }

    #[tokio::test]
    async fn test_create_skill_rejects_duplicate_name() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        create_skill_for_target(&tool, "dup-name", &target_id).await;

        let err = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "dup-name",
                "description": "again",
                "content": "# again",
                "target_agent_id": target_id
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn test_update_skill_rejects_kind_field() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let skill_id = create_skill_for_target(&tool, "kind-reject", &target_id).await;

        let err = tool
            .execute(json!({
                "operation": "update_skill",
                "skill_id": skill_id,
                "kind": "kanban",
                "edit_summary": "trying to change kind"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_update_skill_rejects_target_agent_id_field() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let skill_id = create_skill_for_target(&tool, "target-reject", &target_id).await;

        let err = tool
            .execute(json!({
                "operation": "update_skill",
                "skill_id": skill_id,
                "target_agent_id": target_id,
                "edit_summary": "trying to retarget"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_update_skill_rename_cascades_to_agents() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        let other_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        insert_agent(&state.db, &other_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let skill_id = create_skill_for_target(&tool, "old-name", &target_id).await;
        // Manually grant the same skill to a second agent so the cascade has
        // more than one agent to update.
        tool.grant_skill_to_agent(&other_id, "old-name")
            .await
            .unwrap();

        tool.execute(json!({
            "operation": "update_skill",
            "skill_id": skill_id,
            "name": "new-name",
            "edit_summary": "Renamed"
        }))
        .await
        .unwrap();

        for aid in [&target_id, &other_id] {
            let q = format!("SELECT skills FROM agent:`{}`", aid);
            let rows = state.db.query_json(&q).await.unwrap();
            let skills = rows[0]["skills"].as_array().unwrap();
            assert!(
                skills.iter().any(|v| v.as_str() == Some("new-name")),
                "agent {} missing renamed skill",
                aid
            );
            assert!(
                skills.iter().all(|v| v.as_str() != Some("old-name")),
                "agent {} still has old name",
                aid
            );
        }
    }

    #[tokio::test]
    async fn test_update_skill_rejects_rename_collision() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        create_skill_for_target(&tool, "skill-a", &target_id).await;
        let id_b = create_skill_for_target(&tool, "skill-b", &target_id).await;

        let err = tool
            .execute(json!({
                "operation": "update_skill",
                "skill_id": id_b,
                "name": "skill-a",
                "edit_summary": "Collide on purpose"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn test_list_skill_versions_orders_desc() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let skill_id = create_skill_for_target(&tool, "versioned", &target_id).await;

        tool.execute(json!({
            "operation": "update_skill",
            "skill_id": skill_id,
            "content": "# Updated once",
            "edit_summary": "Second cut"
        }))
        .await
        .unwrap();

        let res = tool
            .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
            .await
            .unwrap();
        let versions = res["versions"].as_array().unwrap();
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0]["version"], 2);
        assert_eq!(versions[1]["version"], 1);
    }

    #[tokio::test]
    async fn test_restore_skill_version_rolls_back_content() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let skill_id = create_skill_for_target(&tool, "rollback-me", &target_id).await;

        // v2 changes the content.
        tool.execute(json!({
            "operation": "update_skill",
            "skill_id": skill_id,
            "content": "# Mutated content",
            "edit_summary": "Worse version"
        }))
        .await
        .unwrap();

        // Identify v1's version_id (the baseline).
        let versions = tool
            .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
            .await
            .unwrap();
        let v1 = versions["versions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["version"] == 1)
            .unwrap()
            .clone();
        let v1_id = v1["id"].as_str().unwrap().to_string();

        tool.execute(json!({
            "operation": "restore_skill_version",
            "skill_id": skill_id,
            "version_id": v1_id
        }))
        .await
        .unwrap();

        // Skill content now matches the baseline; history grew to 3 rows.
        let read = tool
            .execute(json!({"operation": "read_skill", "name": "rollback-me"}))
            .await
            .unwrap();
        assert_eq!(read["skill"]["content"], "# Seed content");

        let versions = tool
            .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
            .await
            .unwrap();
        assert_eq!(versions["versions"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_revoke_skill_from_agent_removes_name() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        create_skill_for_target(&tool, "revoke-me", &target_id).await;

        // Sanity: target has the skill granted by create_skill.
        let q = format!("SELECT skills FROM agent:`{}`", target_id);
        let rows = state.db.query_json(&q).await.unwrap();
        assert!(rows[0]["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v.as_str() == Some("revoke-me")));

        tool.execute(json!({
            "operation": "revoke_skill_from_agent",
            "target_agent_id": target_id,
            "skill_name": "revoke-me"
        }))
        .await
        .unwrap();

        let rows = state.db.query_json(&q).await.unwrap();
        assert!(rows[0]["skills"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v.as_str() != Some("revoke-me")));
    }

    #[tokio::test]
    async fn test_revoke_skill_from_agent_is_idempotent() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let target_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        insert_agent(&state.db, &target_id, None).await;
        let tool = kanban_tool(state.db.clone(), &kanban_id);

        // Skill was never granted; revoke should still succeed.
        let res = tool
            .execute(json!({
                "operation": "revoke_skill_from_agent",
                "target_agent_id": target_id,
                "skill_name": "never-granted"
            }))
            .await
            .unwrap();
        assert_eq!(res["success"], true);
    }

    #[tokio::test]
    async fn test_revoke_skill_rejects_unknown_agent() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        let unknown = uuid::Uuid::new_v4().to_string();
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let err = tool
            .execute(json!({
                "operation": "revoke_skill_from_agent",
                "target_agent_id": unknown,
                "skill_name": "anything"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_standard_agent_denied_on_new_ops() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &agent_id, None).await;
        let tool = standard_tool(state.db.clone(), &agent_id);
        for op in [
            json!({"operation": "list_skill_versions", "skill_id": uuid::Uuid::new_v4().to_string()}),
            json!({
                "operation": "restore_skill_version",
                "skill_id": uuid::Uuid::new_v4().to_string(),
                "version_id": uuid::Uuid::new_v4().to_string()
            }),
            json!({
                "operation": "revoke_skill_from_agent",
                "target_agent_id": agent_id,
                "skill_name": "x"
            }),
        ] {
            let err = tool.execute(op).await.unwrap_err();
            assert!(matches!(err, ToolError::PermissionDenied(_)));
        }
    }

    #[tokio::test]
    async fn test_kanban_create_rejects_unknown_target() {
        let (state, _g) = setup_test_state().await;
        let kanban_id = uuid::Uuid::new_v4().to_string();
        insert_agent(&state.db, &kanban_id, Some("kanban")).await;
        let unknown_id = uuid::Uuid::new_v4().to_string();
        let tool = kanban_tool(state.db.clone(), &kanban_id);
        let err = tool
            .execute(json!({
                "operation": "create_skill",
                "name": "x", "description": "x", "content": "# x",
                "target_agent_id": unknown_id
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }
}
