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

/// Detects a model conflict by its real DB uniqueness key `(provider, api_name)`.
///
/// The `llm_model` UNIQUE index is `model_api_name_idx ON (provider, api_name)`,
/// NOT `name`. Detecting by `name` produces false negatives — a model whose
/// `name` differs but whose `(provider, api_name)` collides would pass
/// validation, then fail at CREATE on the index. This helper detects the real
/// collision while keeping the conflict keyed on `name` so the UI resolution key
/// (`model:{name}`) still matches.
///
/// # Returns
/// `Some(ImportConflict)` with `entity_name = name` and `existing_id` set to the
/// colliding row's id, or `None` if no `(provider, api_name)` row exists.
pub async fn check_model_conflict(
    db: &DBClient,
    provider: &str,
    api_name: &str,
    name: &str,
) -> Option<ImportConflict> {
    let query =
        "SELECT meta::id(id) AS id FROM llm_model WHERE provider = $provider AND api_name = $api_name";
    let results: Vec<serde_json::Value> = db
        .db
        .query(query)
        .bind(("provider", provider.to_string()))
        .bind(("api_name", api_name.to_string()))
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .unwrap_or_default();

    let existing = results.first()?;
    Some(ImportConflict {
        entity_type: "model".to_string(),
        entity_name: name.to_string(),
        existing_id: existing["id"].as_str().unwrap_or("").to_string(),
    })
}

/// Resolves the import action for a model, detecting conflicts by the real DB
/// uniqueness key `(provider, api_name)` rather than by `name`.
///
/// Mirrors [`resolve_import_entity`] but:
/// - **Overwrite** retrieves the existing id via `(provider, api_name)` (the
///   index), so the UPDATE targets the colliding row.
/// - **Rename** reuses the same charset-safe unique-name logic on `name`.
///
/// NOTE(import): a rename can NOT resolve a `(provider, api_name)` collision —
/// `api_name` is immutable on import, so the new row would still collide on the
/// index and CREATE would fail. The gain of this path is correct *detection*
/// (the user is warned) plus a correct Overwrite; renaming a model that collides
/// on `(provider, api_name)` is expected to surface that CREATE error.
pub async fn resolve_import_model(
    db: &DBClient,
    provider: &str,
    api_name: &str,
    entity_name: &str,
    selected_names: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
) -> ImportAction {
    if !selected_names.contains(&entity_name.to_string()) {
        return ImportAction::NotSelected;
    }

    let resolution_key = format!("model:{}", entity_name);
    let resolution = resolutions.get(&resolution_key).cloned();

    if resolution == Some(ConflictResolution::Skip) {
        return ImportAction::Skipped;
    }

    // Overwrite resolves the existing id by the real (provider, api_name) key.
    let existing_id = if resolution == Some(ConflictResolution::Overwrite) {
        let query = "SELECT meta::id(id) AS id FROM llm_model WHERE provider = $provider AND api_name = $api_name";
        let results: Vec<serde_json::Value> = db
            .db
            .query(query)
            .bind(("provider", provider.to_string()))
            .bind(("api_name", api_name.to_string()))
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
        resolve_unique_rename(db, "llm_model", entity_name).await
    } else {
        entity_name.to_string()
    };

    ImportAction::Import {
        id,
        name,
        is_overwrite: resolution == Some(ConflictResolution::Overwrite),
    }
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

    // NOTE(import): for custom_provider the name IS the primary key referenced by
    // agents and models; renaming it breaks those references that point to it by
    // name. The rename here only guarantees a non-colliding, valid name; callers
    // that care about cross-entity integrity must reconcile separately.
    let name = if resolution == Some(ConflictResolution::Rename) {
        resolve_unique_rename(db, table, entity_name).await
    } else {
        entity_name.to_string()
    };

    ImportAction::Import {
        id,
        name,
        is_overwrite: resolution == Some(ConflictResolution::Overwrite),
    }
}

/// Builds a charset-safe, length-bounded rename for an imported entity whose
/// name already exists.
///
/// The previous scheme — `format!("{} (imported)", name)` — produced names that
/// the skill regex (`^[a-zA-Z0-9_-]+$`) rejects (space + parentheses), could
/// exceed the 64-char column limit, and was not unique (a re-import collided on
/// the UNIQUE name index). This builder fixes all three:
///
/// - **Charset-safe**: appends only `-imported` (optionally `-imported-<n>`).
///   The hyphen and digits are valid for every name column, including skills.
/// - **Length-bounded**: the base is truncated *on a char boundary* (never a
///   byte slice) so the final name fits within `max_len`.
/// - **Suffix-driven uniqueness**: callers iterate `suffix_n` until the DB
///   reports the name free.
///
/// # Arguments
/// * `base` - The original entity name.
/// * `suffix_n` - `None` for the first attempt (`-imported`), `Some(n)` for the
///   nth retry (`-imported-<n>`).
/// * `max_len` - Maximum length (in characters) of the produced name.
///
/// # Returns
/// A name of at most `max_len` characters ending with the import suffix.
pub fn build_renamed_base(base: &str, suffix_n: Option<u32>, max_len: usize) -> String {
    let suffix = match suffix_n {
        None => "-imported".to_string(),
        Some(n) => format!("-imported-{}", n),
    };

    let suffix_len = suffix.chars().count();
    // If the suffix alone meets/exceeds the cap, the suffix wins (the base has
    // no room). This is a degenerate input but must never panic or overflow.
    if suffix_len >= max_len {
        return suffix.chars().take(max_len).collect();
    }

    let base_budget = max_len - suffix_len;
    let base_truncated: String = base.chars().take(base_budget).collect();
    format!("{}{}", base_truncated, suffix)
}

