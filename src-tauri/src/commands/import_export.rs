// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
        let query = format!(
            "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers FROM agent WHERE meta::id(id) = '{}'",
            agent_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
        let query = format!(
            "SELECT meta::id(id) AS id, name, enabled, command, env FROM mcp_server WHERE meta::id(id) = '{}'",
            server_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
        let query = format!(
            "SELECT meta::id(id) AS id, name, provider, api_name, is_builtin FROM llm_model WHERE meta::id(id) = '{}'",
            model_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
        let query = format!(
            "SELECT meta::id(id) AS id, name, description, category, content FROM prompt WHERE meta::id(id) = '{}'",
            prompt_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
        let query = format!(
            "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, system_prompt, max_tool_iterations, created_at, updated_at FROM agent WHERE meta::id(id) = '{}'",
            agent_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                mcp_servers: row["mcp_servers"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default(),
                system_prompt: row["system_prompt"].as_str().unwrap_or("").to_string(),
                max_tool_iterations: row["max_tool_iterations"].as_u64().unwrap_or(50) as usize,
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

        let query = format!(
            "SELECT meta::id(id) AS id, name, enabled, command, args, env, description, created_at, updated_at FROM mcp_server WHERE meta::id(id) = '{}'",
            server_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default()
                }
            } else {
                row["args"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
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
        let query = format!(
            "SELECT meta::id(id) AS id, provider, name, api_name, context_window, max_output_tokens, temperature_default, is_builtin, is_reasoning, input_price_per_mtok, output_price_per_mtok, created_at, updated_at FROM llm_model WHERE meta::id(id) = '{}'",
            model_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
        let query = format!(
            "SELECT meta::id(id) AS id, name, description, category, content, created_at, updated_at FROM prompt WHERE meta::id(id) = '{}'",
            prompt_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&query)
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
        let name_query = format!(
            "SELECT meta::id(id) AS id FROM agent WHERE name = {}",
            serde_json::to_string(&agent.name).unwrap_or_default()
        );
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&name_query)
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
        let name_query = format!(
            "SELECT meta::id(id) AS id FROM mcp_server WHERE name = {}",
            serde_json::to_string(&server.name).unwrap_or_default()
        );
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&name_query)
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
        let name_query = format!(
            "SELECT meta::id(id) AS id FROM llm_model WHERE name = {}",
            serde_json::to_string(&model.name).unwrap_or_default()
        );
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&name_query)
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
        let name_query = format!(
            "SELECT meta::id(id) AS id FROM prompt WHERE name = {}",
            serde_json::to_string(&prompt.name).unwrap_or_default()
        );
        let name_results: Vec<serde_json::Value> = state
            .db
            .db
            .query(&name_query)
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
    let package: ExportPackage = serde_json::from_str(&data)
        .map_err(|e| format!("Invalid JSON: {}", e))?;

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
            let query = format!(
                "SELECT meta::id(id) AS id FROM agent WHERE name = {}",
                serde_json::to_string(&agent.name).unwrap_or_default()
            );
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(&query)
                .await
                .map(|mut r| r.take(0).unwrap_or_default())
                .unwrap_or_default();
            results.first().and_then(|r| r["id"].as_str()).map(String::from)
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

        // Build insert/upsert query
        let tools_json = serde_json::to_string(&agent.tools).unwrap_or("[]".to_string());
        let mcp_servers_json = serde_json::to_string(&agent.mcp_servers).unwrap_or("[]".to_string());
        let system_prompt_json = serde_json::to_string(&agent.system_prompt).unwrap_or("\"\"".to_string());
        let name_json = serde_json::to_string(&name).unwrap_or("\"\"".to_string());

        let query = if resolution == Some(ConflictResolution::Overwrite) {
            format!(
                "UPDATE agent:`{}` SET \
                    name = {}, \
                    lifecycle = '{}', \
                    llm = {{ provider: '{}', model: '{}', temperature: {}, max_tokens: {} }}, \
                    tools = {}, \
                    mcp_servers = {}, \
                    system_prompt = {}, \
                    max_tool_iterations = {}, \
                    updated_at = time::now()",
                agent_id,
                name_json,
                agent.lifecycle,
                agent.llm.provider,
                agent.llm.model,
                agent.llm.temperature,
                agent.llm.max_tokens,
                tools_json,
                mcp_servers_json,
                system_prompt_json,
                agent.max_tool_iterations
            )
        } else {
            format!(
                "CREATE agent:`{}` CONTENT {{ \
                    id: '{}', \
                    name: {}, \
                    lifecycle: '{}', \
                    llm: {{ provider: '{}', model: '{}', temperature: {}, max_tokens: {} }}, \
                    tools: {}, \
                    mcp_servers: {}, \
                    system_prompt: {}, \
                    max_tool_iterations: {}, \
                    created_at: time::now(), \
                    updated_at: time::now() \
                }}",
                agent_id,
                agent_id,
                name_json,
                agent.lifecycle,
                agent.llm.provider,
                agent.llm.model,
                agent.llm.temperature,
                agent.llm.max_tokens,
                tools_json,
                mcp_servers_json,
                system_prompt_json,
                agent.max_tool_iterations
            )
        };

        match state.db.execute(&query).await {
            Ok(_) => imported.agents += 1,
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "agent".to_string(),
                    entity_id: agent.name.clone(), // Use name as identifier
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
            let query = format!(
                "SELECT meta::id(id) AS id FROM mcp_server WHERE name = {}",
                serde_json::to_string(&server.name).unwrap_or_default()
            );
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(&query)
                .await
                .map(|mut r| r.take(0).unwrap_or_default())
                .unwrap_or_default();
            results.first().and_then(|r| r["id"].as_str()).map(String::from)
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
        // Double JSON encoding required: env is stored as STRING in SurrealDB
        // Pattern from mcp/manager.rs: serialize HashMap to JSON, then encode as JSON string
        let env_str = serde_json::to_string(&env).unwrap_or("{}".to_string());
        let env_json = serde_json::to_string(&env_str).unwrap_or("\"{}\"".to_string());

        let args_json = serde_json::to_string(&server.args).unwrap_or("[]".to_string());
        let name_json = serde_json::to_string(&name).unwrap_or("\"\"".to_string());
        let description_json = serde_json::to_string(&server.description).unwrap_or("null".to_string());

        let query = if resolution == Some(ConflictResolution::Overwrite) {
            format!(
                "UPDATE mcp_server:`{}` SET \
                    name = {}, \
                    enabled = {}, \
                    command = '{}', \
                    args = {}, \
                    env = {}, \
                    description = {}, \
                    updated_at = time::now()",
                server_id,
                name_json,
                server.enabled,
                server.command,
                args_json,
                env_json,
                description_json
            )
        } else {
            format!(
                "CREATE mcp_server:`{}` CONTENT {{ \
                    id: '{}', \
                    name: {}, \
                    enabled: {}, \
                    command: '{}', \
                    args: {}, \
                    env: {}, \
                    description: {}, \
                    created_at: time::now(), \
                    updated_at: time::now() \
                }}",
                server_id,
                server_id,
                name_json,
                server.enabled,
                server.command,
                args_json,
                env_json,
                description_json
            )
        };

        match state.db.execute(&query).await {
            Ok(_) => imported.mcp_servers += 1,
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "mcp".to_string(),
                    entity_id: server.name.clone(), // Use name as identifier
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
            let query = format!(
                "SELECT meta::id(id) AS id FROM llm_model WHERE name = {}",
                serde_json::to_string(&model.name).unwrap_or_default()
            );
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(&query)
                .await
                .map(|mut r| r.take(0).unwrap_or_default())
                .unwrap_or_default();
            results.first().and_then(|r| r["id"].as_str()).map(String::from)
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

        let name_json = serde_json::to_string(&name).unwrap_or("\"\"".to_string());
        let api_name_json = serde_json::to_string(&model.api_name).unwrap_or("\"\"".to_string());

        let query = if resolution == Some(ConflictResolution::Overwrite) {
            format!(
                "UPDATE llm_model:`{}` SET \
                    provider = '{}', \
                    name = {}, \
                    api_name = {}, \
                    context_window = {}, \
                    max_output_tokens = {}, \
                    temperature_default = {}, \
                    is_builtin = {}, \
                    is_reasoning = {}, \
                    input_price_per_mtok = {}, \
                    output_price_per_mtok = {}, \
                    updated_at = time::now()",
                model_id,
                model.provider,
                name_json,
                api_name_json,
                model.context_window,
                model.max_output_tokens,
                model.temperature_default,
                model.is_builtin,
                model.is_reasoning,
                model.input_price_per_mtok,
                model.output_price_per_mtok
            )
        } else {
            format!(
                "CREATE llm_model:`{}` CONTENT {{ \
                    id: '{}', \
                    provider: '{}', \
                    name: {}, \
                    api_name: {}, \
                    context_window: {}, \
                    max_output_tokens: {}, \
                    temperature_default: {}, \
                    is_builtin: {}, \
                    is_reasoning: {}, \
                    input_price_per_mtok: {}, \
                    output_price_per_mtok: {}, \
                    created_at: time::now(), \
                    updated_at: time::now() \
                }}",
                model_id,
                model_id,
                model.provider,
                name_json,
                api_name_json,
                model.context_window,
                model.max_output_tokens,
                model.temperature_default,
                model.is_builtin,
                model.is_reasoning,
                model.input_price_per_mtok,
                model.output_price_per_mtok
            )
        };

        match state.db.execute(&query).await {
            Ok(_) => imported.models += 1,
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "model".to_string(),
                    entity_id: model.name.clone(), // Use name as identifier
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
            let query = format!(
                "SELECT meta::id(id) AS id FROM prompt WHERE name = {}",
                serde_json::to_string(&prompt.name).unwrap_or_default()
            );
            let results: Vec<serde_json::Value> = state
                .db
                .db
                .query(&query)
                .await
                .map(|mut r| r.take(0).unwrap_or_default())
                .unwrap_or_default();
            results.first().and_then(|r| r["id"].as_str()).map(String::from)
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

        let name_json = serde_json::to_string(&name).unwrap_or("\"\"".to_string());
        let description_json = serde_json::to_string(&prompt.description).unwrap_or("\"\"".to_string());
        let content_json = serde_json::to_string(&prompt.content).unwrap_or("\"\"".to_string());

        // Extract variables from content using the same pattern as create_prompt
        let variables = Prompt::detect_variables(&prompt.content);
        let variables_json = serde_json::to_string(&variables).unwrap_or("[]".to_string());

        let query = if resolution == Some(ConflictResolution::Overwrite) {
            format!(
                "UPDATE prompt:`{}` SET \
                    name = {}, \
                    description = {}, \
                    category = '{}', \
                    content = {}, \
                    variables = {}, \
                    updated_at = time::now()",
                prompt_id,
                name_json,
                description_json,
                prompt.category,
                content_json,
                variables_json
            )
        } else {
            format!(
                "CREATE prompt:`{}` CONTENT {{ \
                    id: '{}', \
                    name: {}, \
                    description: {}, \
                    category: '{}', \
                    content: {}, \
                    variables: {}, \
                    created_at: time::now(), \
                    updated_at: time::now() \
                }}",
                prompt_id,
                prompt_id,
                name_json,
                description_json,
                prompt.category,
                content_json,
                variables_json
            )
        };

        match state.db.execute(&query).await {
            Ok(_) => imported.prompts += 1,
            Err(e) => {
                errors.push(ImportError {
                    entity_type: "prompt".to_string(),
                    entity_id: prompt.name.clone(), // Use name as identifier
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
/// * `path` - Full path to save the file
/// * `content` - JSON content to write
///
/// # Returns
/// Number of bytes written
#[tauri::command]
#[instrument(name = "save_export_to_file", skip(content))]
pub async fn save_export_to_file(path: String, content: String) -> Result<usize, String> {
    let path = PathBuf::from(&path);

    tracing::info!(
        path = %path.display(),
        size_bytes = content.len(),
        "Saving export file"
    );

    std::fs::write(&path, &content).map_err(|e| format!("Failed to write file: {}", e))?;

    tracing::info!(path = %path.display(), "Export file saved successfully");

    Ok(content.len())
}

/// Reads an import file from the specified path.
///
/// # Arguments
/// * `path` - Full path to the file
///
/// # Returns
/// File content as string
#[tauri::command]
#[instrument(name = "read_import_file")]
pub async fn read_import_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(&path);

    tracing::info!(path = %path.display(), "Reading import file");

    let content =
        std::fs::read_to_string(&path).map_err(|e| format!("Failed to read file: {}", e))?;

    tracing::info!(
        path = %path.display(),
        size_bytes = content.len(),
        "Import file read successfully"
    );

    Ok(content)
}
