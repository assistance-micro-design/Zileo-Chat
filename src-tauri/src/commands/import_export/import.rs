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

//! Import Commands (schema v1.1)
//!
//! Tauri commands for importing configuration entities.
//!
//! - `validate_import` - Validate import file, detect conflicts, check cross-dependencies
//! - `execute_import` - Execute import with correct ordering and post-import actions
//!
//! Import order: custom_providers -> models -> mcp_servers -> skills -> agents -> prompts

use crate::db::client::DBClient;
use crate::models::import_export::*;
use crate::state::AppState;
use std::collections::{HashMap, HashSet};
use tauri::State;
use tracing::instrument;

use super::helpers::{check_name_conflict, ImportTracking};
use super::import_ops::{
    import_agents, import_custom_providers, import_mcp_servers, import_models, import_prompts,
    import_skills,
};

/// Validates an import file and detects conflicts with existing entities.
///
/// # Returns
/// Validation result with entities, structured warnings, and conflicts
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

    // Check schema version (accept v1.0 and v1.1)
    if !SUPPORTED_SCHEMA_VERSIONS.contains(&package.manifest.version.as_str()) {
        return Ok(ImportValidation::invalid(vec![format!(
            "Unsupported schema version: {} (supported: {})",
            package.manifest.version,
            SUPPORTED_SCHEMA_VERSIONS.join(", ")
        )]));
    }

    // Check total entity count to prevent DoS
    let total_entities = package.agents.len()
        + package.mcp_servers.len()
        + package.models.len()
        + package.prompts.len()
        + package.skills.len()
        + package.custom_providers.len();
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
    let skill_summaries = validate_skills(&state.db, &package.skills, &mut conflicts).await;
    let custom_provider_summaries =
        validate_custom_providers(&state.db, &package.custom_providers, &mut conflicts).await;

    // Cross-dependency validation (v1.1)
    let dep_warnings =
        validate_cross_dependencies(&package, &state.db, &package.manifest.version).await;
    warnings.extend(dep_warnings);

    tracing::info!(
        agents = agent_summaries.len(),
        mcp_servers = mcp_summaries.len(),
        models = model_summaries.len(),
        prompts = prompt_summaries.len(),
        skills = skill_summaries.len(),
        custom_providers = custom_provider_summaries.len(),
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
            skills: skill_summaries,
            custom_providers: custom_provider_summaries,
        },
        conflicts,
        missing_mcp_env,
    })
}

/// Executes the import with conflict resolutions applied.
///
/// Import order: custom_providers -> models -> mcp_servers -> skills -> agents -> prompts
/// Dependencies are created before entities that reference them.
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
        skills = selection.skills.len(),
        custom_providers = selection.custom_providers.len(),
        "Executing import"
    );

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

    // CORRECT ORDER: dependencies first, then entities that reference them
    // 1. Custom providers (referenced by models and agents)
    import_custom_providers(
        &state.db,
        &package.custom_providers,
        &selection.custom_providers,
        &resolutions,
        &mut tracking,
    )
    .await;

    // 2. Models (referenced by agents)
    import_models(
        &state.db,
        &package.models,
        &selection.models,
        &resolutions,
        &mut tracking,
    )
    .await;

    // 3. MCP servers (referenced by agents)
    import_mcp_servers(
        &state.db,
        &package.mcp_servers,
        &selection.mcp_servers,
        &resolutions,
        &mcp_additions,
        &mut tracking,
    )
    .await;

    // 4. Skills (referenced by agents)
    import_skills(
        &state.db,
        &package.skills,
        &selection.skills,
        &resolutions,
        &mut tracking,
    )
    .await;

    // 5. Agents (references all above)
    import_agents(
        &state.db,
        &package.agents,
        &selection.agents,
        &resolutions,
        &mut tracking,
    )
    .await;

    // 6. Prompts (may reference skills via {{skill:name}})
    import_prompts(
        &state.db,
        &package.prompts,
        &selection.prompts,
        &resolutions,
        &mut tracking,
    )
    .await;

    // Generate post-import actions for actually imported agents
    let post_import_actions = generate_post_import_actions(&package, &selection, &state.db).await;

    let success = errors.is_empty();

    tracing::info!(
        success = success,
        imported_agents = imported.agents,
        imported_mcp = imported.mcp_servers,
        imported_models = imported.models,
        imported_prompts = imported.prompts,
        imported_skills = imported.skills,
        imported_custom_providers = imported.custom_providers,
        skipped_total = skipped.agents
            + skipped.mcp_servers
            + skipped.models
            + skipped.prompts
            + skipped.skills
            + skipped.custom_providers,
        errors = errors.len(),
        post_actions = post_import_actions.len(),
        "Import execution complete"
    );

    Ok(ImportResult {
        success,
        imported,
        skipped,
        errors,
        post_import_actions,
    })
}

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
            skills_count: agent.skills.len(),
            folders_count: agent.folders.len(),
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
    warnings: &mut Vec<ImportWarning>,
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
            warnings.push(ImportWarning {
                warning_type: ImportWarningType::BuiltinModel,
                severity: "info".to_string(),
                entity: format!("Model '{}'", model.name),
                detail: "builtin model may conflict with system defaults".to_string(),
                action: "Review model settings after import if behavior differs".to_string(),
            });
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

