// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Shared helpers for import/export operations.

use crate::db::client::DBClient;
use crate::models::import_export::*;
use std::collections::HashMap;

/// Queries a single entity by ID using a parameterized bind.
///
/// Returns the first result row, or None if the entity doesn't exist.
pub async fn query_entity_by_id(
    db: &DBClient,
    query: &str,
    id: &str,
    entity_label: &str,
) -> Result<Option<serde_json::Value>, String> {
    let results: Vec<serde_json::Value> = db
        .db
        .query(query)
        .bind(("id", id.to_string()))
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .map_err(|e| format!("Failed to query {}: {}", entity_label, e))?;
    Ok(results.into_iter().next())
}

/// Applies MCP sanitization config to env and args from a DB row.
///
/// Returns the sanitized (env, args) pair.
pub fn apply_mcp_sanitization(
    row: &serde_json::Value,
    server_id: &str,
    sanitization: &HashMap<String, MCPSanitizationConfig>,
) -> (HashMap<String, String>, Vec<String>) {
    // Parse env from JSON string
    let env_str = row["env"].as_str().unwrap_or("{}");
    let mut env: HashMap<String, String> = serde_json::from_str(env_str).unwrap_or_default();

    // Apply sanitization to env
    if let Some(config) = sanitization.get(server_id) {
        for key in &config.clear_env_keys {
            if env.contains_key(key) {
                env.insert(key.clone(), String::new());
            }
        }
        for (key, value) in &config.modify_env {
            env.insert(key.clone(), value.clone());
        }
    }

    // Parse args with optional override from sanitization
    let extract_args = || -> Vec<String> {
        row["args"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };

    let args = if let Some(config) = sanitization.get(server_id) {
        if !config.modify_args.is_empty() {
            config.modify_args.clone()
        } else {
            extract_args()
        }
    } else {
        extract_args()
    };

    (env, args)
}

/// Extracts a timestamp field conditionally based on export options.
pub fn extract_optional_timestamp(
    row: &serde_json::Value,
    field: &str,
    include: bool,
) -> Option<String> {
    if include {
        row[field].as_str().map(String::from)
    } else {
        None
    }
}

/// Checks whether an entity with the given name already exists in the table.
///
/// Returns an `ImportConflict` if a name collision is found, None otherwise.
pub async fn check_name_conflict(
    db: &DBClient,
    table: &str,
    entity_type: &str,
    name: &str,
) -> Option<ImportConflict> {
    let query = format!(
        "SELECT meta::id(id) AS id FROM {} WHERE name = $name",
        table
    );
    let results: Vec<serde_json::Value> = db
        .db
        .query(&query)
        .bind(("name", name.to_string()))
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .unwrap_or_default();

    let existing = results.first()?;
    Some(ImportConflict {
        entity_type: entity_type.to_string(),
        entity_name: name.to_string(),
        existing_id: existing["id"].as_str().unwrap_or("").to_string(),
    })
}

/// Loads MCP server export summaries and extracts env keys for sanitization UI.
pub async fn load_mcp_preview(
    db: &DBClient,
    server_ids: &[String],
    mcp_summaries: &mut Vec<MCPServerExportSummary>,
    env_keys: &mut HashMap<String, Vec<String>>,
) -> Result<(), String> {
    for server_id in server_ids {
        let query = "SELECT meta::id(id) AS id, name, enabled, command, env, auth_type, extra_headers FROM mcp_server WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, server_id, "MCP server").await? {
            let id = row["id"].as_str().unwrap_or("").to_string();

            // v1.2 — auth metadata for the MCPFieldEditor section.
            let auth_type = crate::mcp::helpers::parse_auth_type(Some(&row["auth_type"]))
                .filter(|t| !matches!(t, crate::models::mcp::MCPAuthType::None));

            let extra_header_keys: Vec<String> =
                crate::mcp::helpers::parse_extra_headers_json(Some(&row["extra_headers"]))
                    .map(|map| map.into_keys().collect())
                    .unwrap_or_default();

            mcp_summaries.push(MCPServerExportSummary {
                id: Some(id.clone()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(false),
                command: row["command"].as_str().unwrap_or("").to_string(),
                tools_count: 0,
                auth_type,
                extra_header_keys,
            });

            let env_str = row["env"].as_str().unwrap_or("{}");
            if let Ok(env_map) = serde_json::from_str::<HashMap<String, String>>(env_str) {
                let keys: Vec<String> = env_map.keys().cloned().collect();
                if !keys.is_empty() {
                    env_keys.insert(id, keys);
                }
            }
        }
    }
    Ok(())
}

/// Mutable counters for tracking import progress across entity types.
pub struct ImportTracking<'a> {
    pub imported: &'a mut ImportCounts,
    pub skipped: &'a mut ImportCounts,
    pub errors: &'a mut Vec<ImportError>,
}

/// Result of resolving how to handle an entity during import.
pub enum ImportAction {
    /// Entity not in user selection (do not count as skipped).
    NotSelected,
    /// User chose to skip this conflict.
    Skipped,
    /// Entity should be imported with the given parameters.
    Import {
        id: String,
        name: String,
        is_overwrite: bool,
    },
}

/// Resolves the import action for a single entity: selection check, conflict
/// resolution lookup, existing-ID retrieval, and final name computation.
pub async fn resolve_import_entity(
    db: &DBClient,
    table: &str,
    entity_type_prefix: &str,
    entity_name: &str,
    selected_names: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
) -> ImportAction {
    if !selected_names.contains(&entity_name.to_string()) {
        return ImportAction::NotSelected;
    }

    let resolution_key = format!("{}:{}", entity_type_prefix, entity_name);
    let resolution = resolutions.get(&resolution_key).cloned();

    if resolution == Some(ConflictResolution::Skip) {
        return ImportAction::Skipped;
    }

    // For Overwrite, find the existing ID by name
    let existing_id = if resolution == Some(ConflictResolution::Overwrite) {
        let query = format!(
            "SELECT meta::id(id) AS id FROM {} WHERE name = $name",
            table
        );
        let results: Vec<serde_json::Value> = db
            .db
            .query(&query)
            .bind(("name", entity_name.to_string()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .unwrap_or_default();
        results
            .first()
            .and_then(|r| r["id"].as_str())
            .map(String::from)
    } else {
        None
    };

    let id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    let name = if resolution == Some(ConflictResolution::Rename) {
        format!("{} (imported)", entity_name)
    } else {
        entity_name.to_string()
    };

    ImportAction::Import {
        id,
        name,
        is_overwrite: resolution == Some(ConflictResolution::Overwrite),
    }
}

/// Persists an imported entity via CREATE or UPDATE, then sets timestamps.
pub async fn persist_imported_entity(
    db: &DBClient,
    table: &str,
    entity_id: &str,
    data: serde_json::Value,
    is_overwrite: bool,
) -> Result<(), String> {
    let query = if is_overwrite {
        format!("UPDATE {}:`{}` CONTENT $data", table, entity_id)
    } else {
        format!("CREATE {}:`{}` CONTENT $data", table, entity_id)
    };

    db.execute_with_params(&query, vec![("data".to_string(), data)])
        .await
        .map_err(|e| e.to_string())?;

    let ts_query = format!(
        "UPDATE {}:`{}` SET created_at = time::now(), updated_at = time::now()",
        table, entity_id
    );
    if let Err(e) = db.execute(&ts_query).await {
        tracing::warn!(
            table = %table,
            entity_id = %entity_id,
            error = %e,
            "Failed to set timestamps on imported entity"
        );
    }

    Ok(())
}
