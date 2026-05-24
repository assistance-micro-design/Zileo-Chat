// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Grant / revoke a skill name on a target agent's allowlist.

use super::SkillManagerTool;
use crate::models::skill::validate_skill_name;
use crate::security::validate_uuid_field;
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use tracing::info;

impl SkillManagerTool {
    /// Raw idempotent grant: adds `skill_name` to the target agent's `skills`
    /// allowlist. Assumes the caller has already validated existence and the
    /// `skill.kind == agent.kind` invariant. Used by `create_skill` (where the
    /// invariant holds by construction) and by `grant_skill_to_agent` (which
    /// enforces it explicitly first).
    pub(super) async fn grant_skill_name_raw(
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
            .map_err(|e| ToolError::DatabaseError(format!("grant_skill_name_raw: {}", e)))?;
        Ok(())
    }

    /// Normalizes a raw `kind` field (`Some("kanban")` or absent/null) to the
    /// canonical "kanban" / "standard" string used for the separation check.
    fn normalize_kind(raw: Option<&str>) -> &'static str {
        match raw {
            Some("kanban") => "kanban",
            _ => "standard",
        }
    }

    /// Grants an EXISTING skill to an agent's allowlist.
    ///
    /// Unlike the raw helper, this enforces the guards `create_skill` provided
    /// implicitly: the skill must exist, the agent must exist, and — critically
    /// — their `kind` must match (Kanban strict separation). Granting a
    /// `kanban` skill to a `standard` agent (or vice versa) is rejected.
    /// Idempotent: re-granting an already-present skill succeeds silently.
    pub(super) async fn grant_skill_to_agent(&self, input: &Value) -> ToolResult<Value> {
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

        // The skill must exist (and be enabled); capture its kind.
        let skill_rows = self
            .db
            .query_json_with_params(
                "SELECT kind FROM skill WHERE name = $n AND enabled = true LIMIT 1",
                vec![("n".to_string(), json!(skill_name))],
            )
            .await
            .map_err(|e| ToolError::DatabaseError(format!("grant_skill_to_agent: {}", e)))?;
        let skill_row = skill_rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Skill '{}'", skill_name)))?;
        let skill_kind = Self::normalize_kind(skill_row["kind"].as_str());

        // The target agent must exist; capture its kind.
        let agent_q = format!("SELECT kind FROM agent:`{}`", target_agent_id);
        let agent_rows = self
            .db
            .query_json(&agent_q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("grant_skill_to_agent: {}", e)))?;
        let agent_row = agent_rows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Agent {}", target_agent_id)))?;
        let agent_kind = Self::normalize_kind(agent_row["kind"].as_str());

        // Kanban strict separation: a skill can only be granted to an agent of
        // the same kind.
        if skill_kind != agent_kind {
            return Err(ToolError::ValidationFailed(format!(
                "Cannot grant a '{}' skill to a '{}' agent (Kanban strict separation)",
                skill_kind, agent_kind
            )));
        }

        self.grant_skill_name_raw(&target_agent_id, &skill_name)
            .await?;

        info!(
            target_agent_id = %target_agent_id,
            skill_name = %skill_name,
            kind = %skill_kind,
            "Skill granted to agent"
        );
        Ok(json!({
            "success": true,
            "target_agent_id": target_agent_id,
            "skill_name": skill_name
        }))
    }

    pub(super) async fn revoke_skill_from_agent(&self, input: &Value) -> ToolResult<Value> {
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
