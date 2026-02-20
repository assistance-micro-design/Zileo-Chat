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
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", agent_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query agent: {}", e))?;

        if let Some(row) = results.first() {
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
    for server_id in &selection.mcp_servers {
        let query = "SELECT meta::id(id) AS id, name, enabled, command, env FROM mcp_server WHERE meta::id(id) = $id";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", server_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query MCP server: {}", e))?;

        if let Some(row) = results.first() {
            let id = row["id"].as_str().unwrap_or("").to_string();
            preview.mcp_servers.push(MCPServerExportSummary {
                id: Some(id.clone()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(false),
                command: row["command"].as_str().unwrap_or("").to_string(),
                tools_count: 0, // Tools are runtime, not stored in DB
            });

            // Extract env keys for sanitization UI
            // env is stored as JSON string in DB
            let env_str = row["env"].as_str().unwrap_or("{}");
            if let Ok(env_map) = serde_json::from_str::<HashMap<String, String>>(env_str) {
                let keys: Vec<String> = env_map.keys().cloned().collect();
                if !keys.is_empty() {
                    preview.mcp_env_keys.insert(id, keys);
                }
            }
        }
    }

    // Load model summaries
    for model_id in &selection.models {
        let query = "SELECT meta::id(id) AS id, name, provider, api_name, is_builtin FROM llm_model WHERE meta::id(id) = $id";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", model_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query model: {}", e))?;

        if let Some(row) = results.first() {
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
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", prompt_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query prompt: {}", e))?;

        if let Some(row) = results.first() {
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

    let mut agents = Vec::new();
    let mut mcp_servers = Vec::new();
    let mut models = Vec::new();
    let mut prompts = Vec::new();

    // Export agents
    for agent_id in &selection.agents {
        let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, system_prompt, max_tool_iterations, enable_thinking, created_at, updated_at FROM agent WHERE meta::id(id) = $id";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", agent_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query agent: {}", e))?;

        if let Some(row) = results.first() {
            let llm = &row["llm"];
            // Note: ID is NOT exported - entities are identified by name
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
                system_prompt: row["system_prompt"].as_str().unwrap_or("").to_string(),
                max_tool_iterations: row["max_tool_iterations"].as_u64().unwrap_or(50) as usize,
                enable_thinking: row["enable_thinking"].as_bool().unwrap_or(true),
                created_at: if options.include_timestamps {
                    row["created_at"].as_str().map(String::from)
                } else {
                    None
                },
                updated_at: if options.include_timestamps {
                    row["updated_at"].as_str().map(String::from)
                } else {
                    None
                },
            });
        }
    }

    // Export MCP servers with sanitization
    for server_id in &selection.mcp_servers {
        // Check if server should be excluded
        if let Some(config) = sanitization.get(server_id) {
            if config.exclude_from_export {
                continue;
            }
        }

        let query = "SELECT meta::id(id) AS id, name, enabled, command, args, env, description, created_at, updated_at FROM mcp_server WHERE meta::id(id) = $id";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", server_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query MCP server: {}", e))?;

        if let Some(row) = results.first() {
            // Parse env from JSON string
            let env_str = row["env"].as_str().unwrap_or("{}");
            let mut env: HashMap<String, String> =
                serde_json::from_str(env_str).unwrap_or_default();

            // Apply sanitization
            if let Some(config) = sanitization.get(server_id) {
                // Clear specified keys
                for key in &config.clear_env_keys {
                    if env.contains_key(key) {
                        env.insert(key.clone(), String::new());
                    }
                }
                // Apply modifications
                for (key, value) in &config.modify_env {
                    env.insert(key.clone(), value.clone());
                }
            }

            let args: Vec<String> = if let Some(config) = sanitization.get(server_id) {
                if !config.modify_args.is_empty() {
                    config.modify_args.clone()
                } else {
                    row["args"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default()
                }
            } else {
                row["args"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default()
            };

            // Note: ID is NOT exported - entities are identified by name
            mcp_servers.push(MCPServerExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(false),
                command: row["command"].as_str().unwrap_or("").to_string(),
                args,
                env,
                description: row["description"].as_str().map(String::from),
                created_at: if options.include_timestamps {
                    row["created_at"].as_str().map(String::from)
                } else {
                    None
                },
                updated_at: if options.include_timestamps {
                    row["updated_at"].as_str().map(String::from)
                } else {
                    None
                },
            });
        }
    }

    // Export models
    for model_id in &selection.models {
        let query = "SELECT meta::id(id) AS id, provider, name, api_name, context_window, max_output_tokens, temperature_default, is_builtin, is_reasoning, input_price_per_mtok, output_price_per_mtok, created_at, updated_at FROM llm_model WHERE meta::id(id) = $id";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", model_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query model: {}", e))?;

        if let Some(row) = results.first() {
            // Note: ID is NOT exported - entities are identified by name
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
                created_at: if options.include_timestamps {
                    row["created_at"].as_str().map(String::from)
                } else {
                    None
                },
                updated_at: if options.include_timestamps {
                    row["updated_at"].as_str().map(String::from)
                } else {
                    None
                },
            });
        }
    }

    // Export prompts
    for prompt_id in &selection.prompts {
        let query = "SELECT meta::id(id) AS id, name, description, category, content, created_at, updated_at FROM prompt WHERE meta::id(id) = $id";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("id", prompt_id.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query prompt: {}", e))?;

        if let Some(row) = results.first() {
            // Note: ID is NOT exported - entities are identified by name
            prompts.push(PromptExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                description: row["description"].as_str().unwrap_or("").to_string(),
                category: row["category"].as_str().unwrap_or("custom").to_string(),
                content: row["content"].as_str().unwrap_or("").to_string(),
                created_at: if options.include_timestamps {
                    row["created_at"].as_str().map(String::from)
                } else {
                    None
                },
                updated_at: if options.include_timestamps {
                    row["updated_at"].as_str().map(String::from)
                } else {
                    None
                },
            });
        }
    }

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

    // Build entity summaries
    let mut agent_summaries = Vec::new();
    let mut mcp_summaries = Vec::new();
    let mut model_summaries = Vec::new();
    let mut prompt_summaries = Vec::new();

    // Check agent conflicts - by NAME only (IDs are not in the export file)
    for agent in &package.agents {
        agent_summaries.push(AgentExportSummary {
            id: None, // No ID in import file
            name: agent.name.clone(),
            lifecycle: agent.lifecycle.clone(),
            provider: agent.llm.provider.clone(),
            model: agent.llm.model.clone(),
            tools_count: agent.tools.len(),
            mcp_servers_count: agent.mcp_servers.len(),
        });

        // Check for name conflict - this is the ONLY conflict check
        let name_query = "SELECT meta::id(id) AS id FROM agent WHERE name = $name";
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(name_query)
            .bind(("name", agent.name.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .unwrap_or_default();

        if !name_results.is_empty() {
            let existing = &name_results[0];
            conflicts.push(ImportConflict {
                entity_type: "agent".to_string(),
                entity_name: agent.name.clone(),
                existing_id: existing["id"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    // Check MCP server conflicts - by NAME only (IDs are not in the export file)
    for server in &package.mcp_servers {
        mcp_summaries.push(MCPServerExportSummary {
            id: None, // No ID in import file
            name: server.name.clone(),
            enabled: server.enabled,
            command: server.command.clone(),
            tools_count: 0,
        });

        // Check for sensitive env vars with empty values
        let mut missing_keys = Vec::new();
        for (key, value) in &server.env {
            if is_sensitive_env_key(key) && value.is_empty() {
                missing_keys.push(key.clone());
            }
        }
        if !missing_keys.is_empty() {
            // Use server name as key since there's no ID
            missing_mcp_env.insert(server.name.clone(), missing_keys);
        }

        // Check for name conflict - this is the ONLY conflict check
        let name_query = "SELECT meta::id(id) AS id FROM mcp_server WHERE name = $name";
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(name_query)
            .bind(("name", server.name.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .unwrap_or_default();

        if !name_results.is_empty() {
            let existing = &name_results[0];
            conflicts.push(ImportConflict {
                entity_type: "mcp".to_string(),
                entity_name: server.name.clone(),
                existing_id: existing["id"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    // Check model conflicts - by NAME only (IDs are not in the export file)
    for model in &package.models {
        model_summaries.push(LLMModelExportSummary {
            id: None, // No ID in import file
            name: model.name.clone(),
            provider: model.provider.clone(),
            api_name: model.api_name.clone(),
            is_builtin: model.is_builtin,
        });

        // Warn about importing builtin models
        if model.is_builtin {
            warnings.push(format!(
                "Model '{}' is a builtin model and may conflict with system defaults",
                model.name
            ));
        }

        // Check for name conflict - this is the ONLY conflict check
        let name_query = "SELECT meta::id(id) AS id FROM llm_model WHERE name = $name";
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(name_query)
            .bind(("name", model.name.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .unwrap_or_default();

        if !name_results.is_empty() {
            let existing = &name_results[0];
            conflicts.push(ImportConflict {
                entity_type: "model".to_string(),
                entity_name: model.name.clone(),
                existing_id: existing["id"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    // Check prompt conflicts - by NAME only (IDs are not in the export file)
    for prompt in &package.prompts {
        let content = &prompt.content;
        let var_count = content.matches("{{").count();
        prompt_summaries.push(PromptExportSummary {
            id: None, // No ID in import file
            name: prompt.name.clone(),
            description: prompt.description.clone(),
            category: prompt.category.clone(),
            variables_count: var_count,
        });

        // Check for name conflict - this is the ONLY conflict check
        let name_query = "SELECT meta::id(id) AS id FROM prompt WHERE name = $name";
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(name_query)
            .bind(("name", prompt.name.clone()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .unwrap_or_default();

        if !name_results.is_empty() {
            let existing = &name_results[0];
            conflicts.push(ImportConflict {
                entity_type: "prompt".to_string(),
                entity_name: prompt.name.clone(),
                existing_id: existing["id"].as_str().unwrap_or("").to_string(),
            });
        }
    }

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

    // Import agents - selection and resolution by NAME (no IDs in export file)
    for agent in &package.agents {
        // Selection is by name
        if !selection.agents.contains(&agent.name) {
            continue;
        }

        // Resolution keys use format: "entityType:entityName"
        let resolution_key = format!("agent:{}", agent.name);
        let resolution = resolutions.get(&resolution_key).cloned();
        if resolution == Some(ConflictResolution::Skip) {
            skipped.agents += 1;
            continue;
        }

        // For Overwrite, we need to find the existing ID by name
        let existing_id = if resolution == Some(ConflictResolution::Overwrite) {
            let query = "SELECT meta::id(id) AS id FROM agent WHERE name = $name";
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(query)
                .bind(("name", agent.name.clone()))
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

        // Always generate new UUID for new imports
        let agent_id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Rename adds suffix to name
        let name = if resolution == Some(ConflictResolution::Rename) {
            format!("{} (imported)", agent.name)
        } else {
            agent.name.clone()
        };

        // Build parameterized query with bind parameters to prevent injection
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
            "system_prompt": agent.system_prompt,
            "max_tool_iterations": agent.max_tool_iterations,
            "enable_thinking": agent.enable_thinking,
        }));

        let result = if resolution == Some(ConflictResolution::Overwrite) {
            let query = format!("UPDATE agent:`{}` CONTENT $data", agent_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        } else {
            let query = format!("CREATE agent:`{}` CONTENT $data", agent_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        };

        match result {
            Ok(_) => {
                // Set timestamps via separate query (time::now() cannot be in CONTENT $data)
                let ts_query = format!(
                    "UPDATE agent:`{}` SET created_at = time::now(), updated_at = time::now()",
                    agent_id
                );
                if let Err(e) = state.db.execute(&ts_query).await {
                    tracing::warn!(agent_id = %agent_id, error = %e, "Failed to set timestamps on imported agent");
                }
                imported.agents += 1;
            }
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "agent".to_string(),
                    entity_id: agent.name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // Import MCP servers - selection and resolution by NAME (no IDs in export file)
    for server in &package.mcp_servers {
        // Selection is by name
        if !selection.mcp_servers.contains(&server.name) {
            continue;
        }

        // Resolution keys use format: "entityType:entityName"
        let resolution_key = format!("mcp:{}", server.name);
        let resolution = resolutions.get(&resolution_key).cloned();
        if resolution == Some(ConflictResolution::Skip) {
            skipped.mcp_servers += 1;
            continue;
        }

        // For Overwrite, we need to find the existing ID by name
        let existing_id = if resolution == Some(ConflictResolution::Overwrite) {
            let query = "SELECT meta::id(id) AS id FROM mcp_server WHERE name = $name";
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(query)
                .bind(("name", server.name.clone()))
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

        // Always generate new UUID for new imports
        let server_id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Rename adds suffix to name
        let name = if resolution == Some(ConflictResolution::Rename) {
            format!("{} (imported)", server.name)
        } else {
            server.name.clone()
        };

        // Apply additions (keyed by name now)
        let mut env = server.env.clone();
        if let Some(additions) = mcp_additions.get(&server.name) {
            for (key, value) in &additions.add_env {
                env.insert(key.clone(), value.clone());
            }
        }
        // env is stored as JSON STRING in SurrealDB (not as object)
        let env_str = serde_json::to_string(&env).unwrap_or_else(|_| "{}".to_string());

        // Build parameterized query with bind parameters to prevent injection
        let data = sanitize_for_surrealdb(serde_json::json!({
            "name": name,
            "enabled": server.enabled,
            "command": server.command,
            "args": server.args,
            "env": env_str,
            "description": server.description,
        }));

        let result = if resolution == Some(ConflictResolution::Overwrite) {
            let query = format!("UPDATE mcp_server:`{}` CONTENT $data", server_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        } else {
            let query = format!("CREATE mcp_server:`{}` CONTENT $data", server_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        };

        match result {
            Ok(_) => {
                // Set timestamps via separate query (time::now() cannot be in CONTENT $data)
                let ts_query = format!(
                    "UPDATE mcp_server:`{}` SET created_at = time::now(), updated_at = time::now()",
                    server_id
                );
                if let Err(e) = state.db.execute(&ts_query).await {
                    tracing::warn!(server_id = %server_id, error = %e, "Failed to set timestamps on imported MCP server");
                }
                imported.mcp_servers += 1;
            }
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "mcp".to_string(),
                    entity_id: server.name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // Import models - selection and resolution by NAME (no IDs in export file)
    for model in &package.models {
        // Selection is by name
        if !selection.models.contains(&model.name) {
            continue;
        }

        // Resolution keys use format: "entityType:entityName"
        let resolution_key = format!("model:{}", model.name);
        let resolution = resolutions.get(&resolution_key).cloned();
        if resolution == Some(ConflictResolution::Skip) {
            skipped.models += 1;
            continue;
        }

        // For Overwrite, we need to find the existing ID by name
        let existing_id = if resolution == Some(ConflictResolution::Overwrite) {
            let query = "SELECT meta::id(id) AS id FROM llm_model WHERE name = $name";
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(query)
                .bind(("name", model.name.clone()))
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

        // Always generate new UUID for new imports
        let model_id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Rename adds suffix to name
        let name = if resolution == Some(ConflictResolution::Rename) {
            format!("{} (imported)", model.name)
        } else {
            model.name.clone()
        };

        // Build parameterized query with bind parameters to prevent injection
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
        }));

        let result = if resolution == Some(ConflictResolution::Overwrite) {
            let query = format!("UPDATE llm_model:`{}` CONTENT $data", model_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        } else {
            let query = format!("CREATE llm_model:`{}` CONTENT $data", model_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        };

        match result {
            Ok(_) => {
                // Set timestamps via separate query (time::now() cannot be in CONTENT $data)
                let ts_query = format!(
                    "UPDATE llm_model:`{}` SET created_at = time::now(), updated_at = time::now()",
                    model_id
                );
                if let Err(e) = state.db.execute(&ts_query).await {
                    tracing::warn!(model_id = %model_id, error = %e, "Failed to set timestamps on imported model");
                }
                imported.models += 1;
            }
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "model".to_string(),
                    entity_id: model.name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    // Import prompts - selection and resolution by NAME (no IDs in export file)
    for prompt in &package.prompts {
        // Selection is by name
        if !selection.prompts.contains(&prompt.name) {
            continue;
        }

        // Resolution keys use format: "entityType:entityName"
        let resolution_key = format!("prompt:{}", prompt.name);
        let resolution = resolutions.get(&resolution_key).cloned();
        if resolution == Some(ConflictResolution::Skip) {
            skipped.prompts += 1;
            continue;
        }

        // For Overwrite, we need to find the existing ID by name
        let existing_id = if resolution == Some(ConflictResolution::Overwrite) {
            let query = "SELECT meta::id(id) AS id FROM prompt WHERE name = $name";
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(query)
                .bind(("name", prompt.name.clone()))
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

        // Always generate new UUID for new imports
        let prompt_id = existing_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Rename adds suffix to name
        let name = if resolution == Some(ConflictResolution::Rename) {
            format!("{} (imported)", prompt.name)
        } else {
            prompt.name.clone()
        };

        // Extract variables from content using the same pattern as create_prompt
        let variables = Prompt::detect_variables(&prompt.content);

        // Build parameterized query with bind parameters to prevent injection
        let data = sanitize_for_surrealdb(serde_json::json!({
            "name": name,
            "description": prompt.description,
            "category": prompt.category,
            "content": prompt.content,
            "variables": variables,
        }));

        let result = if resolution == Some(ConflictResolution::Overwrite) {
            let query = format!("UPDATE prompt:`{}` CONTENT $data", prompt_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        } else {
            let query = format!("CREATE prompt:`{}` CONTENT $data", prompt_id);
            state
                .db
                .execute_with_params(&query, vec![("data".to_string(), data)])
                .await
        };

        match result {
            Ok(_) => {
                // Set timestamps via separate query (time::now() cannot be in CONTENT $data)
                let ts_query = format!(
                    "UPDATE prompt:`{}` SET created_at = time::now(), updated_at = time::now()",
                    prompt_id
                );
                if let Err(e) = state.db.execute(&ts_query).await {
                    tracing::warn!(prompt_id = %prompt_id, error = %e, "Failed to set timestamps on imported prompt");
                }
                imported.prompts += 1;
            }
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "prompt".to_string(),
                    entity_id: prompt.name.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

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
