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

//! Agent CRUD Tauri commands
//!
//! Provides IPC commands for managing agent configurations with persistence.
//!
//! ## Commands
//!
//! - [`list_agents`] - List all agents (returns AgentSummary[])
//! - [`get_agent_config`] - Get full agent configuration by ID
//! - [`create_agent`] - Create a new agent
//! - [`update_agent`] - Update an existing agent
//! - [`delete_agent`] - Delete an agent

mod validation;

#[cfg(test)]
mod tests;

use crate::agents::LLMAgent;
use crate::models::{AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary, Lifecycle};
use crate::security::Validator;
use crate::state::AppState;
use crate::tools::context::AgentToolContext;
use serde_json::json;
use std::sync::Arc;
use tauri::State;
use tracing::{error, info, instrument, warn};
use validation::{format_reasoning_effort, merge_agent_config, validate_agent_create};

/// Registers an LLMAgent in the registry with proper context
async fn register_agent_runtime(state: &AppState, agent_id: &str, config: AgentConfig) {
    let agent_context = AgentToolContext::from_app_state_full(state);
    let llm_agent = LLMAgent::with_context(
        config,
        state.llm_manager.clone(),
        state.tool_factory.clone(),
        agent_context,
    );
    state
        .registry
        .register(agent_id.to_string(), Arc::new(llm_agent))
        .await;
}

/// Lists all agents with summary information
#[tauri::command]
#[instrument(name = "list_agents", skip(state))]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentSummary>, String> {
    info!("Listing agents");

    let agent_ids = state.registry.list().await;
    let mut summaries = Vec::with_capacity(agent_ids.len());

    for id in agent_ids {
        if let Some(agent) = state.registry.get(&id).await {
            summaries.push(AgentSummary::from(agent.config()));
        }
    }

    info!(count = summaries.len(), "Agents listed");
    Ok(summaries)
}

