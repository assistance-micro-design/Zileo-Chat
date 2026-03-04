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

//! Import/Export Settings Commands
//!
//! Tauri commands for exporting and importing configuration entities.
//!
//! ## Export Commands
//! - `prepare_export_preview` - Get preview data for selected entities
//! - `generate_export_file` - Generate export JSON with sanitization applied
//!
//! ## Import Commands
//! - `validate_import` - Validate import file and detect conflicts
//! - `execute_import` - Execute import with conflict resolutions

use crate::db::client::DBClient;
use crate::db::sanitize_for_surrealdb;
use crate::models::import_export::*;
use crate::models::prompt::Prompt;
use crate::state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use tracing::instrument;

// ============================================================================
// Export Commands
// ============================================================================

/// Prepares export preview data for the selected entities.
///
/// Returns summaries of all selected entities plus MCP env var keys
/// for the sanitization UI.
///
/// # Arguments
/// * `selection` - IDs of entities to include in export
/// * `state` - Application state
///
/// # Returns
/// Export preview data with entity summaries and MCP env keys
#[tauri::command]
#[instrument(name = "prepare_export_preview", skip(state))]
pub async fn prepare_export_preview(
    selection: ExportSelection,
    state: State<'_, AppState>,
) -> Result<ExportPreviewData, String> {
    tracing::info!(
        agents = selection.agents.len(),
        mcp_servers = selection.mcp_servers.len(),
        models = selection.models.len(),
        prompts = selection.prompts.len(),
        "Preparing export preview"
    );

    if selection.is_empty() {
        return Err("At least one entity must be selected for export".to_string());
    }

    let mut preview = ExportPreviewData {
        agents: Vec::new(),
        mcp_servers: Vec::new(),
        models: Vec::new(),
        prompts: Vec::new(),
        mcp_env_keys: HashMap::new(),
    };

    // Load agent summaries
    for agent_id in &selection.agents {
        let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers FROM agent WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(&state.db, query, agent_id, "agent").await? {
            let llm = &row["llm"];
            preview.agents.push(AgentExportSummary {
                id: Some(row["id"].as_str().unwrap_or("").to_string()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                lifecycle: row["lifecycle"].as_str().unwrap_or("permanent").to_string(),
                provider: llm["provider"].as_str().unwrap_or("").to_string(),
                model: llm["model"].as_str().unwrap_or("").to_string(),
                tools_count: row["tools"].as_array().map(|a| a.len()).unwrap_or(0),
                mcp_servers_count: row["mcp_servers"].as_array().map(|a| a.len()).unwrap_or(0),
            });
        }
    }

    // Load MCP server summaries and env keys
    load_mcp_preview(
        &state.db,
        &selection.mcp_servers,
        &mut preview.mcp_servers,
        &mut preview.mcp_env_keys,
    )
    .await?;

    // Load model summaries
    for model_id in &selection.models {
        let query = "SELECT meta::id(id) AS id, name, provider, api_name, is_builtin FROM llm_model WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(&state.db, query, model_id, "model").await? {
            preview.models.push(LLMModelExportSummary {
                id: Some(row["id"].as_str().unwrap_or("").to_string()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                provider: row["provider"].as_str().unwrap_or("").to_string(),
                api_name: row["api_name"].as_str().unwrap_or("").to_string(),
                is_builtin: row["is_builtin"].as_bool().unwrap_or(false),
            });
        }
    }

    // Load prompt summaries
    for prompt_id in &selection.prompts {
        let query = "SELECT meta::id(id) AS id, name, description, category, content FROM prompt WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(&state.db, query, prompt_id, "prompt").await? {
            let content = row["content"].as_str().unwrap_or("");
            let var_count = content.matches("{{").count();
            preview.prompts.push(PromptExportSummary {
                id: Some(row["id"].as_str().unwrap_or("").to_string()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                description: row["description"].as_str().unwrap_or("").to_string(),
                category: row["category"].as_str().unwrap_or("custom").to_string(),
                variables_count: var_count,
            });
        }
    }

    tracing::info!(
        agents = preview.agents.len(),
        mcp_servers = preview.mcp_servers.len(),
        models = preview.models.len(),
        prompts = preview.prompts.len(),
        "Export preview prepared"
    );

    Ok(preview)
}

/// Generates the export file content with optional MCP sanitization.
///
/// # Arguments
/// * `selection` - IDs of entities to include
/// * `options` - Export options (format, timestamps, sanitize)
/// * `sanitization` - MCP sanitization config per server
/// * `state` - Application state
///
/// # Returns
/// JSON string ready for file download
#[tauri::command]
#[instrument(name = "generate_export_file", skip(state, sanitization))]
pub async fn generate_export_file(
    selection: ExportSelection,
    options: ExportOptions,
    sanitization: HashMap<String, MCPSanitizationConfig>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    tracing::info!(
        total = selection.total_count(),
        include_timestamps = options.include_timestamps,
        "Generating export file"
    );

    if selection.is_empty() {
        return Err("At least one entity must be selected for export".to_string());
    }

    let ts = options.include_timestamps;
    let agents = export_agents(&state.db, &selection.agents, ts).await?;
    let mcp_servers =
        export_mcp_servers(&state.db, &selection.mcp_servers, ts, &sanitization).await?;
    let models = export_models(&state.db, &selection.models, ts).await?;
    let prompts = export_prompts(&state.db, &selection.prompts, ts).await?;

    // Build export package
    let package = ExportPackage::new(agents, mcp_servers, models, prompts, None);

    // Serialize to JSON
    let json = serde_json::to_string_pretty(&package)
        .map_err(|e| format!("Failed to serialize export: {}", e))?;

    tracing::info!(
        agents = package.manifest.counts.agents,
        mcp_servers = package.manifest.counts.mcp_servers,
        models = package.manifest.counts.models,
        prompts = package.manifest.counts.prompts,
        size_bytes = json.len(),
        "Export file generated"
    );

    Ok(json)
}

// ============================================================================
// Import Commands
// ============================================================================

/// Validates an import file and detects conflicts with existing entities.
///
/// # Arguments
/// * `data` - JSON string from the import file
/// * `state` - Application state
///
/// # Returns
/// Validation result with entities, conflicts, and warnings
#[tauri::command]
#[instrument(name = "validate_import", skip(state, data))]
pub async fn validate_import(
    data: String,
    state: State<'_, AppState>,
) -> Result<ImportValidation, String> {
    tracing::info!(size_bytes = data.len(), "Validating import file");

    // Check file size
    if data.len() > MAX_IMPORT_FILE_SIZE {
        return Ok(ImportValidation::invalid(vec![format!(
            "File size ({} bytes) exceeds maximum ({} bytes)",
            data.len(),
            MAX_IMPORT_FILE_SIZE
        )]));
    }

    // Parse JSON
    let package: ExportPackage = match serde_json::from_str(&data) {
        Ok(p) => p,
        Err(e) => {
            return Ok(ImportValidation::invalid(vec![format!(
                "Invalid JSON format: {}",
                e
            )]));
        }
    };

    // Check schema version
    if package.manifest.version != EXPORT_SCHEMA_VERSION {
        return Ok(ImportValidation::invalid(vec![format!(
            "Unsupported schema version: {} (expected {})",
            package.manifest.version, EXPORT_SCHEMA_VERSION
        )]));
    }

    // Check total entity count to prevent DoS via huge import files
    let total_entities = package.agents.len()
        + package.mcp_servers.len()
        + package.models.len()
        + package.prompts.len();
    if total_entities > crate::db::utils::MAX_IMPORT_ENTITIES {
        return Ok(ImportValidation::invalid(vec![format!(
            "Import contains {} entities, exceeds maximum of {}",
            total_entities,
            crate::db::utils::MAX_IMPORT_ENTITIES
        )]));
    }

    let mut warnings = Vec::new();
    let mut conflicts = Vec::new();
    let mut missing_mcp_env = HashMap::new();

    let agent_summaries = validate_agents(&state.db, &package.agents, &mut conflicts).await;
    let mcp_summaries = validate_mcp_servers(
        &state.db,
        &package.mcp_servers,
        &mut conflicts,
        &mut missing_mcp_env,
    )
    .await;
    let model_summaries =
        validate_models(&state.db, &package.models, &mut conflicts, &mut warnings).await;
    let prompt_summaries = validate_prompts(&state.db, &package.prompts, &mut conflicts).await;

    tracing::info!(
        agents = agent_summaries.len(),
        mcp_servers = mcp_summaries.len(),
        models = model_summaries.len(),
        prompts = prompt_summaries.len(),
        conflicts = conflicts.len(),
        warnings = warnings.len(),
        "Import validation complete"
    );

    Ok(ImportValidation {
        valid: true,
        schema_version: package.manifest.version,
        errors: Vec::new(),
        warnings,
        entities: ImportEntities {
            agents: agent_summaries,
            mcp_servers: mcp_summaries,
            models: model_summaries,
            prompts: prompt_summaries,
        },
        conflicts,
        missing_mcp_env,
    })
}

/// Executes the import with conflict resolutions applied.
///
/// # Arguments
/// * `data` - JSON string from the import file
/// * `selection` - IDs of entities to import
/// * `resolutions` - Conflict resolutions per entity ID
/// * `mcp_additions` - Additional env vars/args for MCP servers
/// * `state` - Application state
///
/// # Returns
/// Import result with counts and errors
#[tauri::command]
#[instrument(name = "execute_import", skip(state, data, resolutions, mcp_additions))]
pub async fn execute_import(
    data: String,
    selection: ImportSelection,
    resolutions: HashMap<String, ConflictResolution>,
    mcp_additions: HashMap<String, MCPAdditions>,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    tracing::info!(
        agents = selection.agents.len(),
        mcp_servers = selection.mcp_servers.len(),
        models = selection.models.len(),
        prompts = selection.prompts.len(),
        "Executing import"
    );

    // Parse package
    let package: ExportPackage =
        serde_json::from_str(&data).map_err(|e| format!("Invalid JSON: {}", e))?;

    let mut imported = ImportCounts::default();
    let mut skipped = ImportCounts::default();
    let mut errors = Vec::new();

    let mut tracking = ImportTracking {
        imported: &mut imported,
        skipped: &mut skipped,
        errors: &mut errors,
    };
    import_agents(
        &state.db,
        &package.agents,
        &selection.agents,
        &resolutions,
        &mut tracking,
    )
    .await;
    import_mcp_servers(
        &state.db,
        &package.mcp_servers,
        &selection.mcp_servers,
        &resolutions,
        &mcp_additions,
        &mut tracking,
    )
    .await;
    import_models(
        &state.db,
        &package.models,
        &selection.models,
        &resolutions,
        &mut tracking,
    )
    .await;
    import_prompts(
        &state.db,
        &package.prompts,
        &selection.prompts,
        &resolutions,
        &mut tracking,
    )
    .await;

    let success = errors.is_empty();

    tracing::info!(
        success = success,
        imported_agents = imported.agents,
        imported_mcp = imported.mcp_servers,
        imported_models = imported.models,
        imported_prompts = imported.prompts,
        skipped_total = skipped.agents + skipped.mcp_servers + skipped.models + skipped.prompts,
        errors = errors.len(),
        "Import execution complete"
    );

    Ok(ImportResult {
        success,
        imported,
        skipped,
        errors,
    })
}

// ============================================================================
// File Operations
// ============================================================================

/// Saves export content to a file at the specified path.
///
/// # Arguments
/// * `path` - Full path to save the file (must end with .json, no path traversal)
/// * `content` - JSON content to write
///
/// # Returns
/// Number of bytes written
///
/// # Errors
/// Returns error if path contains traversal sequences, points to system directories,
/// or does not end with .json or .csv extension.
#[tauri::command]
#[instrument(name = "save_export_to_file", skip(content))]
pub async fn save_export_to_file(path: String, content: String) -> Result<usize, String> {
    let path = PathBuf::from(&path);

    // Validate path: reject traversal sequences
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err("Invalid path: path traversal ('..') is not allowed".to_string());
    }

    // Validate path: reject system directories
    let forbidden_prefixes = ["/etc", "/sys", "/proc", "/dev"];
    for prefix in &forbidden_prefixes {
        if path_str.starts_with(prefix) {
            return Err(format!(
                "Invalid path: writing to system directory '{}' is not allowed",
                prefix
            ));
        }
    }

    // Validate path: must end with .json or .csv
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("json") | Some("csv") => {}
        _ => {
            return Err("Invalid path: export file must have .json or .csv extension".to_string());
        }
    }

    tracing::info!(
        path = %path.display(),
        size_bytes = content.len(),
        "Saving export file"
    );

    std::fs::write(&path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

    tracing::info!(path = %path.display(), "Export file saved successfully");

    Ok(content.len())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Loads MCP server export summaries and extracts env keys for sanitization UI.
async fn load_mcp_preview(
    db: &DBClient,
    server_ids: &[String],
    mcp_summaries: &mut Vec<MCPServerExportSummary>,
    env_keys: &mut HashMap<String, Vec<String>>,
) -> Result<(), String> {
    for server_id in server_ids {
        let query = "SELECT meta::id(id) AS id, name, enabled, command, env FROM mcp_server WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, server_id, "MCP server").await? {
            let id = row["id"].as_str().unwrap_or("").to_string();
            mcp_summaries.push(MCPServerExportSummary {
                id: Some(id.clone()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(false),
                command: row["command"].as_str().unwrap_or("").to_string(),
                tools_count: 0,
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

/// Queries a single entity by ID using a parameterized bind.
///
/// Returns the first result row, or None if the entity doesn't exist.
async fn query_entity_by_id(
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
fn apply_mcp_sanitization(
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
fn extract_optional_timestamp(
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
async fn check_name_conflict(
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

/// Mutable counters for tracking import progress across entity types.
struct ImportTracking<'a> {
    imported: &'a mut ImportCounts,
    skipped: &'a mut ImportCounts,
    errors: &'a mut Vec<ImportError>,
}

/// Result of resolving how to handle an entity during import.
enum ImportAction {
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
async fn resolve_import_entity(
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

// ============================================================================
// Per-Entity Export Helpers
// ============================================================================

/// Exports agent entities from the database.
async fn export_agents(
    db: &DBClient,
    agent_ids: &[String],
    include_timestamps: bool,
) -> Result<Vec<AgentExportData>, String> {
    let mut agents = Vec::new();
    for agent_id in agent_ids {
        let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, skills, system_prompt, max_tool_iterations, reasoning_effort, created_at, updated_at FROM agent WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, agent_id, "agent").await? {
            let llm = &row["llm"];
            agents.push(AgentExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                lifecycle: row["lifecycle"].as_str().unwrap_or("permanent").to_string(),
                llm: LLMConfigExport {
                    provider: llm["provider"].as_str().unwrap_or("").to_string(),
                    model: llm["model"].as_str().unwrap_or("").to_string(),
                    temperature: llm["temperature"].as_f64().unwrap_or(0.7) as f32,
                    max_tokens: llm["max_tokens"].as_u64().unwrap_or(4096) as usize,
                },
                tools: row["tools"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                mcp_servers: row["mcp_servers"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                skills: row["skills"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                system_prompt: row["system_prompt"].as_str().unwrap_or("").to_string(),
                max_tool_iterations: row["max_tool_iterations"].as_u64().unwrap_or(50) as usize,
                reasoning_effort: row["reasoning_effort"].as_str().and_then(|s| {
                    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
                }),
                created_at: extract_optional_timestamp(&row, "created_at", include_timestamps),
                updated_at: extract_optional_timestamp(&row, "updated_at", include_timestamps),
            });
        }
    }
    Ok(agents)
}

/// Exports MCP server entities with sanitization applied.
async fn export_mcp_servers(
    db: &DBClient,
    server_ids: &[String],
    include_timestamps: bool,
    sanitization: &HashMap<String, MCPSanitizationConfig>,
) -> Result<Vec<MCPServerExportData>, String> {
    let mut servers = Vec::new();
    for server_id in server_ids {
        if let Some(config) = sanitization.get(server_id) {
            if config.exclude_from_export {
                continue;
            }
        }
        let query = "SELECT meta::id(id) AS id, name, enabled, command, args, env, description, created_at, updated_at FROM mcp_server WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, server_id, "MCP server").await? {
            let (env, args) = apply_mcp_sanitization(&row, server_id, sanitization);
            servers.push(MCPServerExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(false),
                command: row["command"].as_str().unwrap_or("").to_string(),
                args,
                env,
                description: row["description"].as_str().map(String::from),
                created_at: extract_optional_timestamp(&row, "created_at", include_timestamps),
                updated_at: extract_optional_timestamp(&row, "updated_at", include_timestamps),
            });
        }
    }
    Ok(servers)
}

/// Exports model entities from the database.
async fn export_models(
    db: &DBClient,
    model_ids: &[String],
    include_timestamps: bool,
) -> Result<Vec<LLMModelExportData>, String> {
    let mut models = Vec::new();
    for model_id in model_ids {
        let query = "SELECT meta::id(id) AS id, provider, name, api_name, context_window, max_output_tokens, temperature_default, is_builtin, is_reasoning, input_price_per_mtok, output_price_per_mtok, (cache_read_price_per_mtok ?? 0.0) AS cache_read_price_per_mtok, (cache_write_price_per_mtok ?? 0.0) AS cache_write_price_per_mtok, created_at, updated_at FROM llm_model WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, model_id, "model").await? {
            models.push(LLMModelExportData {
                provider: row["provider"].as_str().unwrap_or("").to_string(),
                name: row["name"].as_str().unwrap_or("").to_string(),
                api_name: row["api_name"].as_str().unwrap_or("").to_string(),
                context_window: row["context_window"].as_u64().unwrap_or(0) as usize,
                max_output_tokens: row["max_output_tokens"].as_u64().unwrap_or(0) as usize,
                temperature_default: row["temperature_default"].as_f64().unwrap_or(0.7) as f32,
                is_builtin: row["is_builtin"].as_bool().unwrap_or(false),
                is_reasoning: row["is_reasoning"].as_bool().unwrap_or(false),
                input_price_per_mtok: row["input_price_per_mtok"].as_f64().unwrap_or(0.0),
                output_price_per_mtok: row["output_price_per_mtok"].as_f64().unwrap_or(0.0),
                cache_read_price_per_mtok: row["cache_read_price_per_mtok"].as_f64().unwrap_or(0.0),
                cache_write_price_per_mtok: row["cache_write_price_per_mtok"]
                    .as_f64()
                    .unwrap_or(0.0),
                created_at: extract_optional_timestamp(&row, "created_at", include_timestamps),
                updated_at: extract_optional_timestamp(&row, "updated_at", include_timestamps),
            });
        }
    }
    Ok(models)
}

/// Exports prompt entities from the database.
async fn export_prompts(
    db: &DBClient,
    prompt_ids: &[String],
    include_timestamps: bool,
) -> Result<Vec<PromptExportData>, String> {
    let mut prompts = Vec::new();
    for prompt_id in prompt_ids {
        let query = "SELECT meta::id(id) AS id, name, description, category, content, created_at, updated_at FROM prompt WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, prompt_id, "prompt").await? {
            prompts.push(PromptExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                description: row["description"].as_str().unwrap_or("").to_string(),
                category: row["category"].as_str().unwrap_or("custom").to_string(),
                content: row["content"].as_str().unwrap_or("").to_string(),
                created_at: extract_optional_timestamp(&row, "created_at", include_timestamps),
                updated_at: extract_optional_timestamp(&row, "updated_at", include_timestamps),
            });
        }
    }
    Ok(prompts)
}

// ============================================================================
// Per-Entity Validation Helpers
// ============================================================================

/// Builds agent summaries and detects name conflicts.
async fn validate_agents(
    db: &DBClient,
    agents: &[AgentExportData],
    conflicts: &mut Vec<ImportConflict>,
) -> Vec<AgentExportSummary> {
    let mut summaries = Vec::new();
    for agent in agents {
        summaries.push(AgentExportSummary {
            id: None,
            name: agent.name.clone(),
            lifecycle: agent.lifecycle.clone(),
            provider: agent.llm.provider.clone(),
            model: agent.llm.model.clone(),
            tools_count: agent.tools.len(),
            mcp_servers_count: agent.mcp_servers.len(),
        });
        if let Some(conflict) = check_name_conflict(db, "agent", "agent", &agent.name).await {
            conflicts.push(conflict);
        }
    }
    summaries
}

/// Builds MCP server summaries, detects conflicts, and checks for missing env keys.
async fn validate_mcp_servers(
    db: &DBClient,
    servers: &[MCPServerExportData],
    conflicts: &mut Vec<ImportConflict>,
    missing_mcp_env: &mut HashMap<String, Vec<String>>,
) -> Vec<MCPServerExportSummary> {
    let mut summaries = Vec::new();
    for server in servers {
        summaries.push(MCPServerExportSummary {
            id: None,
            name: server.name.clone(),
            enabled: server.enabled,
            command: server.command.clone(),
            tools_count: 0,
        });

        let missing_keys: Vec<String> = server
            .env
            .iter()
            .filter(|(key, value)| is_sensitive_env_key(key) && value.is_empty())
            .map(|(key, _)| key.clone())
            .collect();
        if !missing_keys.is_empty() {
            missing_mcp_env.insert(server.name.clone(), missing_keys);
        }

        if let Some(conflict) = check_name_conflict(db, "mcp_server", "mcp", &server.name).await {
            conflicts.push(conflict);
        }
    }
    summaries
}

/// Builds model summaries, detects conflicts, and warns about builtins.
async fn validate_models(
    db: &DBClient,
    models: &[LLMModelExportData],
    conflicts: &mut Vec<ImportConflict>,
    warnings: &mut Vec<String>,
) -> Vec<LLMModelExportSummary> {
    let mut summaries = Vec::new();
    for model in models {
        summaries.push(LLMModelExportSummary {
            id: None,
            name: model.name.clone(),
            provider: model.provider.clone(),
            api_name: model.api_name.clone(),
            is_builtin: model.is_builtin,
        });
        if model.is_builtin {
            warnings.push(format!(
                "Model '{}' is a builtin model and may conflict with system defaults",
                model.name
            ));
        }
        if let Some(conflict) = check_name_conflict(db, "llm_model", "model", &model.name).await {
            conflicts.push(conflict);
        }
    }
    summaries
}

/// Builds prompt summaries and detects name conflicts.
async fn validate_prompts(
    db: &DBClient,
    prompts: &[PromptExportData],
    conflicts: &mut Vec<ImportConflict>,
) -> Vec<PromptExportSummary> {
    let mut summaries = Vec::new();
    for prompt in prompts {
        let var_count = prompt.content.matches("{{").count();
        summaries.push(PromptExportSummary {
            id: None,
            name: prompt.name.clone(),
            description: prompt.description.clone(),
            category: prompt.category.clone(),
            variables_count: var_count,
        });
        if let Some(conflict) = check_name_conflict(db, "prompt", "prompt", &prompt.name).await {
            conflicts.push(conflict);
        }
    }
    summaries
}

// ============================================================================
// Per-Entity Import Helpers
// ============================================================================

/// Imports agent entities with conflict resolution.
async fn import_agents(
    db: &DBClient,
    agents: &[AgentExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for agent in agents {
        match resolve_import_entity(db, "agent", "agent", &agent.name, selected, resolutions).await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.agents += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "lifecycle": agent.lifecycle,
                    "llm": {
                        "provider": agent.llm.provider,
                        "model": agent.llm.model,
                        "temperature": agent.llm.temperature,
                        "max_tokens": agent.llm.max_tokens,
                    },
                    "tools": agent.tools,
                    "mcp_servers": agent.mcp_servers,
                    "skills": agent.skills,
                    "system_prompt": agent.system_prompt,
                    "max_tool_iterations": agent.max_tool_iterations,
                    "reasoning_effort": agent.reasoning_effort,
                }));
                match persist_imported_entity(db, "agent", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.agents += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "agent".to_string(),
                        entity_id: agent.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports MCP server entities with conflict resolution and env additions.
async fn import_mcp_servers(
    db: &DBClient,
    servers: &[MCPServerExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    mcp_additions: &HashMap<String, MCPAdditions>,
    t: &mut ImportTracking<'_>,
) {
    for server in servers {
        match resolve_import_entity(db, "mcp_server", "mcp", &server.name, selected, resolutions)
            .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.mcp_servers += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let mut env = server.env.clone();
                if let Some(additions) = mcp_additions.get(&server.name) {
                    for (key, value) in &additions.add_env {
                        env.insert(key.clone(), value.clone());
                    }
                }
                let env_str = serde_json::to_string(&env).unwrap_or_else(|_| "{}".to_string());
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "enabled": server.enabled,
                    "command": server.command,
                    "args": server.args,
                    "env": env_str,
                    "description": server.description,
                }));
                match persist_imported_entity(db, "mcp_server", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.mcp_servers += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "mcp".to_string(),
                        entity_id: server.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports model entities with conflict resolution.
async fn import_models(
    db: &DBClient,
    models: &[LLMModelExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for model in models {
        match resolve_import_entity(db, "llm_model", "model", &model.name, selected, resolutions)
            .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.models += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "provider": model.provider,
                    "name": name,
                    "api_name": model.api_name,
                    "context_window": model.context_window,
                    "max_output_tokens": model.max_output_tokens,
                    "temperature_default": model.temperature_default,
                    "is_builtin": model.is_builtin,
                    "is_reasoning": model.is_reasoning,
                    "input_price_per_mtok": model.input_price_per_mtok,
                    "output_price_per_mtok": model.output_price_per_mtok,
                    "cache_read_price_per_mtok": model.cache_read_price_per_mtok,
                    "cache_write_price_per_mtok": model.cache_write_price_per_mtok,
                }));
                match persist_imported_entity(db, "llm_model", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.models += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "model".to_string(),
                        entity_id: model.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports prompt entities with conflict resolution.
async fn import_prompts(
    db: &DBClient,
    prompts: &[PromptExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for prompt in prompts {
        match resolve_import_entity(db, "prompt", "prompt", &prompt.name, selected, resolutions)
            .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.prompts += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let variables = Prompt::detect_variables(&prompt.content);
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "description": prompt.description,
                    "category": prompt.category,
                    "content": prompt.content,
                    "variables": variables,
                }));
                match persist_imported_entity(db, "prompt", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.prompts += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "prompt".to_string(),
                        entity_id: prompt.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Persists an imported entity via CREATE or UPDATE, then sets timestamps.
async fn persist_imported_entity(
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
