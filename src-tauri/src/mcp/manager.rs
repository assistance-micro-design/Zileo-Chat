// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! MCP Manager
//!
//! Central management component for MCP servers. Handles:
//! - Server registry and lifecycle management
//! - Database persistence for server configurations
//! - Tool routing across multiple servers
//! - Automatic server startup on application launch
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │            MCPManager                   │
//! │  - servers: HashMap<name, MCPClient>    │
//! │  - db: Arc<DBClient>                    │
//! └─────────────────┬───────────────────────┘
//!                   │
//!     ┌─────────────┼─────────────┐
//!     ↓             ↓             ↓
//! ┌───────────┐ ┌───────────┐ ┌───────────┐
//! │MCPClient  │ │MCPClient  │ │MCPClient  │
//! │ "serena"  │ │ "context7"│ │ "magic"   │
//! └───────────┘ └───────────┘ └───────────┘
//! ```
//!
//! ## Database Storage
//!
//! Server configurations are stored in the `mcp_server` table and
//! automatically loaded on startup. Tool calls are logged to `mcp_call_log`.

use crate::db::DBClient;
use crate::mcp::client::MCPClient;
use crate::mcp::{MCPError, MCPResult};
use crate::models::mcp::{
    MCPCallLog, MCPServer, MCPServerConfig, MCPServerCreate, MCPServerStatus, MCPTestResult,
    MCPTool, MCPToolCallRequest, MCPToolCallResult,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

/// MCP Manager
///
/// Manages the lifecycle of multiple MCP servers and provides
/// a unified interface for tool invocation.
///
/// # Thread Safety
///
/// The manager uses `RwLock` internally and is safe to share
/// across threads via `Arc<MCPManager>`.
///
/// # Example
///
/// ```rust,ignore
/// let db = Arc::new(DBClient::new("db_path").await?);
/// let manager = MCPManager::new(db).await?;
///
/// // Load servers from database
/// manager.load_from_db().await?;
///
/// // Or spawn a new server
/// let config = MCPServerConfig { /* ... */ };
/// let server = manager.spawn_server(config).await?;
///
/// // Call a tool
/// let result = manager.call_tool("serena", "find_symbol", json!({"name": "Foo"})).await?;
/// ```
pub struct MCPManager {
    /// Connected clients indexed by server name
    clients: RwLock<HashMap<String, MCPClient>>,
    /// Database client for persistence
    db: Arc<DBClient>,
}

impl MCPManager {
    /// Creates a new MCP manager
    ///
    /// # Arguments
    ///
    /// * `db` - Database client for persisting server configurations
    ///
    /// # Returns
    ///
    /// Returns a new `MCPManager` instance without any servers loaded.
    /// Call `load_from_db()` to load saved configurations.
    pub async fn new(db: Arc<DBClient>) -> MCPResult<Self> {
        info!("Creating MCP manager");

        Ok(Self {
            clients: RwLock::new(HashMap::new()),
            db,
        })
    }

    /// Loads server configurations from the database
    ///
    /// Queries all enabled servers from the database and spawns them.
    /// Servers that fail to start are logged but don't prevent other
    /// servers from starting.
    pub async fn load_from_db(&self) -> MCPResult<()> {
        info!("Loading MCP servers from database");

        let configs = self.get_saved_configs().await?;
        let enabled_configs: Vec<_> = configs.into_iter().filter(|c| c.enabled).collect();

        info!(
            total_configs = enabled_configs.len(),
            "Found enabled MCP server configurations"
        );

        for config in enabled_configs {
            match self.spawn_server_internal(config.clone()).await {
                Ok(_) => {
                    info!(
                        server_id = %config.id,
                        server_name = %config.name,
                        "MCP server started successfully"
                    );
                }
                Err(e) => {
                    warn!(
                        server_id = %config.id,
                        server_name = %config.name,
                        error = %e,
                        "Failed to start MCP server (will be marked as error)"
                    );
                }
            }
        }

        Ok(())
    }

    /// Spawns a new MCP server
    ///
    /// Creates a new server from the configuration, saves it to the database,
    /// and starts the server process.
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    ///
    /// # Returns
    ///
    /// Returns the server state after initialization.
    ///
    /// # Errors
    ///
    /// Returns `MCPError::ServerAlreadyExists` if a server with the same ID exists.
    pub async fn spawn_server(&self, config: MCPServerConfig) -> MCPResult<MCPServer> {
        // Check if server already exists (by ID)
        {
            let clients = self.clients.read().await;
            if clients.contains_key(&config.id) {
                return Err(MCPError::ServerAlreadyExists {
                    server: config.id.clone(),
                });
            }
        }

        // Save to database first
        self.save_server_config(&config).await?;

        // Spawn the server
        let server = self.spawn_server_internal(config).await?;

        Ok(server)
    }

    /// Internal method to spawn a server without saving to database
    async fn spawn_server_internal(&self, config: MCPServerConfig) -> MCPResult<MCPServer> {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            "Spawning MCP server"
        );

        let id = config.id.clone();
        let client = MCPClient::connect(config.clone()).await?;

        let server = MCPServer {
            config: config.clone(),
            status: client.status(),
            tools: client.tools().to_vec(),
            resources: client.resources().to_vec(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Add to registry (keyed by ID for consistent lookup)
        {
            let mut clients = self.clients.write().await;
            clients.insert(id.clone(), client);
        }

        info!(
            server_id = %id,
            server_name = %config.name,
            tools_count = server.tools.len(),
            resources_count = server.resources.len(),
            "MCP server spawned and registered"
        );

        Ok(server)
    }

    /// Stops an MCP server
    ///
    /// Terminates the server process and removes it from the registry.
    /// The configuration remains in the database.
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to stop
    ///
    /// # Errors
    ///
    /// Returns `MCPError::ServerNotFound` if the server doesn't exist.
    pub async fn stop_server(&self, id: &str) -> MCPResult<()> {
        info!(server_id = %id, "Stopping MCP server");

        let mut client = {
            let mut clients = self.clients.write().await;
            clients
                .remove(id)
                .ok_or_else(|| MCPError::ServerNotFound {
                    server: id.to_string(),
                })?
        };

        client.disconnect().await?;

        info!(server_id = %id, "MCP server stopped");

        Ok(())
    }

    /// Gets a server by ID
    ///
    /// Checks both running servers (in HashMap) and configured servers (in database).
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to look up
    ///
    /// # Returns
    ///
    /// Returns the server state if found (running or stopped).
    pub async fn get_server(&self, id: &str) -> Option<MCPServer> {
        // First check running servers
        {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(id) {
                return Some(MCPServer {
                    config: client.config().clone(),
                    status: client.status(),
                    tools: client.tools().to_vec(),
                    resources: client.resources().to_vec(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                });
            }
        }

        // Then check database for stopped servers
        if let Ok(configs) = self.get_saved_configs().await {
            if let Some(config) = configs.into_iter().find(|c| c.id == id) {
                return Some(MCPServer {
                    config,
                    status: MCPServerStatus::Stopped,
                    tools: Vec::new(),
                    resources: Vec::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                });
            }
        }

        None
    }

    /// Lists all servers (both running and configured)
    ///
    /// Returns servers from both the active registry and database configurations.
    pub async fn list_servers(&self) -> MCPResult<Vec<MCPServer>> {
        let mut servers = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // First, add running servers (keyed by ID)
        {
            let clients = self.clients.read().await;
            for (id, client) in clients.iter() {
                seen_ids.insert(id.clone());
                servers.push(MCPServer {
                    config: client.config().clone(),
                    status: client.status(),
                    tools: client.tools().to_vec(),
                    resources: client.resources().to_vec(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                });
            }
        }

        // Then add configured but not running servers from database
        let configs = self.get_saved_configs().await?;
        for config in configs {
            if !seen_ids.contains(&config.id) {
                servers.push(MCPServer {
                    config,
                    status: MCPServerStatus::Stopped,
                    tools: Vec::new(),
                    resources: Vec::new(),
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                });
            }
        }

        Ok(servers)
    }

    /// Calls a tool on a specific server
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the server to call (display name, not ID)
    /// * `tool_name` - Name of the tool to invoke
    /// * `arguments` - Tool arguments as JSON value
    ///
    /// # Returns
    ///
    /// Returns the tool call result.
    ///
    /// # Errors
    ///
    /// Returns an error if the server or tool doesn't exist, or if the call fails.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<MCPToolCallResult> {
        debug!(
            server_name = %server_name,
            tool_name = %tool_name,
            "Calling MCP tool"
        );

        let start = Instant::now();

        let result = {
            let mut clients = self.clients.write().await;
            // Find client by server name (config.name), not by ID
            let client = clients
                .values_mut()
                .find(|c| c.config().name == server_name)
                .ok_or(MCPError::ServerNotFound {
                    server: server_name.to_string(),
                })?;

            client.call_tool(tool_name, arguments.clone()).await
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Log the call regardless of success/failure
        let (success, result_value, _error_msg) = match &result {
            Ok(r) => (r.success, r.content.clone(), r.error.clone()),
            Err(e) => (false, serde_json::Value::Null, Some(e.to_string())),
        };

        // Log to database (fire and forget)
        let log_entry = MCPCallLog {
            id: Uuid::new_v4().to_string(),
            workflow_id: None, // Set by caller if in workflow context
            server_name: server_name.to_string(),
            tool_name: tool_name.to_string(),
            params: arguments,
            result: result_value.clone(),
            success,
            duration_ms,
            timestamp: Utc::now(),
        };

        if let Err(e) = self.log_call(log_entry).await {
            warn!(error = %e, "Failed to log MCP call to database");
        }

        result
    }

    /// Calls a tool using a request object
    ///
    /// Convenience method that extracts parameters from `MCPToolCallRequest`.
    pub async fn call_tool_request(
        &self,
        request: MCPToolCallRequest,
    ) -> MCPResult<MCPToolCallResult> {
        self.call_tool(&request.server_name, &request.tool_name, request.arguments)
            .await
    }

    /// Lists tools available on a specific server
    ///
    /// # Arguments
    ///
    /// * `server_name` - Display name of the server (not ID)
    ///
    /// # Returns
    ///
    /// Returns the list of tools, or empty list if server not found.
    pub async fn list_server_tools(&self, server_name: &str) -> Vec<MCPTool> {
        let clients = self.clients.read().await;
        // Find client by server name (config.name), not by ID
        clients
            .values()
            .find(|c| c.config().name == server_name)
            .map(|c| c.tools().to_vec())
            .unwrap_or_default()
    }

    /// Lists all tools across all connected servers
    ///
    /// # Returns
    ///
    /// Returns a map of server name (display name) to list of tools.
    pub async fn list_all_tools(&self) -> HashMap<String, Vec<MCPTool>> {
        let clients = self.clients.read().await;
        clients
            .values()
            .map(|client| (client.config().name.clone(), client.tools().to_vec()))
            .collect()
    }

    /// Tests a server configuration without saving it
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration to test
    ///
    /// # Returns
    ///
    /// Returns the test result with connection status and discovered capabilities.
    pub async fn test_server(&self, config: MCPServerConfig) -> MCPTestResult {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            "Testing MCP server connection"
        );

        MCPClient::test_connection(config).await
    }

    /// Stops all running servers
    ///
    /// Called during application shutdown to cleanly terminate all processes.
    pub async fn shutdown(&self) -> MCPResult<()> {
        info!("Shutting down MCP manager");

        let mut clients = self.clients.write().await;

        for (name, mut client) in clients.drain() {
            if let Err(e) = client.disconnect().await {
                warn!(
                    server_name = %name,
                    error = %e,
                    "Error stopping MCP server during shutdown"
                );
            }
        }

        info!("MCP manager shutdown complete");

        Ok(())
    }

    // =========================================================================
    // Database Operations
    // =========================================================================

    /// Saves a server configuration to the database
    async fn save_server_config(&self, config: &MCPServerConfig) -> MCPResult<()> {
        let create_data = MCPServerCreate::from_config(config);

        self.db
            .create("mcp_server", &config.id, create_data)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "save server config".to_string(),
                message: e.to_string(),
            })?;

        debug!(
            server_id = %config.id,
            "Server configuration saved to database"
        );

        Ok(())
    }

    /// Updates a server configuration in the database
    pub async fn update_server_config(&self, config: &MCPServerConfig) -> MCPResult<()> {
        let create_data = MCPServerCreate::from_config(config);
        let json_data = serde_json::to_value(&create_data)?;

        // Use CONTENT to replace the entire record, then add updated_at
        // SurrealDB doesn't support MERGE + SET together
        let query = format!(
            "UPDATE mcp_server:`{}` CONTENT $data",
            config.id
        );

        let _: Vec<serde_json::Value> = self
            .db
            .query_with_params(&query, vec![("data".to_string(), json_data)])
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "update server config".to_string(),
                message: e.to_string(),
            })?;

        debug!(
            server_id = %config.id,
            "Server configuration updated in database"
        );

        Ok(())
    }

    /// Deletes a server configuration from the database
    pub async fn delete_server_config(&self, id: &str) -> MCPResult<()> {
        // First stop the server if running (by ID)
        let _ = self.stop_server(id).await;

        // Use raw query instead of SDK delete method (which has issues with record IDs)
        let query = format!("DELETE mcp_server:`{}`", id);

        let _: Vec<serde_json::Value> = self
            .db
            .query_json(&query)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "delete server config".to_string(),
                message: e.to_string(),
            })?;

        debug!(server_id = %id, "Server configuration deleted from database");

        Ok(())
    }

    /// Gets all saved server configurations from the database
    async fn get_saved_configs(&self) -> MCPResult<Vec<MCPServerConfig>> {
        let query = "SELECT meta::id(id) AS id, name, enabled, command, args, env, description FROM mcp_server";

        let result: Vec<serde_json::Value> =
            self.db
                .query_json(query)
                .await
                .map_err(|e| MCPError::DatabaseError {
                    context: "get saved configs".to_string(),
                    message: e.to_string(),
                })?;

        let configs: Vec<MCPServerConfig> = result
            .into_iter()
            .filter_map(|v| {
                // Convert command string back to enum
                let command_str = v.get("command")?.as_str()?;
                let command = match command_str {
                    "docker" => crate::models::mcp::MCPDeploymentMethod::Docker,
                    "npx" => crate::models::mcp::MCPDeploymentMethod::Npx,
                    "uvx" => crate::models::mcp::MCPDeploymentMethod::Uvx,
                    _ => return None,
                };

                // Extract env as HashMap
                let env: HashMap<String, String> = v
                    .get("env")
                    .and_then(|e| serde_json::from_value(e.clone()).ok())
                    .unwrap_or_default();

                Some(MCPServerConfig {
                    id: v.get("id")?.as_str()?.to_string(),
                    name: v.get("name")?.as_str()?.to_string(),
                    enabled: v.get("enabled")?.as_bool()?,
                    command,
                    args: v
                        .get("args")?
                        .as_array()?
                        .iter()
                        .filter_map(|a| a.as_str().map(String::from))
                        .collect(),
                    env,
                    description: v
                        .get("description")
                        .and_then(|d| d.as_str().map(String::from)),
                })
            })
            .collect();

        Ok(configs)
    }

    /// Gets a single server configuration from the database
    pub async fn get_server_config(&self, id: &str) -> MCPResult<Option<MCPServerConfig>> {
        let configs = self.get_saved_configs().await?;
        Ok(configs.into_iter().find(|c| c.id == id))
    }

    /// Logs a tool call to the database
    async fn log_call(&self, log: MCPCallLog) -> MCPResult<()> {
        let id = log.id.clone();
        self.db
            .create("mcp_call_log", &id, log)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "log call".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }

    /// Gets the number of connected servers
    pub async fn connected_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Restarts a server
    ///
    /// Stops the server if running, then starts it again.
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to restart
    pub async fn restart_server(&self, id: &str) -> MCPResult<MCPServer> {
        info!(server_id = %id, "Restarting MCP server");

        // Get config by ID
        let config = {
            let clients = self.clients.read().await;
            clients.get(id).map(|c| c.config().clone())
        };

        let config = if let Some(c) = config {
            c
        } else {
            // Try database - find by ID
            let configs = self.get_saved_configs().await?;
            configs
                .into_iter()
                .find(|c| c.id == id)
                .ok_or_else(|| MCPError::ServerNotFound {
                    server: id.to_string(),
                })?
        };

        // Stop if running (by ID)
        let _ = self.stop_server(id).await;

        // Spawn again
        self.spawn_server_internal(config).await
    }
}

