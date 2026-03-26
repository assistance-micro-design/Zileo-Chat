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
//! Tauri commands for exporting configuration entities.
//!
//! - `prepare_export_preview` - Get preview data for selected entities
//! - `generate_export_file` - Generate export JSON with sanitization applied
//! - `save_export_to_file` - Save export content to a file

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
                    temperature: llm["temperature"].as_f64().unwrap_or(0.7),
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
                temperature_default: row["temperature_default"].as_f64().unwrap_or(0.7),
                is_builtin: row["is_builtin"].as_bool().unwrap_or(false),
                is_reasoning: row["is_reasoning"].as_bool().unwrap_or(false),
                input_price_per_mtok: row["input_price_per_mtok"].as_f64().unwrap_or(0.0),
                output_price_per_mtok: row["output_price_per_mtok"].as_f64().unwrap_or(0.0),
                cache_read_price_per_mtok: row["cache_read_price_per_mtok"]
                    .as_f64()
                    .unwrap_or(0.0),
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
