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

//! Import Commands
//!
//! Tauri commands for importing configuration entities.
//!
//! - `validate_import` - Validate import file and detect conflicts
//! - `execute_import` - Execute import with conflict resolutions

use crate::db::client::DBClient;
use crate::models::import_export::*;
use crate::state::AppState;
use std::collections::HashMap;
use tauri::State;
use tracing::instrument;

use super::helpers::{check_name_conflict, ImportTracking};
use super::import_ops::{import_agents, import_mcp_servers, import_models, import_prompts};

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
