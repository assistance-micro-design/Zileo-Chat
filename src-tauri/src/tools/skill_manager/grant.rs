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
    pub(super) async fn grant_skill_to_agent(
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
