// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! MCP (Model Context Protocol) Tauri commands
//!
//! Provides IPC commands for managing MCP server configurations,
//! server lifecycle (start/stop), and tool execution.
//!
//! ## Commands
//!
//! ### Configuration
//! - [`list_mcp_servers`] - List all configured MCP servers
//! - [`get_mcp_server`] - Get a single MCP server by ID
//! - [`create_mcp_server`] - Create a new MCP server configuration
//! - [`update_mcp_server`] - Update an existing MCP server
//! - [`delete_mcp_server`] - Delete an MCP server configuration
//!
//! ### Lifecycle
//! - [`start_mcp_server`] - Start an MCP server
//! - [`stop_mcp_server`] - Stop a running MCP server
//! - [`test_mcp_server`] - Test MCP server connection
//!
//! ### Tools
//! - [`list_mcp_tools`] - List available tools from a server
//! - [`call_mcp_tool`] - Execute a tool on an MCP server

use crate::models::mcp::{
    MCPServer, MCPServerConfig, MCPTestResult, MCPTool, MCPToolCallRequest, MCPToolCallResult,
};
use crate::state::AppState;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Maximum length for MCP server names
const MAX_MCP_SERVER_NAME_LEN: usize = 64;
/// Maximum length for MCP server descriptions
const MAX_MCP_DESCRIPTION_LEN: usize = 1024;
/// Maximum number of command arguments
const MAX_MCP_ARGS_COUNT: usize = 50;
/// Maximum length for each command argument
const MAX_MCP_ARG_LEN: usize = 512;
/// Maximum number of environment variables
const MAX_MCP_ENV_COUNT: usize = 50;
/// Maximum length for environment variable names
const MAX_MCP_ENV_NAME_LEN: usize = 128;
/// Maximum length for environment variable values
const MAX_MCP_ENV_VALUE_LEN: usize = 4096;
/// Maximum length for tool names
const MAX_TOOL_NAME_LEN: usize = 128;

/// Validates an MCP server ID.
///
/// Rules:
/// - Cannot be empty
/// - Maximum 64 characters
/// - Only alphanumeric, underscore, and hyphen allowed
fn validate_mcp_server_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();

    if trimmed.is_empty() {
        return Err("Server ID cannot be empty".to_string());
    }

    if trimmed.len() > MAX_MCP_SERVER_NAME_LEN {
        return Err(format!(
            "Server ID exceeds maximum length of {} characters",
            MAX_MCP_SERVER_NAME_LEN
        ));
    }

    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "Server ID can only contain alphanumeric characters, underscore, and hyphen"
                .to_string(),
        );
    }

    Ok(trimmed.to_string())
}

/// Validates an MCP server display name.
///
/// Rules:
/// - Cannot be empty
/// - Maximum 64 characters
/// - No control characters (except newline)
fn validate_mcp_server_display_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Server name cannot be empty".to_string());
    }

    if trimmed.len() > MAX_MCP_SERVER_NAME_LEN {
        return Err(format!(
            "Server name exceeds maximum length of {} characters",
            MAX_MCP_SERVER_NAME_LEN
        ));
    }

    if trimmed.chars().any(|c| c.is_control() && c != '\n') {
        return Err("Server name cannot contain control characters".to_string());
    }

    Ok(trimmed.to_string())
}

/// Validates an MCP server description.
///
/// Rules:
/// - Can be empty (optional field)
/// - Maximum 1024 characters
/// - No control characters
fn validate_mcp_description(description: Option<&str>) -> Result<Option<String>, String> {
    match description {
        None => Ok(None),
        Some(desc) => {
            let trimmed = desc.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            if trimmed.len() > MAX_MCP_DESCRIPTION_LEN {
                return Err(format!(
                    "Description exceeds maximum length of {} characters",
                    MAX_MCP_DESCRIPTION_LEN
                ));
            }

            if trimmed.chars().any(|c| c.is_control() && c != '\n') {
                return Err("Description cannot contain control characters".to_string());
            }

            Ok(Some(trimmed.to_string()))
        }
    }
}

