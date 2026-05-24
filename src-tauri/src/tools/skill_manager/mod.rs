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
//!
//! The module is split by concern :
//! - [`validators`] — input validation and immutable-field guards.
//! - [`crud`]       — list / read / create / update operations.
//! - [`versions`]   — version history (list / restore).
//! - [`grant`]      — grant / revoke a skill on a target agent's allowlist.
//! - [`tests`]      — integration tests against an in-memory DB.

use crate::db::DBClient;
use crate::models::agent::AgentKind;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, warn};

mod crud;
mod grant;
mod validators;
mod versions;

#[cfg(test)]
mod tests;

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "SkillManagerTool".to_string(),
    name: "SkillManager".to_string(),
    summary: "List/read ANY skill in the system, plus create/update/version/revoke skills (Kanban agents only). Use this — not ReadSkill — to inspect or improve a skill you do not own".to_string(),
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
        "You want to attach an existing skill to another agent (same kind only)",
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
            "grant_skill_to_agent",
            "Attach an EXISTING skill (`skill_name`) to `target_agent_id`'s allowlist. \
             The skill and agent must exist and share the same kind (a kanban skill \
             only grants to a kanban agent, a standard skill to a standard agent) — \
             cross-kind grants are rejected. Idempotent.",
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
            "operation": "grant_skill_to_agent",
            "target_agent_id": "<uuid>",
            "skill_name": "existing-skill"
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
                    "list_skill_versions", "restore_skill_version",
                    "grant_skill_to_agent", "revoke_skill_from_agent"
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
}
});

pub struct SkillManagerTool {
    pub(super) db: Arc<DBClient>,
    pub(super) agent_id: String,
    /// Whether the calling agent is a Kanban-kind agent. Non-Kanban callers
    /// receive `PermissionDenied` on every operation.
    pub(super) is_kanban: bool,
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

    pub(super) fn edited_by(&self) -> String {
        format!("agent:{}", self.agent_id)
    }

    pub(super) fn ensure_kanban(&self) -> ToolResult<()> {
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
            "grant_skill_to_agent" => self.grant_skill_to_agent(&input).await,
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
            | "grant_skill_to_agent"
            | "revoke_skill_from_agent" => Ok(()),
            other => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                other
            ))),
        }
    }
}