/// Gets agent configuration by ID
#[tauri::command]
#[instrument(name = "get_agent_config", skip(state), fields(agent_id = %agent_id))]
pub async fn get_agent_config(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<AgentConfig, String> {
    info!("Getting agent configuration");

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    let agent = state
        .registry
        .get(&validated_agent_id)
        .await
        .ok_or_else(|| {
            warn!(agent_id = %validated_agent_id, "Agent not found");
            "Agent not found".to_string()
        })?;

    let config = agent.config().clone();
    info!(
        agent_name = %config.name,
        lifecycle = ?config.lifecycle,
        tools_count = config.tools.len(),
        "Agent configuration retrieved"
    );

    Ok(config)
}

/// Checks that agent name is unique (case-insensitive, trimmed).
///
/// - `exclude_id`: If Some, excludes this agent from the check (for update_agent).
async fn check_agent_name_unique(
    db: &crate::db::DBClient,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<(), String> {
    let trimmed = name.trim();

    let (query, params) = match exclude_id {
        Some(id) => (
            "SELECT meta::id(id) AS id FROM agent WHERE string::lowercase(name) = string::lowercase($name) AND meta::id(id) != $id LIMIT 1",
            vec![
                ("name".to_string(), serde_json::json!(trimmed)),
                ("id".to_string(), serde_json::json!(id)),
            ],
        ),
        None => (
            "SELECT meta::id(id) AS id FROM agent WHERE string::lowercase(name) = string::lowercase($name) LIMIT 1",
            vec![("name".to_string(), serde_json::json!(trimmed))],
        ),
    };

    let results: Vec<serde_json::Value> = db
        .query_with_params(query, params)
        .await
        .map_err(|e| format!("Failed to check agent name uniqueness: {}", e))?;

    if !results.is_empty() {
        return Err(format!("An agent with name '{}' already exists", trimmed));
    }

    Ok(())
}

/// Refreshes the LLMConfig snapshot fields that are owned by the model card
/// (`is_reasoning`, `context_window`) from the current `llm_model` row.
///
/// The frontend snapshots these fields at agent save time, but the snapshot
/// can become stale: editing the model card (e.g. toggling "reasoning") does
/// not propagate to existing agents until they are re-saved, and the
/// `loadAllLLMData()` cache inside `AgentForm.svelte` is only refreshed at
/// `onMount`. By re-reading from the DB just before persisting the agent we
/// make the model card the single source of truth for these fields and
/// eliminate the entire class of "snapshot stale" bugs (see ERR_LLM_010).
///
/// Fields that may legitimately diverge from the model defaults
/// (`temperature`, `max_tokens`) are NOT overridden — those stay user-editable.
///
/// If no matching `llm_model` row exists (e.g. ad-hoc model name not yet
/// registered), the snapshot is kept as-is so creation does not fail.
async fn hydrate_llm_from_model(
    db: &crate::db::DBClient,
    llm: &mut crate::models::LLMConfig,
) -> Result<(), String> {
    use serde_json::json;

    // `llm_model.provider` is always stored lowercase (see seed.rs normalization),
    // and `ProviderType::as_id()` also returns lowercase. Skip the parse round-trip
    // — `to_lowercase()` matches both builtin and Custom providers.
    let provider_id = llm.provider.to_lowercase();
    let api_name = llm.model.trim();
    if api_name.is_empty() {
        return Ok(());
    }

    let query = "SELECT is_reasoning, context_window FROM llm_model \
                 WHERE provider = $provider AND api_name = $api_name LIMIT 1";
    let rows: Vec<serde_json::Value> = db
        .query_with_params(
            query,
            vec![
                ("provider".to_string(), json!(provider_id)),
                ("api_name".to_string(), json!(api_name)),
            ],
        )
        .await
        .map_err(|e| format!("Failed to look up model card: {}", e))?;

    if let Some(row) = rows.first() {
        match row.get("is_reasoning").and_then(|v| v.as_bool()) {
            Some(is_reasoning) => llm.is_reasoning = is_reasoning,
            None => {
                if row.get("is_reasoning").is_some() {
                    warn!(
                        provider = %provider_id,
                        api_name = %api_name,
                        "llm_model.is_reasoning is not a bool — keeping snapshot value (schema drift?)"
                    );
                }
            }
        }
        match row.get("context_window").and_then(|v| v.as_u64()) {
            Some(ctx) => llm.context_window = Some(ctx as usize),
            None => {
                if row.get("context_window").is_some() {
                    warn!(
                        provider = %provider_id,
                        api_name = %api_name,
                        "llm_model.context_window is not an unsigned int — keeping snapshot value (schema drift?)"
                    );
                }
                // Field absent from row: leave snapshot as-is.
            }
        }
    }

    Ok(())
}

/// Creates a new agent
///
/// Validates the configuration, persists to database, and registers in memory.
#[tauri::command]
#[instrument(name = "create_agent", skip(state, config), fields(agent_name = %config.name))]
pub async fn create_agent(
    config: AgentConfigCreate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating new agent");

    let mut validated = validate_agent_create(&config).map_err(|e| {
        warn!(error = %e, "Agent validation failed");
        e
    })?;

    check_agent_name_unique(&state.db, &validated.name, None)
        .await
        .map_err(|e| {
            warn!(error = %e, "Agent name uniqueness check failed");
            e
        })?;

    // Override the snapshot fields owned by the model card with the current
    // DB values, so a stale snapshot from the frontend cannot persist a wrong
    // is_reasoning / context_window. See ERR_LLM_010.
    hydrate_llm_from_model(&state.db, &mut validated.llm)
        .await
        .map_err(|e| {
            warn!(error = %e, "Failed to hydrate LLM config from model card");
            e
        })?;

    let agent_id = uuid::Uuid::new_v4().to_string();

    // Destructure validated config
    let AgentConfigCreate {
        name,
        lifecycle,
        llm,
        tools,
        mcp_servers,
        skills,
        folders,
        require_file_confirmation,
        system_prompt,
        max_tool_iterations,
        reasoning_effort,
    } = validated;

    let lifecycle_str = match lifecycle {
        Lifecycle::Permanent => "permanent",
        Lifecycle::Temporary => "temporary",
    };

    let agent_config = AgentConfig {
        id: agent_id.clone(),
        name,
        lifecycle,
        llm,
        tools,
        mcp_servers,
        skills,
        folders,
        require_file_confirmation,
        system_prompt,
        max_tool_iterations,
        reasoning_effort,
    };

    let reasoning_sql = format_reasoning_effort(&agent_config);

    let query = format!(
        "CREATE agent:`{agent_id}` CONTENT {{
            id: '{agent_id}',
            name: $name,
            lifecycle: $lifecycle,
            llm: $llm,
            tools: $tools,
            mcp_servers: $mcp_servers,
            skills: $skills,
            folders: $folders,
            require_file_confirmation: $require_file_confirmation,
            system_prompt: $system_prompt,
            max_tool_iterations: $max_tool_iterations,
            reasoning_effort: {reasoning_sql},
            created_at: time::now(),
            updated_at: time::now()
        }}"
    );

    let llm_value = serde_json::to_value(&agent_config.llm)
        .map_err(|e| format!("Failed to serialize LLM config: {}", e))?;

    state
        .db
        .execute_with_params(
            &query,
            vec![
                ("name".to_string(), json!(agent_config.name)),
                ("lifecycle".to_string(), json!(lifecycle_str)),
                ("llm".to_string(), llm_value),
                ("tools".to_string(), json!(agent_config.tools)),
                ("mcp_servers".to_string(), json!(agent_config.mcp_servers)),
                ("skills".to_string(), json!(agent_config.skills)),
                ("folders".to_string(), json!(agent_config.folders)),
                (
                    "require_file_confirmation".to_string(),
                    json!(agent_config.require_file_confirmation),
                ),
                (
                    "system_prompt".to_string(),
                    json!(agent_config.system_prompt),
                ),
                (
                    "max_tool_iterations".to_string(),
                    json!(agent_config.max_tool_iterations),
                ),
            ],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to persist agent to database");
            format!("Failed to persist agent: {}", e)
        })?;

    register_agent_runtime(state.inner(), &agent_id, agent_config).await;

    info!(agent_id = %agent_id, "Agent created successfully");
    Ok(agent_id)
}

