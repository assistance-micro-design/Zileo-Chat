// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Input validation and rename-cascade helpers for `SkillManagerTool`.

use super::SkillManagerTool;
use crate::models::skill::SkillCategory;
use crate::security::validate_edit_summary as validate_edit_summary_core;
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};

/// Parses a skill category from its string representation.
pub(super) fn parse_skill_category(s: &str) -> ToolResult<SkillCategory> {
    serde_json::from_value(json!(s))
        .map_err(|_| ToolError::ValidationFailed(format!("Unknown category: {}", s)))
}

impl SkillManagerTool {
    /// Thin wrapper around the shared validator. Agents always tag updates,
    /// so `edit_summary` is mandatory here.
    pub(super) fn validate_edit_summary(s: &str) -> ToolResult<String> {
        validate_edit_summary_core(Some(s), true)
            .map_err(ToolError::ValidationFailed)?
            .ok_or_else(|| ToolError::ValidationFailed("edit_summary is required".to_string()))
    }

    /// Reject `kind` and `target_agent_id` on update_skill payloads. Both
    /// fields are immutable once the skill is created — silently ignoring
    /// them would surprise the LLM (changes vanish without error).
    pub(super) fn reject_immutable_fields_on_update(input: &Value) -> ToolResult<()> {
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
    pub(super) async fn ensure_name_unique(
        &self,
        name: &str,
        exclude_id: Option<&str>,
    ) -> ToolResult<()> {
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
    pub(super) async fn cascade_skill_rename(
        &self,
        old_name: &str,
        new_name: &str,
    ) -> ToolResult<()> {
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
}
