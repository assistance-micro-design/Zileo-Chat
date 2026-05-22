// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Skill version history operations : list + restore.

use super::SkillManagerTool;
use crate::commands::skill_version::{list_skill_versions_core, restore_skill_version_core};
use crate::security::validate_uuid_field;
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use tracing::info;

impl SkillManagerTool {
    pub(super) async fn list_skill_versions(&self, input: &Value) -> ToolResult<Value> {
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

    pub(super) async fn restore_skill_version(&self, input: &Value) -> ToolResult<Value> {
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
}