/// Updates an existing agent
///
/// Validates the configuration, updates database, and re-registers in memory.
#[tauri::command]
#[instrument(name = "update_agent", skip(state, config), fields(agent_id = %agent_id))]
pub async fn update_agent(
    agent_id: String,
    config: AgentConfigUpdate,
    state: State<'_, AppState>,
) -> Result<AgentConfig, String> {
    info!("Updating agent");

    let validated_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    let existing = state.registry.get(&validated_id).await.ok_or_else(|| {
        warn!(agent_id = %validated_id, "Agent not found");
        "Agent not found".to_string()
    })?;

    let mut updated_config = merge_agent_config(&config, existing.config())?;
    updated_config.id = validated_id.clone();

    check_agent_name_unique(&state.db, &updated_config.name, Some(&validated_id))
        .await
        .map_err(|e| {
            warn!(error = %e, "Agent name uniqueness check failed on update");
            e
        })?;

    // Override the snapshot fields owned by the model card with the current
    // DB values, so editing the model and re-saving the agent is sufficient
    // to refresh is_reasoning / context_window even if the frontend cache is
    // stale. See ERR_LLM_010.
    hydrate_llm_from_model(&state.db, &mut updated_config.llm)
        .await
        .map_err(|e| {
            warn!(error = %e, "Failed to hydrate LLM config from model card");
            e
        })?;

    let reasoning_sql = format_reasoning_effort(&updated_config);

    let query = format!(
        "UPDATE agent:`{validated_id}` SET
            name = $name,
            llm = $llm,
            tools = $tools,
            mcp_servers = $mcp_servers,
            skills = $skills,
            folders = $folders,
            require_file_confirmation = $require_file_confirmation,
            system_prompt = $system_prompt,
            max_tool_iterations = $max_tool_iterations,
            reasoning_effort = {reasoning_sql},
            updated_at = time::now()"
    );

    let llm_value = serde_json::to_value(&updated_config.llm)
        .map_err(|e| format!("Failed to serialize LLM config: {}", e))?;

    state
        .db
        .execute_with_params(
            &query,
            vec![
                ("name".to_string(), json!(updated_config.name)),
                ("llm".to_string(), llm_value),
                ("tools".to_string(), json!(updated_config.tools)),
                ("mcp_servers".to_string(), json!(updated_config.mcp_servers)),
                ("skills".to_string(), json!(updated_config.skills)),
                ("folders".to_string(), json!(updated_config.folders)),
                (
                    "require_file_confirmation".to_string(),
                    json!(updated_config.require_file_confirmation),
                ),
                (
                    "system_prompt".to_string(),
                    json!(updated_config.system_prompt),
                ),
                (
                    "max_tool_iterations".to_string(),
                    json!(updated_config.max_tool_iterations),
                ),
            ],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update agent in database");
            format!("Failed to update agent: {}", e)
        })?;

    state.registry.unregister_any(&validated_id).await;
    register_agent_runtime(state.inner(), &validated_id, updated_config.clone()).await;

    info!(agent_id = %validated_id, "Agent updated successfully");
    Ok(updated_config)
}

/// Deletes an agent
///
/// Removes from database and unregisters from memory.
#[tauri::command]
#[instrument(name = "delete_agent", skip(state), fields(agent_id = %agent_id))]
pub async fn delete_agent(agent_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting agent");

    let validated_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    if state.registry.get(&validated_id).await.is_none() {
        warn!(agent_id = %validated_id, "Agent not found");
        return Err("Agent not found".to_string());
    }

    let query = format!("DELETE agent:`{}`", validated_id);
    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to delete agent from database");
        format!("Failed to delete agent: {}", e)
    })?;

    state.registry.unregister_any(&validated_id).await;

    info!(agent_id = %validated_id, "Agent deleted successfully");
    Ok(())
}
