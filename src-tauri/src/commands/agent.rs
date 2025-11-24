// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use tauri::State;
use crate::{AppState, models::AgentConfig};

/// Lists all available agent IDs
#[tauri::command]
pub async fn list_agents(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let agent_ids = state.registry.list().await;
    Ok(agent_ids)
}

/// Gets agent configuration by ID
#[tauri::command]
pub async fn get_agent_config(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<AgentConfig, String> {
    let agent = state
        .registry
        .get(&agent_id)
        .await
        .ok_or("Agent not found")?;

    Ok(agent.config().clone())
}
