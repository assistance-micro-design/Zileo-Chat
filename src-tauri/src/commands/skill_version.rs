// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Skill version snapshots: list / get / restore.
//!
//! A snapshot row is inserted in `skill_version` BEFORE every `UPDATE skill`.
//! Restore is implemented as a snapshot-then-update — never destroys history.

use crate::db::DBClient;
use crate::models::skill_version::{SkillVersion, SkillVersionSummary};
use crate::security::{
    serialize_for_query, validate_edit_summary as validate_edit_summary_core, validate_uuid_field,
};
use crate::AppState;
use serde_json::json;
use tauri::State;
use tracing::{error, info, instrument};

const SKILL_VERSION_FIELDS: &str = "meta::id(id) AS id, skill_id, version, name, description, \
    category, content, edited_by, edit_summary, edited_at";

const SKILL_VERSION_SUMMARY_FIELDS: &str = "meta::id(id) AS id, skill_id, version, edited_by, \
    edit_summary, edited_at";

fn validate_edited_by(edited_by: &str) -> Result<String, String> {
    let trimmed = edited_by.trim();
    if trimmed == "user" {
        return Ok(trimmed.to_string());
    }
    if let Some(rest) = trimmed.strip_prefix("agent:") {
        validate_uuid_field(rest, "agent_id")?;
        return Ok(trimmed.to_string());
    }
    Err("edited_by must be 'user' or 'agent:<uuid>'".to_string())
}

/// `edit_summary` is optional when authored by the user, required when an
/// agent edits the skill. Delegates to the shared validator.
fn validate_edit_summary(
    edited_by: &str,
    edit_summary: &Option<String>,
) -> Result<Option<String>, String> {
    let required = edited_by.starts_with("agent:");
    validate_edit_summary_core(edit_summary.as_deref(), required)
}

pub async fn snapshot_skill_version_core(
    db: &DBClient,
    skill_id: &str,
    edited_by: &str,
    edit_summary: Option<String>,
) -> Result<i64, String> {
    let skill_id = validate_uuid_field(skill_id, "skill_id")?;
    let edited_by = validate_edited_by(edited_by)?;
    let edit_summary = validate_edit_summary(&edited_by, &edit_summary)?;

    let current_query = format!(
        "SELECT name, description, category, content FROM skill:`{}`",
        skill_id
    );
    let rows = db
        .query_json(&current_query)
        .await
        .map_err(|e| format!("Failed to load skill before snapshot: {}", e))?;
    let current = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Skill not found: {}", skill_id))?;

    let name = current["name"].as_str().unwrap_or("").to_string();
    let description = current["description"].as_str().unwrap_or("").to_string();
    let category = current["category"].as_str().unwrap_or("custom").to_string();
    let content = current["content"].as_str().unwrap_or("").to_string();

    let max_query = format!(
        "SELECT math::max(version) AS max_version FROM skill_version WHERE skill_id = '{}' GROUP ALL",
        skill_id
    );
    let max_rows = db
        .query_json(&max_query)
        .await
        .map_err(|e| format!("Failed to compute next version: {}", e))?;
    let next_version = max_rows
        .into_iter()
        .next()
        .and_then(|r| r["max_version"].as_i64())
        .unwrap_or(0)
        + 1;

    let id = uuid::Uuid::new_v4().to_string();
    let edit_summary_sql = match &edit_summary {
        Some(s) => format!("edit_summary: {}", serialize_for_query(s, "edit_summary")?),
        None => "edit_summary: NONE".to_string(),
    };

    let insert_query = format!(
        "CREATE skill_version:`{id}` CONTENT {{
            id: '{id}',
            skill_id: '{skill_id}',
            version: {next_version},
            name: $name,
            description: $description,
            category: $category,
            content: $content,
            edited_by: $edited_by,
            {edit_summary_sql},
            edited_at: time::now()
        }}"
    );

    db.execute_with_params(
        &insert_query,
        vec![
            ("name".to_string(), json!(name)),
            ("description".to_string(), json!(description)),
            ("category".to_string(), json!(category)),
            ("content".to_string(), json!(content)),
            ("edited_by".to_string(), json!(edited_by)),
        ],
    )
    .await
    .map_err(|e| format!("Failed to insert skill_version: {}", e))?;

    info!(skill_id = %skill_id, version = next_version, "Skill version snapshot created");
    Ok(next_version)
}