/// Maximum name length used when generating a unique rename. 64 is the most
/// restrictive name-column cap across import targets (agent, llm_model,
/// custom_provider); skills allow more, so 64 is always safe.
const RENAME_MAX_LEN: usize = 64;

/// Maximum number of numbered suffixes tried before falling back to a UUID
/// fragment for uniqueness.
const RENAME_MAX_ATTEMPTS: u32 = 100;

/// Resolves a unique, charset-safe, length-bounded rename for `base` in `table`.
///
/// Tries `base-imported`, then `base-imported-2`, `base-imported-3`, ... until
/// the name is free (no row with that name). After [`RENAME_MAX_ATTEMPTS`]
/// collisions it falls back to a UUID fragment suffix, which is charset-safe
/// (hex + hyphen) and effectively collision-free.
async fn resolve_unique_rename(db: &DBClient, table: &str, base: &str) -> String {
    // First attempt has no numeric suffix; subsequent attempts are numbered 2..N.
    let mut suffix_n: Option<u32> = None;
    let mut attempt: u32 = 0;

    loop {
        let candidate = build_renamed_base(base, suffix_n, RENAME_MAX_LEN);
        if !name_taken(db, table, &candidate).await {
            return candidate;
        }

        attempt += 1;
        if attempt >= RENAME_MAX_ATTEMPTS {
            break;
        }
        suffix_n = Some(attempt + 1);
    }

    // Fallback: UUID fragment (hex + hyphens) keeps the name charset-safe and
    // unique without another round-trip.
    let frag = uuid::Uuid::new_v4().to_string();
    let frag_short: String = frag.chars().take(8).collect();
    let budget = RENAME_MAX_LEN.saturating_sub(frag_short.chars().count() + 1);
    let base_truncated: String = base.chars().take(budget).collect();
    format!("{}-{}", base_truncated, frag_short)
}

/// Returns true if a row with the given name already exists in `table`.
async fn name_taken(db: &DBClient, table: &str, name: &str) -> bool {
    let query = format!(
        "SELECT count() AS c FROM {} WHERE name = $name GROUP ALL",
        table
    );
    let results: Vec<serde_json::Value> = db
        .db
        .query(&query)
        .bind(("name", name.to_string()))
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .unwrap_or_default();
    results.first().and_then(|r| r["c"].as_u64()).unwrap_or(0) > 0
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renamed_base_first_suffix_is_imported() {
        assert_eq!(build_renamed_base("agent", None, 64), "agent-imported");
    }

    #[test]
    fn renamed_base_numbered_suffix() {
        assert_eq!(build_renamed_base("agent", Some(2), 64), "agent-imported-2");
        assert_eq!(build_renamed_base("agent", Some(3), 64), "agent-imported-3");
    }

    #[test]
    fn renamed_base_is_charset_safe_for_skill_regex() {
        // The skill name regex is ^[a-zA-Z0-9_-]+$. The suffix must not introduce
        // spaces or parentheses (the previous "{} (imported)" scheme did).
        let name = build_renamed_base("my-skill_1", Some(7), 64);
        assert!(name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(name, "my-skill_1-imported-7");
    }

    #[test]
    fn renamed_base_truncates_base_to_fit_max_len() {
        // 64-char base + "-imported" would overflow; the base is truncated so the
        // final name fits exactly within max_len.
        let base = "a".repeat(64);
        let out = build_renamed_base(&base, None, 64);
        assert_eq!(out.chars().count(), 64);
        assert!(out.ends_with("-imported"));
        // base truncated to 64 - len("-imported") = 55 'a's.
        assert_eq!(out, format!("{}-imported", "a".repeat(55)));
    }

    #[test]
    fn renamed_base_truncates_for_numbered_suffix() {
        let base = "x".repeat(64);
        let out = build_renamed_base(&base, Some(12), 64);
        assert_eq!(out.chars().count(), 64);
        assert!(out.ends_with("-imported-12"));
    }

    #[test]
    fn renamed_base_truncates_multibyte_on_char_boundary() {
        // Multi-byte chars must be truncated on a char boundary (never a byte
        // slice that would panic). 30 accented chars + suffix > 32-char cap.
        let base = "é".repeat(30);
        let out = build_renamed_base(&base, None, 32);
        assert!(out.chars().count() <= 32);
        assert!(out.ends_with("-imported"));
        // No replacement char / corruption.
        assert!(!out.contains('\u{FFFD}'));
    }

    #[test]
    fn renamed_base_empty_base() {
        // Degenerate base still produces a usable suffixed name.
        assert_eq!(build_renamed_base("", None, 64), "-imported");
    }
}