/// Builds skill summaries and detects name conflicts (v1.1).
async fn validate_skills(
    db: &DBClient,
    skills: &[SkillExportData],
    conflicts: &mut Vec<ImportConflict>,
) -> Vec<SkillExportSummary> {
    let mut summaries = Vec::new();
    for skill in skills {
        summaries.push(SkillExportSummary {
            id: None,
            name: skill.name.clone(),
            category: skill.category.clone(),
            enabled: skill.enabled,
            content_length: skill.content.len(),
        });
        if let Some(conflict) = check_name_conflict(db, "skill", "skill", &skill.name).await {
            conflicts.push(conflict);
        }
    }
    summaries
}

/// Builds custom provider summaries and detects name conflicts (v1.1).
async fn validate_custom_providers(
    db: &DBClient,
    providers: &[CustomProviderExportData],
    conflicts: &mut Vec<ImportConflict>,
) -> Vec<CustomProviderExportSummary> {
    let mut summaries = Vec::new();
    for provider in providers {
        summaries.push(CustomProviderExportSummary {
            id: Some(provider.name.clone()),
            name: provider.name.clone(),
            display_name: provider.display_name.clone(),
            base_url: provider.base_url.clone(),
        });
        if let Some(conflict) =
            check_name_conflict(db, "custom_provider", "custom_provider", &provider.name).await
        {
            conflicts.push(conflict);
        }
    }
    summaries
}

/// Validates cross-entity dependencies and generates structured warnings.
/// Checks that agents' referenced models, MCP servers, skills, and custom providers
/// exist either in the database or in the import package.
async fn validate_cross_dependencies(
    package: &ExportPackage,
    db: &DBClient,
    schema_version: &str,
) -> Vec<ImportWarning> {
    let mut warnings = Vec::new();

    let pkg_model_names: HashSet<&str> = package.models.iter().map(|m| m.name.as_str()).collect();
    let pkg_mcp_names: HashSet<&str> = package
        .mcp_servers
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let pkg_skill_names: HashSet<&str> = package.skills.iter().map(|s| s.name.as_str()).collect();
    let pkg_provider_names: HashSet<&str> = package
        .custom_providers
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    for agent in &package.agents {
        let entity = format!("Agent '{}'", agent.name);

        // Check model reference
        if !agent.llm.model.is_empty()
            && !pkg_model_names.contains(agent.llm.model.as_str())
            && !name_exists_in_db(db, "llm_model", &agent.llm.model).await
        {
            warnings.push(ImportWarning {
                warning_type: ImportWarningType::MissingDependency,
                severity: "high".to_string(),
                entity: entity.clone(),
                detail: format!(
                    "model '{}' not found in database or import file",
                    agent.llm.model
                ),
                action: "Add the model in Settings > Models after import, or agent cannot chat"
                    .to_string(),
            });
        }

        // Check MCP servers
        for mcp in &agent.mcp_servers {
            if !pkg_mcp_names.contains(mcp.as_str())
                && !name_exists_in_db(db, "mcp_server", mcp).await
            {
                warnings.push(ImportWarning {
                    warning_type: ImportWarningType::MissingDependency,
                    severity: "medium".to_string(),
                    entity: entity.clone(),
                    detail: format!("MCP server '{}' not found", mcp),
                    action:
                        "Configure the server in Settings > MCP after import, or remove from agent"
                            .to_string(),
                });
            }
        }

        // Check skills
        for skill in &agent.skills {
            if !pkg_skill_names.contains(skill.as_str())
                && !name_exists_in_db(db, "skill", skill).await
            {
                warnings.push(ImportWarning {
                    warning_type: ImportWarningType::MissingDependency,
                    severity: "medium".to_string(),
                    entity: entity.clone(),
                    detail: format!("skill '{}' not found", skill),
                    action:
                        "Create a skill with the same name in Settings > Skills, or edit agent to remove reference"
                            .to_string(),
                });
            }
        }

        // Check custom provider
        if let Some(custom_name) = extract_custom_provider_name(&agent.llm.provider) {
            if !pkg_provider_names.contains(custom_name.as_str())
                && !name_exists_in_db(db, "custom_provider", &custom_name).await
            {
                warnings.push(ImportWarning {
                    warning_type: ImportWarningType::MissingDependency,
                    severity: "high".to_string(),
                    entity: entity.clone(),
                    detail: format!("custom provider '{}' not found", custom_name),
                    action: "Create the provider in Settings > Models before using this agent"
                        .to_string(),
                });
            }
        }

        // Check folders (machine-specific paths)
        if !agent.folders.is_empty() {
            warnings.push(ImportWarning {
                warning_type: ImportWarningType::MachineSpecific,
                severity: "info".to_string(),
                entity: entity.clone(),
                detail: format!(
                    "{} folder path(s) are machine-specific",
                    agent.folders.len()
                ),
                action: "Verify folder paths exist on this machine in Settings > Agents"
                    .to_string(),
            });
        }
    }

    // Check models -> custom providers
    for model in &package.models {
        if let Some(custom_name) = extract_custom_provider_name(&model.provider) {
            if !pkg_provider_names.contains(custom_name.as_str())
                && !name_exists_in_db(db, "custom_provider", &custom_name).await
            {
                warnings.push(ImportWarning {
                    warning_type: ImportWarningType::MissingDependency,
                    severity: "high".to_string(),
                    entity: format!("Model '{}'", model.name),
                    detail: format!("custom provider '{}' not found", custom_name),
                    action: "Create the provider in Settings > Models before using this model"
                        .to_string(),
                });
            }
        }
    }

    // Custom providers in package: warn about API keys (never exported)
    if !package.custom_providers.is_empty() {
        warnings.push(ImportWarning {
            warning_type: ImportWarningType::DefaultApplied,
            severity: "medium".to_string(),
            entity: format!("{} custom provider(s)", package.custom_providers.len()),
            detail: "API keys are never exported for security reasons".to_string(),
            action: "Configure API keys for imported custom providers in Settings > Models"
                .to_string(),
        });
    }

    // v1.0 schema: warn about defaulted fields
    if schema_version == "1.0" && !package.agents.is_empty() {
        warnings.push(ImportWarning {
            warning_type: ImportWarningType::DefaultApplied,
            severity: "medium".to_string(),
            entity: "All agents".to_string(),
            detail: "v1.0 export: is_reasoning defaulted to false, context_window to auto, folders to empty".to_string(),
            action: "Verify LLM settings for thinking models in Settings > Agents".to_string(),
        });
    }

    warnings
}

