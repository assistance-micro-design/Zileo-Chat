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

//! MCP CRUD commands
//!
//! Tauri commands for listing, getting, creating, updating, and deleting
//! MCP server configurations.

use crate::commands::mcp::validation::{
    check_mcp_http_warning, validate_mcp_server_config, validate_mcp_server_id,
};
use crate::models::mcp::{MCPServer, MCPServerConfig, MCPServerResponse};
use crate::state::AppState;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Lists all configured MCP servers.
///
/// Returns all MCP servers with their current status, tools, and resources.
/// Servers that are not running will have empty tools/resources lists.
///
/// # Returns
///
/// A vector of [`MCPServer`] objects containing configuration and runtime state.
///
/// # Errors
///
/// Returns an error string if the server list cannot be retrieved.
#[tauri::command]
#[instrument(name = "list_mcp_servers", skip(state))]
pub async fn list_mcp_servers(state: State<'_, AppState>) -> Result<Vec<MCPServer>, String> {
    info!("Listing MCP servers");

    let servers = state.mcp_manager.list_servers().await.map_err(|e| {
        error!(error = %e, "Failed to list MCP servers");
        format!("Failed to list MCP servers: {}", e)
    })?;

    info!(count = servers.len(), "MCP servers listed");
    Ok(servers)
}

/// Gets a single MCP server by ID.
///
/// # Arguments
///
/// * `id` - The unique identifier of the MCP server
///
/// # Returns
///
/// The [`MCPServer`] if found, with current status and discovered tools/resources.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The server is not found
#[tauri::command]
#[instrument(name = "get_mcp_server", skip(state))]
pub async fn get_mcp_server(id: String, state: State<'_, AppState>) -> Result<MCPServer, String> {
    let validated_id = validate_mcp_server_id(&id)?;
    info!(id = %validated_id, "Getting MCP server");

    state
        .mcp_manager
        .get_server(&validated_id)
        .await
        .ok_or_else(|| format!("MCP server '{}' not found", validated_id))
}

/// Creates a new MCP server configuration.
///
/// The server is saved to the database but not started automatically
/// unless `enabled` is true, in which case it will be started.
///
/// # Arguments
///
/// * `config` - The MCP server configuration
///
/// # Returns
///
/// The created [`MCPServer`] with initial status.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration is invalid
/// - A server with the same ID already exists
/// - The server fails to start (if enabled)
#[tauri::command]
#[instrument(name = "create_mcp_server", skip(state, config), fields(server_id))]
pub async fn create_mcp_server(
    config: MCPServerConfig,
    state: State<'_, AppState>,
) -> Result<MCPServerResponse, String> {
    // Log what we received from frontend BEFORE validation
    info!(
        name = %config.name,
        env_count_received = config.env.len(),
        env_keys_received = ?config.env.keys().collect::<Vec<_>>(),
        "Received MCP server config from frontend"
    );

    let validated_config = validate_mcp_server_config(&config)?;
    tracing::Span::current().record("server_id", &validated_config.id);
    info!(
        name = %validated_config.name,
        command = %validated_config.command,
        enabled = validated_config.enabled,
        env_count_validated = validated_config.env.len(),
        "Creating MCP server"
    );

    // Check if server already exists
    if state
        .mcp_manager
        .get_server(&validated_config.id)
        .await
        .is_some()
    {
        return Err(format!(
            "MCP server with ID '{}' already exists",
            validated_config.id
        ));
    }

    let warning = check_mcp_http_warning(&validated_config);
    if warning.is_some() {
        warn!(
            id = %validated_config.id,
            name = %validated_config.name,
            "MCP server created with insecure HTTP URL"
        );
    }

    // Spawn the server (this also saves to DB)
    let server = state
        .mcp_manager
        .spawn_server(validated_config)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create MCP server");
            format!("Failed to create MCP server: {}", e)
        })?;

    info!(
        id = %server.config.id,
        status = %server.status,
        "MCP server created"
    );
    Ok(MCPServerResponse { server, warning })
}

/// Updates an existing MCP server configuration.
///
/// If the server is running, it will be restarted with the new configuration.
///
/// # Arguments
///
/// * `id` - The unique identifier of the server to update
/// * `config` - The new configuration
///
/// # Returns
///
/// The updated [`MCPServer`] with current status.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The configuration is invalid
/// - The server is not found
/// - The update fails
#[tauri::command]
#[instrument(name = "update_mcp_server", skip(state, config), fields(server_id = %id))]
pub async fn update_mcp_server(
    id: String,
    config: MCPServerConfig,
    state: State<'_, AppState>,
) -> Result<MCPServerResponse, String> {
    let validated_id = validate_mcp_server_id(&id)?;
    let validated_config = validate_mcp_server_config(&config)?;

    // Ensure the ID in config matches the path ID
    if validated_config.id != validated_id {
        return Err("Server ID in config must match the path ID".to_string());
    }

    info!(
        id = %validated_id,
        name = %validated_config.name,
        "Updating MCP server"
    );

    // Check if server exists
    if state.mcp_manager.get_server(&validated_id).await.is_none() {
        return Err(format!("MCP server '{}' not found", validated_id));
    }

    let warning = check_mcp_http_warning(&validated_config);
    if warning.is_some() {
        warn!(
            id = %validated_id,
            name = %validated_config.name,
            "MCP server updated with insecure HTTP URL"
        );
    }

    // Update the configuration in database
    state
        .mcp_manager
        .update_server_config(&validated_config)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update MCP server");
            format!("Failed to update MCP server: {}", e)
        })?;

    // If the server is running, restart it with new config
    if let Some(current_server) = state.mcp_manager.get_server(&validated_id).await {
        if current_server.status == crate::models::mcp::MCPServerStatus::Running {
            // Restart to apply new configuration
            let _ = state.mcp_manager.restart_server(&validated_id).await;
        }
    }

    // Get updated server state
    let server = state
        .mcp_manager
        .get_server(&validated_id)
        .await
        .ok_or_else(|| format!("MCP server '{}' not found after update", validated_id))?;

    info!(
        id = %server.config.id,
        status = %server.status,
        "MCP server updated"
    );
    Ok(MCPServerResponse { server, warning })
}

/// Deletes an MCP server configuration.
///
/// If the server is running, it will be stopped before deletion.
/// The server configuration is removed from the database.
///
/// # Arguments
///
/// * `id` - The unique identifier of the server to delete
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The server is not found
/// - Deletion fails
#[tauri::command]
#[instrument(name = "delete_mcp_server", skip(state), fields(server_id = %id))]
pub async fn delete_mcp_server(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let validated_id = validate_mcp_server_id(&id)?;
    info!(id = %validated_id, "Deleting MCP server");

    // Check if server exists
    if state.mcp_manager.get_server(&validated_id).await.is_none() {
        return Err(format!("MCP server '{}' not found", validated_id));
    }

    state
        .mcp_manager
        .delete_server_config(&validated_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete MCP server");
            format!("Failed to delete MCP server: {}", e)
        })?;

    info!(id = %validated_id, "MCP server deleted");
    Ok(())
}