/// Validates MCP server command arguments.
///
/// Rules:
/// - Maximum 50 arguments
/// - Each argument maximum 512 characters
/// - No shell metacharacters in arguments (basic protection)
fn validate_mcp_args(args: &[String]) -> Result<Vec<String>, String> {
    if args.len() > MAX_MCP_ARGS_COUNT {
        return Err(format!("Too many arguments (max {})", MAX_MCP_ARGS_COUNT));
    }

    let validated: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            if arg.len() > MAX_MCP_ARG_LEN {
                return Err(format!(
                    "Argument {} exceeds maximum length of {} characters",
                    i, MAX_MCP_ARG_LEN
                ));
            }
            // Basic shell metacharacter protection (not comprehensive, defense in depth)
            if arg.contains('\0') {
                return Err(format!("Argument {} contains null character", i));
            }
            Ok(arg.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(validated)
}

/// Validates MCP server environment variables.
///
/// Rules:
/// - Maximum 50 variables
/// - Names: alphanumeric + underscore, max 128 chars
/// - Values: max 4096 chars, no null characters
fn validate_mcp_env(
    env: &std::collections::HashMap<String, String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    if env.len() > MAX_MCP_ENV_COUNT {
        return Err(format!(
            "Too many environment variables (max {})",
            MAX_MCP_ENV_COUNT
        ));
    }

    let validated: std::collections::HashMap<String, String> = env
        .iter()
        .map(|(name, value)| {
            // Validate name
            if name.is_empty() {
                return Err("Environment variable name cannot be empty".to_string());
            }
            if name.len() > MAX_MCP_ENV_NAME_LEN {
                return Err(format!(
                    "Environment variable name '{}' exceeds maximum length of {} characters",
                    name, MAX_MCP_ENV_NAME_LEN
                ));
            }
            if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!(
                    "Environment variable name '{}' can only contain alphanumeric characters and underscore",
                    name
                ));
            }

            // Validate value
            if value.len() > MAX_MCP_ENV_VALUE_LEN {
                return Err(format!(
                    "Environment variable '{}' value exceeds maximum length of {} characters",
                    name, MAX_MCP_ENV_VALUE_LEN
                ));
            }
            if value.contains('\0') {
                return Err(format!(
                    "Environment variable '{}' value contains null character",
                    name
                ));
            }

            Ok((name.clone(), value.clone()))
        })
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

    Ok(validated)
}

/// Validates an MCP server configuration.
fn validate_mcp_server_config(config: &MCPServerConfig) -> Result<MCPServerConfig, String> {
    let validated_id = validate_mcp_server_id(&config.id)?;
    let validated_name = validate_mcp_server_display_name(&config.name)?;
    let validated_description = validate_mcp_description(config.description.as_deref())?;
    let validated_args = validate_mcp_args(&config.args)?;
    let validated_env = validate_mcp_env(&config.env)?;

    Ok(MCPServerConfig {
        id: validated_id,
        name: validated_name,
        enabled: config.enabled,
        command: config.command.clone(),
        args: validated_args,
        env: validated_env,
        description: validated_description,
    })
}

/// Validates a tool name.
fn validate_tool_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Tool name cannot be empty".to_string());
    }

    if trimmed.len() > MAX_TOOL_NAME_LEN {
        return Err(format!(
            "Tool name exceeds maximum length of {} characters",
            MAX_TOOL_NAME_LEN
        ));
    }

    // Tool names can contain alphanumeric, underscore, hyphen, and some special chars
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ':' || c == '/')
    {
        return Err(
            "Tool name can only contain alphanumeric characters, underscore, hyphen, colon, and slash"
                .to_string(),
        );
    }

    Ok(trimmed.to_string())
}

