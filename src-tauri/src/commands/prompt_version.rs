// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Prompt version snapshots: list / get / restore.
//!
//! A snapshot row is inserted in `prompt_version` BEFORE every `UPDATE prompt`.
//! Restore is implemented as a snapshot-then-update — never destroys history.

use crate::db::DBClient;
use crate::models::prompt_version::{PromptVersion, PromptVersionSummary};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::AppState;
use serde_json::json;
use tauri::State;
use tracing::{error, info, instrument};

const PROMPT_VERSION_FIELDS: &str = "meta::id(id) AS id, prompt_id, version, name, description, \
    category, content, variables_json, edited_by, edit_summary, edited_at";

const PROMPT_VERSION_SUMMARY_FIELDS: &str = "meta::id(id) AS id, prompt_id, version, edited_by, \
    edit_summary, edited_at";

/// Validates `edited_by`: either `"user"` or `"agent:<uuid>"`.
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

/// Validates `edit_summary`: optional, max 500 chars when present.
/// Required when `edited_by` starts with "agent:".
fn validate_edit_summary(
    edited_by: &str,
    edit_summary: &Option<String>,
) -> Result<Option<String>, String> {
    let is_agent = edited_by.starts_with("agent:");
    match edit_summary {
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                if is_agent {
                    return Err("edit_summary is required when edited_by is an agent".to_string());
                }
                return Ok(None);
            }
            if trimmed.len() > 500 {
                return Err("edit_summary exceeds 500 chars".to_string());
            }
            // Reject control chars (log injection vector).
            if trimmed.chars().any(|c| c.is_control() && c != ' ') {
                return Err("edit_summary contains control characters".to_string());
            }
            Ok(Some(trimmed.to_string()))
        }
        None => {
            if is_agent {
                Err("edit_summary is required when edited_by is an agent".to_string())
            } else {
                Ok(None)
            }
        }
    }
}

/// Snapshots the current state of `prompt_id` into `prompt_version` BEFORE an UPDATE.
///
/// Returns the new version number (monotonically increasing per prompt_id).
/// Caller must already have validated that the prompt exists.
pub async fn snapshot_prompt_version_core(
    db: &DBClient,
    prompt_id: &str,
    edited_by: &str,
    edit_summary: Option<String>,
) -> Result<i64, String> {
    let prompt_id = validate_uuid_field(prompt_id, "prompt_id")?;
    let edited_by = validate_edited_by(edited_by)?;
    let edit_summary = validate_edit_summary(&edited_by, &edit_summary)?;

    // Load current prompt state.
    let current_query = format!(
        "SELECT name, description, category, content, variables FROM prompt:`{}`",
        prompt_id
    );
    let rows = db
        .query_json(&current_query)
        .await
        .map_err(|e| format!("Failed to load prompt before snapshot: {}", e))?;
    let current = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Prompt not found: {}", prompt_id))?;

    let name = current["name"].as_str().unwrap_or("").to_string();
    let description = current["description"].as_str().unwrap_or("").to_string();
    let category = current["category"].as_str().unwrap_or("custom").to_string();
    let content = current["content"].as_str().unwrap_or("").to_string();
    let variables_json =
        serde_json::to_string(&current["variables"]).unwrap_or_else(|_| "[]".to_string());

    // Determine the next version number.
    let max_query = format!(
        "SELECT math::max(version) AS max_version FROM prompt_version WHERE prompt_id = '{}' GROUP ALL",
        prompt_id
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
        "CREATE prompt_version:`{id}` CONTENT {{
            id: '{id}',
            prompt_id: '{prompt_id}',
            version: {next_version},
            name: $name,
            description: $description,
            category: $category,
            content: $content,
            variables_json: $variables_json,
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
            ("variables_json".to_string(), json!(variables_json)),
            ("edited_by".to_string(), json!(edited_by)),
        ],
    )
    .await
    .map_err(|e| format!("Failed to insert prompt_version: {}", e))?;

    info!(prompt_id = %prompt_id, version = next_version, "Prompt version snapshot created");
    Ok(next_version)
}

