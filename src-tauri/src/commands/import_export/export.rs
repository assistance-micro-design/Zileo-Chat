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

//! Export Commands
//!
//! Tauri commands for exporting configuration entities (schema v1.1).
//!
//! - `prepare_export_preview` - Get preview data for selected entities
//! - `generate_export_file` - Generate export JSON with sanitization applied
//! - `save_export_to_file` - Save export content to a file
//!
//! Supported entities: Agents, MCP Servers, Models, Prompts, Skills, Custom Providers.

use crate::db::client::DBClient;
use crate::models::import_export::*;
use crate::state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::State;
use tracing::instrument;

use super::helpers::{
    apply_mcp_sanitization, extract_optional_timestamp, load_mcp_preview, query_entity_by_id,
};

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
        skills: Vec::new(),
        custom_providers: Vec::new(),
        mcp_env_keys: HashMap::new(),
    };

    // Load agent summaries
    for agent_id in &selection.agents {
        let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, skills, folders FROM agent WHERE meta::id(id) = $id";
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
                skills_count: row["skills"].as_array().map(|a| a.len()).unwrap_or(0),
                folders_count: row["folders"].as_array().map(|a| a.len()).unwrap_or(0),
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

    // Load skill summaries
    for skill_id in &selection.skills {
        let query = "SELECT meta::id(id) AS id, name, category, enabled, content FROM skill WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(&state.db, query, skill_id, "skill").await? {
            let content_len = row["content"].as_str().map(|s| s.len()).unwrap_or(0);
            preview.skills.push(SkillExportSummary {
                id: Some(row["id"].as_str().unwrap_or("").to_string()),
                name: row["name"].as_str().unwrap_or("Unknown").to_string(),
                category: row["category"].as_str().unwrap_or("custom").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(true),
                content_length: content_len,
            });
        }
    }

    // Load custom provider summaries (queried by name, not UUID)
    for provider_name in &selection.custom_providers {
        let query = "SELECT name, display_name, base_url FROM custom_provider WHERE name = $name";
        let results: Vec<serde_json::Value> = state
            .db
            .db
            .query(query)
            .bind(("name", provider_name.to_string()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query custom provider: {}", e))?;
        if let Some(row) = results.into_iter().next() {
            preview.custom_providers.push(CustomProviderExportSummary {
                id: Some(row["name"].as_str().unwrap_or("").to_string()),
                name: row["name"].as_str().unwrap_or("").to_string(),
                display_name: row["display_name"].as_str().unwrap_or("").to_string(),
                base_url: row["base_url"].as_str().unwrap_or("").to_string(),
            });
        }
    }

    tracing::info!(
        agents = preview.agents.len(),
        mcp_servers = preview.mcp_servers.len(),
        models = preview.models.len(),
        prompts = preview.prompts.len(),
        skills = preview.skills.len(),
        custom_providers = preview.custom_providers.len(),
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
    let skills = export_skills(&state.db, &selection.skills, ts).await?;
    let custom_providers = export_custom_providers(&state.db, &selection.custom_providers).await?;

    // Build export package
    let package = ExportPackage::new(
        agents,
        mcp_servers,
        models,
        prompts,
        skills,
        custom_providers,
        None,
    );

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

/// Exports agent entities from the database.
async fn export_agents(
    db: &DBClient,
    agent_ids: &[String],
    include_timestamps: bool,
) -> Result<Vec<AgentExportData>, String> {
    let mut agents = Vec::new();
    for agent_id in agent_ids {
        let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, skills, system_prompt, max_tool_iterations, reasoning_effort, folders, require_file_confirmation, created_at, updated_at FROM agent WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, agent_id, "agent").await? {
            let llm = &row["llm"];
            let extract_string_array = |val: &serde_json::Value| -> Vec<String> {
                val.as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default()
            };
            agents.push(AgentExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                lifecycle: row["lifecycle"].as_str().unwrap_or("permanent").to_string(),
                llm: LLMConfigExport {
                    provider: llm["provider"].as_str().unwrap_or("").to_string(),
                    model: llm["model"].as_str().unwrap_or("").to_string(),
                    temperature: llm["temperature"].as_f64().unwrap_or(0.7),
                    max_tokens: llm["max_tokens"].as_u64().unwrap_or(4096) as usize,
                    is_reasoning: llm["is_reasoning"].as_bool().unwrap_or(false),
                    context_window: llm["context_window"].as_u64().map(|v| v as usize),
                },
                tools: extract_string_array(&row["tools"]),
                mcp_servers: extract_string_array(&row["mcp_servers"]),
                skills: extract_string_array(&row["skills"]),
                system_prompt: row["system_prompt"].as_str().unwrap_or("").to_string(),
                max_tool_iterations: row["max_tool_iterations"].as_u64().unwrap_or(50) as usize,
                reasoning_effort: row["reasoning_effort"].as_str().and_then(|s| {
                    serde_json::from_value(serde_json::Value::String(s.to_string())).ok()
                }),
                folders: extract_string_array(&row["folders"]),
                require_file_confirmation: row["require_file_confirmation"]
                    .as_bool()
                    .unwrap_or(true),
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
        let query = "SELECT meta::id(id) AS id, name, enabled, command, args, env, description, \
                     auth_type, auth_metadata, extra_headers, created_at, updated_at \
                     FROM mcp_server WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, server_id, "MCP server").await? {
            let (env, args) = apply_mcp_sanitization(&row, server_id, sanitization);

            let san = sanitization.get(server_id);
            let clear_auth = san.is_some_and(|c| c.clear_auth_metadata);
            let clear_headers = san.is_some_and(|c| c.clear_extra_headers);

            // v1.2 auth metadata - parsed from JSON strings stored in DB.
            // Secrets are NEVER exported; only the type + non-sensitive metadata.
            let auth_type = if clear_auth {
                None
            } else {
                crate::mcp::helpers::parse_auth_type(row.get("auth_type"))
                    .filter(|t| *t != crate::models::mcp::MCPAuthType::None)
            };
            let auth_metadata = if clear_auth {
                None
            } else {
                crate::mcp::helpers::parse_auth_metadata_json(row.get("auth_metadata"))
            };
            let extra_headers = if clear_headers {
                None
            } else {
                crate::mcp::helpers::parse_extra_headers_json(row.get("extra_headers"))
            };

            servers.push(MCPServerExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(false),
                command: row["command"].as_str().unwrap_or("").to_string(),
                args,
                env,
                description: row["description"].as_str().map(String::from),
                auth_type,
                auth_metadata,
                extra_headers,
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
                temperature_default: row["temperature_default"].as_f64().unwrap_or(0.7),
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

/// Exports skill entities from the database (v1.1).
async fn export_skills(
    db: &DBClient,
    skill_ids: &[String],
    include_timestamps: bool,
) -> Result<Vec<SkillExportData>, String> {
    let mut skills = Vec::new();
    for skill_id in skill_ids {
        let query = "SELECT meta::id(id) AS id, name, description, category, content, enabled, created_at, updated_at FROM skill WHERE meta::id(id) = $id";
        if let Some(row) = query_entity_by_id(db, query, skill_id, "skill").await? {
            skills.push(SkillExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                description: row["description"].as_str().unwrap_or("").to_string(),
                category: row["category"].as_str().unwrap_or("custom").to_string(),
                content: row["content"].as_str().unwrap_or("").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(true),
                created_at: extract_optional_timestamp(&row, "created_at", include_timestamps),
                updated_at: extract_optional_timestamp(&row, "updated_at", include_timestamps),
            });
        }
    }
    Ok(skills)
}

/// Exports custom provider entities from the database (v1.1).
/// Custom providers use name as primary key (not UUID).
async fn export_custom_providers(
    db: &DBClient,
    provider_names: &[String],
) -> Result<Vec<CustomProviderExportData>, String> {
    let mut providers = Vec::new();
    for name in provider_names {
        let query = "SELECT name, display_name, base_url, enabled, created_at FROM custom_provider WHERE name = $name";
        let results: Vec<serde_json::Value> = db
            .db
            .query(query)
            .bind(("name", name.to_string()))
            .await
            .map(|mut r| r.take(0).unwrap_or_default())
            .map_err(|e| format!("Failed to query custom provider '{}': {}", name, e))?;
        if let Some(row) = results.into_iter().next() {
            providers.push(CustomProviderExportData {
                name: row["name"].as_str().unwrap_or("").to_string(),
                display_name: row["display_name"].as_str().unwrap_or("").to_string(),
                base_url: row["base_url"].as_str().unwrap_or("").to_string(),
                enabled: row["enabled"].as_bool().unwrap_or(true),
                created_at: row["created_at"].as_str().map(String::from),
            });
        }
    }
    Ok(providers)
}