impl Drop for MCPManager {
    fn drop(&mut self) {
        // Note: Async cleanup should be done via shutdown() before dropping
        // This is just a safety net for the underlying handles
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mcp::MCPDeploymentMethod;

    fn create_test_config(id: &str, name: &str) -> MCPServerConfig {
        MCPServerConfig {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec![
                "run".to_string(),
                "-i".to_string(),
                "test:latest".to_string(),
            ],
            env: HashMap::new(),
            description: Some("Test server".to_string()),
        }
    }

    #[test]
    fn test_server_config_creation() {
        let config = create_test_config("test_id", "test_server");
        assert_eq!(config.id, "test_id");
        assert_eq!(config.name, "test_server");
        assert!(config.enabled);
    }

    #[test]
    fn test_mcp_server_create_from_config() {
        let config = create_test_config("test_id", "test_server");
        let create = MCPServerCreate::from_config(&config);

        assert_eq!(create.name, "test_server");
        assert_eq!(create.command, "docker");
        assert!(create.enabled);
    }

    #[test]
    fn test_call_log_serialization() {
        let log = MCPCallLog {
            id: "log_123".to_string(),
            workflow_id: Some("wf_456".to_string()),
            server_name: "serena".to_string(),
            tool_name: "find_symbol".to_string(),
            params: serde_json::json!({"name": "MyClass"}),
            result: serde_json::json!({"found": true}),
            success: true,
            duration_ms: 150,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("\"server_name\":\"serena\""));
        assert!(json.contains("\"tool_name\":\"find_symbol\""));
        assert!(json.contains("\"success\":true"));
    }
}