pub async fn list_skill_versions_core(
    db: &DBClient,
    skill_id: &str,
) -> Result<Vec<SkillVersionSummary>, String> {
    let skill_id = validate_uuid_field(skill_id, "skill_id")?;
    let query = format!(
        "SELECT {SKILL_VERSION_SUMMARY_FIELDS} FROM skill_version \
         WHERE skill_id = '{skill_id}' ORDER BY version DESC"
    );
    let rows = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to list skill versions: {}", e))?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| format!("Deserialization failed: {}", e)))
        .collect()
}

pub async fn get_skill_version_core(
    db: &DBClient,
    version_id: &str,
) -> Result<SkillVersion, String> {
    let version_id = validate_uuid_field(version_id, "version_id")?;
    let query = format!("SELECT {SKILL_VERSION_FIELDS} FROM skill_version:`{version_id}`");
    let rows = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to load skill version: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| "Skill version not found".to_string())?;
    serde_json::from_value(row).map_err(|e| format!("Failed to deserialize version: {}", e))
}

pub async fn restore_skill_version_core(
    db: &DBClient,
    skill_id: &str,
    version_id: &str,
    edited_by: &str,
) -> Result<(), String> {
    let skill_id = validate_uuid_field(skill_id, "skill_id")?;
    let target = get_skill_version_core(db, version_id).await?;
    if target.skill_id != skill_id {
        return Err("Version does not belong to this skill".to_string());
    }

    let summary = Some(format!("Restore to version {}", target.version));
    snapshot_skill_version_core(db, &skill_id, edited_by, summary).await?;

    let name_json = serialize_for_query(&target.name, "name")?;
    let desc_json = serialize_for_query(&target.description, "description")?;
    let cat_json = serialize_for_query(&target.category, "category")?;
    let content_json = serialize_for_query(&target.content, "content")?;

    let query = format!(
        "UPDATE skill:`{skill_id}` SET name = {name_json}, description = {desc_json}, \
         category = {cat_json}, content = {content_json}, updated_at = time::now()"
    );
    db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to restore skill");
        format!("Failed to restore skill: {}", e)
    })?;
    info!(skill_id = %skill_id, version = target.version, "Skill restored");
    Ok(())
}