pub async fn list_prompt_versions_core(
    db: &DBClient,
    prompt_id: &str,
) -> Result<Vec<PromptVersionSummary>, String> {
    let prompt_id = validate_uuid_field(prompt_id, "prompt_id")?;
    let query = format!(
        "SELECT {PROMPT_VERSION_SUMMARY_FIELDS} FROM prompt_version \
         WHERE prompt_id = '{prompt_id}' ORDER BY version DESC"
    );
    let rows = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to list prompt versions: {}", e))?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| format!("Deserialization failed: {}", e)))
        .collect()
}

pub async fn get_prompt_version_core(
    db: &DBClient,
    version_id: &str,
) -> Result<PromptVersion, String> {
    let version_id = validate_uuid_field(version_id, "version_id")?;
    let query = format!("SELECT {PROMPT_VERSION_FIELDS} FROM prompt_version:`{version_id}`");
    let rows = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to load prompt version: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| "Prompt version not found".to_string())?;
    serde_json::from_value(row).map_err(|e| format!("Failed to deserialize version: {}", e))
}

/// Restores `prompt_id` to the content of `version_id`.
///
/// Strategy: snapshot the CURRENT state first (so the restore is reversible),
/// then overwrite the prompt with the targeted version's payload.
pub async fn restore_prompt_version_core(
    db: &DBClient,
    prompt_id: &str,
    version_id: &str,
    edited_by: &str,
) -> Result<(), String> {
    let prompt_id = validate_uuid_field(prompt_id, "prompt_id")?;
    let target = get_prompt_version_core(db, version_id).await?;
    if target.prompt_id != prompt_id {
        return Err("Version does not belong to this prompt".to_string());
    }

    // 1. Snapshot the current state, with a restore-flavoured summary.
    let summary = Some(format!("Restore to version {}", target.version));
    snapshot_prompt_version_core(db, &prompt_id, edited_by, summary).await?;

    // 2. Overwrite the prompt with the target version's content.
    let name_json = serialize_for_query(&target.name, "name")?;
    let desc_json = serialize_for_query(&target.description, "description")?;
    let cat_json = serialize_for_query(&target.category, "category")?;
    let content_json = serialize_for_query(&target.content, "content")?;
    let variables: serde_json::Value =
        serde_json::from_str(&target.variables_json).unwrap_or_else(|_| json!([]));
    let variables_json = serialize_for_query(&variables, "variables")?;

    let query = format!(
        "UPDATE prompt:`{prompt_id}` SET name = {name_json}, description = {desc_json}, \
         category = {cat_json}, content = {content_json}, variables = {variables_json}, \
         updated_at = time::now()"
    );
    db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to restore prompt");
        format!("Failed to restore prompt: {}", e)
    })?;
    info!(prompt_id = %prompt_id, version = target.version, "Prompt restored");
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
#[instrument(name = "list_prompt_versions", skip(state), fields(prompt_id = %prompt_id))]
pub async fn list_prompt_versions(
    prompt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<PromptVersionSummary>, String> {
    list_prompt_versions_core(&state.db, &prompt_id).await
}

#[tauri::command]
#[instrument(name = "get_prompt_version", skip(state), fields(version_id = %version_id))]
pub async fn get_prompt_version(
    version_id: String,
    state: State<'_, AppState>,
) -> Result<PromptVersion, String> {
    get_prompt_version_core(&state.db, &version_id).await
}

#[tauri::command]
#[instrument(name = "restore_prompt_version", skip(state))]
pub async fn restore_prompt_version(
    prompt_id: String,
    version_id: String,
    edited_by: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let edited_by = edited_by.unwrap_or_else(|| "user".to_string());
    restore_prompt_version_core(&state.db, &prompt_id, &version_id, &edited_by).await
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
        assert!(validate_edit_summary(&id, &Some("".to_string())).is_err());
        assert!(validate_edit_summary(&id, &Some("ok".to_string())).is_ok());
    }

    #[test]
    fn test_edit_summary_optional_for_user() {
        assert!(validate_edit_summary("user", &None).unwrap().is_none());
        assert_eq!(
            validate_edit_summary("user", &Some("hi".to_string()))
                .unwrap()
                .as_deref(),
            Some("hi")
        );
    }

    #[test]
    fn test_edit_summary_too_long() {
        let big = "x".repeat(501);
        assert!(validate_edit_summary("user", &Some(big)).is_err());
    }

    #[test]
    fn test_edit_summary_rejects_control_chars() {
        assert!(validate_edit_summary("user", &Some("ok\nthere".to_string())).is_err());
    }
}