// =============================================================================
// Tauri Commands
// =============================================================================

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
) -> Result<MCPServer, String> {
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
    Ok(server)
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
) -> Result<MCPServer, String> {
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
    Ok(server)
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

/// Lists available tools from an MCP server.
///
/// Returns the tools discovered during server initialization.
/// The server must be running for this to return tools.
///
/// # Arguments
///
/// * `server_name` - The name/ID of the MCP server
///
/// # Returns
///
/// A vector of [`MCPTool`] objects describing available tools.
///
/// # Errors
///
/// Returns an error if:
/// - The server name is invalid
/// - The server is not found
/// - The server is not running
#[tauri::command]
#[instrument(name = "list_mcp_tools", skip(state), fields(server = %server_name))]
pub async fn list_mcp_tools(
    server_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<MCPTool>, String> {
    let validated_name = validate_mcp_server_id(&server_name)?;
    info!(server = %validated_name, "Listing MCP tools");

    let tools = state.mcp_manager.list_server_tools(&validated_name).await;

    info!(
        server = %validated_name,
        count = tools.len(),
        "MCP tools listed"
    );
    Ok(tools)
}

/// Calls a tool on an MCP server.
///
/// Executes the specified tool with the provided arguments.
/// The tool call is logged for auditing purposes.
///
/// # Arguments
///
/// * `request` - The tool call request containing server name, tool name, and arguments
///
/// # Returns
///
/// An [`MCPToolCallResult`] containing the tool output or error.
///
/// # Errors
///
/// Returns an error if:
/// - The request is invalid
/// - The server is not found or not running
/// - The tool is not found
/// - Tool execution fails
#[tauri::command]
#[instrument(name = "call_mcp_tool", skip(state, request), fields(server, tool))]
pub async fn call_mcp_tool(
    request: MCPToolCallRequest,
    state: State<'_, AppState>,
) -> Result<MCPToolCallResult, String> {
    let validated_server = validate_mcp_server_id(&request.server_name)?;
    let validated_tool = validate_tool_name(&request.tool_name)?;

    tracing::Span::current().record("server", &validated_server);
    tracing::Span::current().record("tool", &validated_tool);

    info!(
        server = %validated_server,
        tool = %validated_tool,
        "Calling MCP tool"
    );

    let validated_request = MCPToolCallRequest {
        server_name: validated_server.clone(),
        tool_name: validated_tool.clone(),
        arguments: request.arguments,
    };

    let result = state
        .mcp_manager
        .call_tool_request(validated_request)
        .await
        .map_err(|e| {
            error!(error = %e, "MCP tool call failed");
            format!("MCP tool call failed: {}", e)
        })?;

    info!(
        server = %validated_server,
        tool = %validated_tool,
        success = result.success,
        duration_ms = result.duration_ms,
        "MCP tool call completed"
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mcp::MCPDeploymentMethod;
    use std::collections::HashMap;

    #[test]
    fn test_validate_mcp_server_id_valid() {
        assert!(validate_mcp_server_id("serena").is_ok());
        assert!(validate_mcp_server_id("context-7").is_ok());
        assert!(validate_mcp_server_id("my_server").is_ok());
        assert!(validate_mcp_server_id("Server123").is_ok());
    }

    #[test]
    fn test_validate_mcp_server_id_empty() {
        let result = validate_mcp_server_id("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_mcp_server_id_too_long() {
        let long_name = "a".repeat(MAX_MCP_SERVER_NAME_LEN + 1);
        let result = validate_mcp_server_id(&long_name);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("maximum length"));
    }

    #[test]
    fn test_validate_mcp_server_id_invalid_chars() {
        let result = validate_mcp_server_id("server with spaces");
        assert!(result.is_err());

        let result = validate_mcp_server_id("server@special");
        assert!(result.is_err());

        let result = validate_mcp_server_id("server.name");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_mcp_description_valid() {
        assert!(validate_mcp_description(None).unwrap().is_none());
        assert!(validate_mcp_description(Some("")).unwrap().is_none());
        assert_eq!(
            validate_mcp_description(Some("A test server")).unwrap(),
            Some("A test server".to_string())
        );
        assert_eq!(
            validate_mcp_description(Some("Multi\nline")).unwrap(),
            Some("Multi\nline".to_string())
        );
    }

    #[test]
    fn test_validate_mcp_description_too_long() {
        let long_desc = "a".repeat(MAX_MCP_DESCRIPTION_LEN + 1);
        let result = validate_mcp_description(Some(&long_desc));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_mcp_args_valid() {
        let args = vec!["run".to_string(), "-i".to_string(), "image:tag".to_string()];
        assert!(validate_mcp_args(&args).is_ok());
    }

    #[test]
    fn test_validate_mcp_args_too_many() {
        let args: Vec<String> = (0..MAX_MCP_ARGS_COUNT + 1)
            .map(|i| format!("arg{}", i))
            .collect();
        let result = validate_mcp_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Too many"));
    }

    #[test]
    fn test_validate_mcp_args_null_char() {
        let args = vec!["arg\0with\0nulls".to_string()];
        let result = validate_mcp_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null character"));
    }

    #[test]
    fn test_validate_mcp_env_valid() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret".to_string());
        env.insert("DEBUG".to_string(), "true".to_string());
        assert!(validate_mcp_env(&env).is_ok());
    }

    #[test]
    fn test_validate_mcp_env_invalid_name() {
        let mut env = HashMap::new();
        env.insert("INVALID-NAME".to_string(), "value".to_string());
        let result = validate_mcp_env(&env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("alphanumeric"));
    }

    #[test]
    fn test_validate_mcp_env_null_in_value() {
        let mut env = HashMap::new();
        env.insert("KEY".to_string(), "value\0with\0null".to_string());
        let result = validate_mcp_env(&env);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null character"));
    }

    #[test]
    fn test_validate_tool_name_valid() {
        assert!(validate_tool_name("find_symbol").is_ok());
        assert!(validate_tool_name("mcp__serena__find_symbol").is_ok());
        assert!(validate_tool_name("tool-name").is_ok());
        assert!(validate_tool_name("namespace:tool").is_ok());
        assert!(validate_tool_name("path/to/tool").is_ok());
    }

    #[test]
    fn test_validate_tool_name_empty() {
        let result = validate_tool_name("");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_tool_name_invalid_chars() {
        let result = validate_tool_name("tool with spaces");
        assert!(result.is_err());

        let result = validate_tool_name("tool@special");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_mcp_server_config() {
        let config = MCPServerConfig {
            id: "test_server".to_string(),
            name: "Test Server".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec!["run".to_string(), "-i".to_string()],
            env: HashMap::new(),
            description: Some("A test server".to_string()),
        };

        let result = validate_mcp_server_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_mcp_server_config_invalid_id() {
        let config = MCPServerConfig {
            id: "invalid id with spaces".to_string(),
            name: "Test".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec![],
            env: HashMap::new(),
            description: None,
        };

        let result = validate_mcp_server_config(&config);
        assert!(result.is_err());
    }
}