/// Hard-deletes a single skill version row. The row must exist (returns
/// "not found" otherwise). Unlike `restore`, this is destructive and not
/// reversible — the UI confirms before calling. Only exposed via the
/// Settings UI; the SkillManagerTool intentionally does not expose this.
///
/// Refuses to delete the last remaining version for a given skill so the
/// skill always has at least one snapshot to fall back on.
pub async fn delete_skill_version_core(db: &DBClient, version_id: &str) -> Result<(), String> {
    let version_id = validate_uuid_field(version_id, "version_id")?;

    let check_q = format!(
        "SELECT meta::id(id) AS id, skill_id FROM skill_version:`{}`",
        version_id
    );
    let rows = db
        .query_json(&check_q)
        .await
        .map_err(|e| format!("Failed to check skill version: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Skill version not found: {}", version_id))?;
    let skill_id = row["skill_id"]
        .as_str()
        .ok_or_else(|| "Skill version row missing skill_id".to_string())?
        .to_string();

    let count_q = format!(
        "SELECT count() AS c FROM skill_version WHERE skill_id = '{}' GROUP ALL",
        skill_id
    );
    let count_rows = db
        .query_json(&count_q)
        .await
        .map_err(|e| format!("Failed to count skill versions: {}", e))?;
    let count = count_rows
        .into_iter()
        .next()
        .and_then(|r| r["c"].as_i64())
        .unwrap_or(0);
    if count <= 1 {
        return Err(
            "Cannot delete the last remaining version — at least one snapshot must be kept"
                .to_string(),
        );
    }

    let q = format!("DELETE skill_version:`{}`", version_id);
    db.execute(&q)
        .await
        .map_err(|e| format!("Failed to delete skill version: {}", e))?;
    info!(version_id = %version_id, skill_id = %skill_id, "Skill version deleted");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
#[instrument(name = "list_skill_versions", skip(state), fields(skill_id = %skill_id))]
pub async fn list_skill_versions(
    skill_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SkillVersionSummary>, String> {
    list_skill_versions_core(&state.db, &skill_id).await
}

#[tauri::command]
#[instrument(name = "get_skill_version", skip(state), fields(version_id = %version_id))]
pub async fn get_skill_version(
    version_id: String,
    state: State<'_, AppState>,
) -> Result<SkillVersion, String> {
    get_skill_version_core(&state.db, &version_id).await
}

#[tauri::command]
#[instrument(name = "restore_skill_version", skip(state))]
pub async fn restore_skill_version(
    skill_id: String,
    version_id: String,
    edited_by: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let edited_by = edited_by.unwrap_or_else(|| "user".to_string());
    restore_skill_version_core(&state.db, &skill_id, &version_id, &edited_by).await
}

#[tauri::command]
#[instrument(name = "delete_skill_version", skip(state), fields(version_id = %version_id))]
pub async fn delete_skill_version(
    version_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    delete_skill_version_core(&state.db, &version_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_edited_by_user() {
        assert_eq!(validate_edited_by("user").unwrap(), "user");
    }

    #[test]
    fn test_validate_edited_by_agent() {
        let id = uuid::Uuid::new_v4().to_string();
        let input = format!("agent:{}", id);
        assert_eq!(validate_edited_by(&input).unwrap(), input);
    }

    #[test]
    fn test_validate_edited_by_rejects_garbage() {
        assert!(validate_edited_by("nobody").is_err());
        assert!(validate_edited_by("agent:not-a-uuid").is_err());
    }

    #[test]
    fn test_edit_summary_required_for_agent() {
        let id = format!("agent:{}", uuid::Uuid::new_v4());
        assert!(validate_edit_summary(&id, &None).is_err());
        assert!(validate_edit_summary(&id, &Some("ok".to_string())).is_ok());
    }

    #[test]
    fn test_edit_summary_optional_for_user() {
        assert!(validate_edit_summary("user", &None).unwrap().is_none());
    }

    use crate::test_utils::setup_test_state;

    async fn seed_skill_with_versions(
        db: &DBClient,
        version_count: usize,
    ) -> (String, Vec<String>) {
        let skill_id = uuid::Uuid::new_v4().to_string();
        let create_q = format!(
            "CREATE skill:`{}` CONTENT {{
                name: 'seed', description: 'd', category: 'custom',
                content: '# c', enabled: true,
                created_at: time::now(), updated_at: time::now()
            }}",
            skill_id
        );
        db.execute(&create_q).await.unwrap();

        let mut version_ids = Vec::new();
        for _ in 0..version_count {
            let v = snapshot_skill_version_core(db, &skill_id, "user", None)
                .await
                .unwrap();
            let q = format!(
                "SELECT meta::id(id) AS id FROM skill_version WHERE skill_id = '{}' AND version = {}",
                skill_id, v
            );
            let rows = db.query_json(&q).await.unwrap();
            version_ids.push(rows[0]["id"].as_str().unwrap().to_string());
        }
        (skill_id, version_ids)
    }

    #[tokio::test]
    async fn test_delete_skill_version_removes_row() {
        let (state, _g) = setup_test_state().await;
        let (_skill_id, version_ids) = seed_skill_with_versions(&state.db, 2).await;
        delete_skill_version_core(&state.db, &version_ids[0])
            .await
            .unwrap();
        let q = format!(
            "SELECT meta::id(id) AS id FROM skill_version:`{}`",
            version_ids[0]
        );
        let rows = state.db.query_json(&q).await.unwrap();
        assert!(rows.is_empty(), "row should be gone after delete");
    }

    #[tokio::test]
    async fn test_delete_skill_version_refuses_last_remaining() {
        let (state, _g) = setup_test_state().await;
        let (_skill_id, version_ids) = seed_skill_with_versions(&state.db, 1).await;
        let err = delete_skill_version_core(&state.db, &version_ids[0])
            .await
            .unwrap_err();
        assert!(err.contains("last remaining"));
    }

    #[tokio::test]
    async fn test_delete_skill_version_not_found() {
        let (state, _g) = setup_test_state().await;
        let phantom = uuid::Uuid::new_v4().to_string();
        let err = delete_skill_version_core(&state.db, &phantom)
            .await
            .unwrap_err();
        assert!(err.contains("not found"));
    }

    #[tokio::test]
    async fn test_delete_skill_version_rejects_invalid_uuid() {
        let (state, _g) = setup_test_state().await;
        assert!(delete_skill_version_core(&state.db, "not-a-uuid")
            .await
            .is_err());
    }
}
