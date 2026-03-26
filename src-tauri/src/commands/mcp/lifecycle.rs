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

//! MCP lifecycle commands
//!
//! Tauri commands for starting, stopping, and testing MCP servers.

use crate::commands::mcp::validation::{validate_mcp_server_config, validate_mcp_server_id};
use crate::models::mcp::{MCPServer, MCPServerConfig, MCPTestResult};
use crate::state::AppState;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Tests an MCP server connection.
///
/// Spawns a temporary server process, initializes the MCP protocol,
/// and retrieves available tools and resources. The server is stopped
/// after the test completes.
///
/// # Arguments
///
/// * `config` - The MCP server configuration to test
///
/// # Returns
///
/// An [`MCPTestResult`] containing success status, discovered tools/resources,
/// and connection latency.
///
/// # Errors
///
/// Returns an error if:
/// - The configuration is invalid
/// - The test fails to execute
#[tauri::command]
#[instrument(name = "test_mcp_server", skip(state, config))]
pub async fn test_mcp_server(
    config: MCPServerConfig,
    state: State<'_, AppState>,
) -> Result<MCPTestResult, String> {
    let validated_config = validate_mcp_server_config(&config)?;
    info!(
        name = %validated_config.name,
        command = %validated_config.command,
        "Testing MCP server connection"
    );

    let result = state.mcp_manager.test_server(validated_config).await;

    if !result.success {
        warn!(message = %result.message, "MCP server test failed");
    } else {
        info!(
            success = result.success,
            tools_count = result.tools.len(),
            latency_ms = result.latency_ms,
            "MCP server test completed"
        );
    }
    Ok(result)
}

/// Starts an MCP server.
///
/// Spawns the server process and initializes the MCP protocol.
/// Tools and resources are discovered during initialization.
///
/// # Arguments
///
/// * `id` - The unique identifier of the server to start
///
/// # Returns
///
/// The [`MCPServer`] with updated status and discovered tools/resources.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The server is not found
/// - The server is already running
/// - Starting fails
#[tauri::command]
#[instrument(name = "start_mcp_server", skip(state), fields(server_id = %id))]
pub async fn start_mcp_server(id: String, state: State<'_, AppState>) -> Result<MCPServer, String> {
    let validated_id = validate_mcp_server_id(&id)?;
    info!(id = %validated_id, "Starting MCP server");

    // Get current server state
    let server = state
        .mcp_manager
        .get_server(&validated_id)
        .await
        .ok_or_else(|| format!("MCP server '{}' not found", validated_id))?;

    // Check if already running
    if server.status == crate::models::mcp::MCPServerStatus::Running {
        return Err(format!("MCP server '{}' is already running", validated_id));
    }

    // Restart the server (this handles the spawn)
    let updated_server = state
        .mcp_manager
        .restart_server(&validated_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to start MCP server");
            format!("Failed to start MCP server: {}", e)
        })?;

    info!(
        id = %updated_server.config.id,
        status = %updated_server.status,
        tools_count = updated_server.tools.len(),
        "MCP server started"
    );
    Ok(updated_server)
}

/// Stops a running MCP server.
///
/// Gracefully terminates the server process. The configuration
/// remains in the database and can be restarted later.
///
/// # Arguments
///
/// * `id` - The unique identifier of the server to stop
///
/// # Returns
///
/// The [`MCPServer`] with updated status (stopped).
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The server is not found
/// - The server is not running
/// - Stopping fails
#[tauri::command]
#[instrument(name = "stop_mcp_server", skip(state), fields(server_id = %id))]
pub async fn stop_mcp_server(id: String, state: State<'_, AppState>) -> Result<MCPServer, String> {
    let validated_id = validate_mcp_server_id(&id)?;
    info!(id = %validated_id, "Stopping MCP server");

    // Get current server state
    let server = state
        .mcp_manager
        .get_server(&validated_id)
        .await
        .ok_or_else(|| format!("MCP server '{}' not found", validated_id))?;

    // Check if already stopped
    if server.status == crate::models::mcp::MCPServerStatus::Stopped {
        return Err(format!("MCP server '{}' is already stopped", validated_id));
    }

    // Stop the server
    state
        .mcp_manager
        .stop_server(&validated_id)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to stop MCP server");
            format!("Failed to stop MCP server: {}", e)
        })?;

    // Get updated state
    let updated_server = state
        .mcp_manager
        .get_server(&validated_id)
        .await
        .ok_or_else(|| format!("MCP server '{}' not found after stop", validated_id))?;

    info!(
        id = %updated_server.config.id,
        status = %updated_server.status,
        "MCP server stopped"
    );
    Ok(updated_server)
}