/// Checks if an entity with the given name exists in a database table.
async fn name_exists_in_db(db: &DBClient, table: &str, name: &str) -> bool {
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

/// Generates actionable post-import items for entities that were actually imported.
/// Re-checks dependencies after import to account for entities created during import.
async fn generate_post_import_actions(
    package: &ExportPackage,
    selection: &ImportSelection,
    db: &DBClient,
) -> Vec<String> {
    let mut actions = Vec::new();

    // Only check agents that were selected for import
    let selected_agents: HashSet<&str> = selection.agents.iter().map(|s| s.as_str()).collect();

    for agent in &package.agents {
        if !selected_agents.contains(agent.name.as_str()) {
            continue;
        }

        // Check skills (may still be missing if not in package or not selected)
        for skill in &agent.skills {
            if !name_exists_in_db(db, "skill", skill).await {
                actions.push(format!(
                    "Agent '{}': create skill '{}' or remove reference (Settings > Agents)",
                    agent.name, skill
                ));
            }
        }

        // Check MCP servers
        for mcp in &agent.mcp_servers {
            if !name_exists_in_db(db, "mcp_server", mcp).await {
                actions.push(format!(
                    "Agent '{}': configure MCP server '{}' (Settings > MCP)",
                    agent.name, mcp
                ));
            }
        }

        // Check model
        if !agent.llm.model.is_empty()
            && !name_exists_in_db(db, "llm_model", &agent.llm.model).await
        {
            actions.push(format!(
                "Agent '{}': add model '{}' (Settings > Models)",
                agent.name, agent.llm.model
            ));
        }

        // Check custom provider
        if let Some(custom_name) = extract_custom_provider_name(&agent.llm.provider) {
            if !name_exists_in_db(db, "custom_provider", &custom_name).await {
                actions.push(format!(
                    "Agent '{}': create custom provider '{}' (Settings > Models)",
                    agent.name, custom_name
                ));
            }
        }

        // Folder paths
        if !agent.folders.is_empty() {
            actions.push(format!(
                "Agent '{}': verify {} folder path(s) on this machine (Settings > Agents)",
                agent.name,
                agent.folders.len()
            ));
        }
    }

    actions
}
