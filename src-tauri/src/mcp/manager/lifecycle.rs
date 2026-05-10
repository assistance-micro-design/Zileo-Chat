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

//! MCP Manager lifecycle operations
//!
//! Server spawning, stopping, restarting, and health checks.

use super::MCPManager;
use crate::mcp::circuit_breaker::CircuitBreaker;
use crate::mcp::client::MCPClient;
use crate::mcp::{MCPError, MCPResult};
use crate::models::mcp::{MCPServer, MCPServerConfig, MCPTestResult};
use chrono::Utc;
use tracing::info;

impl MCPManager {
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
    /// Returns `MCPError::ServerAlreadyExists` if a server with the same name exists.
    pub async fn spawn_server(&self, config: MCPServerConfig) -> MCPResult<MCPServer> {
        // Check if server already exists (by NAME)
        {
            let clients = self.clients.read().await;
            if clients.contains_key(&config.name) {
                return Err(MCPError::ServerAlreadyExists {
                    server: config.name.clone(),
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
    pub(crate) async fn spawn_server_internal(
        &self,
        config: MCPServerConfig,
    ) -> MCPResult<MCPServer> {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            env_count = config.env.len(),
            env_keys = ?config.env.keys().collect::<Vec<_>>(),
            "Spawning MCP server"
        );

        // NOTE: Caller (spawn_server or load_from_db) must verify name uniqueness before calling

        let client = MCPClient::connect(config.clone()).await?;

        Ok(self.register_client(config, client).await)
    }

    /// Registers a connected MCP client in the internal registries.
    ///
    /// Inserts the client into the name-keyed registry, the ID->name lookup
    /// table, and creates a circuit breaker. Used by both `spawn_server_internal`
    /// (single server) and `load_from_db` (parallel startup).
    pub(crate) async fn register_client(
        &self,
        config: MCPServerConfig,
        client: MCPClient,
    ) -> MCPServer {
        let name = config.name.clone();
        let id = config.id.clone();

        let server = MCPServer {
            config,
            status: client.status(),
            tools: client.tools().to_vec(),
            resources: client.resources().to_vec(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Add to registry (keyed by NAME for functional identification)
        {
            let mut clients = self.clients.write().await;
            clients.insert(name.clone(), client);
        }

        // Add ID -> Name lookup for O(1) access
        {
            let mut id_lookup = self.id_to_name.write().await;
            id_lookup.insert(id.clone(), name.clone());
        }

        // Create circuit breaker for this server
        {
            let mut breakers = self.circuit_breakers.write().await;
            breakers.insert(name.clone(), CircuitBreaker::with_defaults(name.clone()));
        }

        info!(
            server_id = %id,
            server_name = %name,
            tools_count = server.tools.len(),
            resources_count = server.resources.len(),
            "MCP server spawned and registered by name"
        );

        server
    }

    /// Stops an MCP server
    ///
    /// Terminates the server process and removes it from the registry.
    /// The configuration remains in the database.
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to stop (uses O(1) lookup via id_to_name table)
    ///
    /// # Errors
    ///
    /// Returns `MCPError::ServerNotFound` if the server doesn't exist.
    pub async fn stop_server(&self, id: &str) -> MCPResult<()> {
        info!(server_id = %id, "Stopping MCP server");

        // O(1) lookup via id_to_name table
        let name = {
            let id_lookup = self.id_to_name.read().await;
            id_lookup.get(id).cloned()
        }
        .ok_or_else(|| MCPError::ServerNotFound {
            server: id.to_string(),
        })?;

        {
            let mut clients = self.clients.write().await;
            let client = clients
                .get_mut(&name)
                .ok_or_else(|| MCPError::ServerNotFound {
                    server: id.to_string(),
                })?;

            client.disconnect().await?;
            clients.remove(&name);
        }

        // Cleanup lookup table and circuit breaker only after disconnect succeeded.
        {
            let mut id_lookup = self.id_to_name.write().await;
            id_lookup.remove(id);
        }
        {
            let mut breakers = self.circuit_breakers.write().await;
            breakers.remove(&name);
        }

        info!(server_id = %id, server_name = %name, "MCP server stopped");

        Ok(())
    }

    /// Restarts a server
    ///
    /// Stops the server if running, then starts it again.
    /// Also resets the circuit breaker for the server.
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to restart (uses O(1) lookup via id_to_name table)
    pub async fn restart_server(&self, id: &str) -> MCPResult<MCPServer> {
        info!(server_id = %id, "Restarting MCP server");

        // O(1) lookup via id_to_name table
        let name = {
            let id_lookup = self.id_to_name.read().await;
            id_lookup.get(id).cloned()
        };

        // Get config using the name if found, or from database
        let config = if let Some(ref name) = name {
            let clients = self.clients.read().await;
            clients.get(name).map(|c| c.config().clone())
        } else {
            None
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

        // Stop if running (by ID). A missing runtime client means the server is
        // already stopped; every other stop failure must block restart to avoid
        // duplicate process/state.
        match self.stop_server(id).await {
            Ok(()) | Err(MCPError::ServerNotFound { .. }) => {}
            Err(err) => return Err(err),
        }

        // Spawn again (this will create fresh circuit breaker and id_to_name entry)
        self.spawn_server_internal(config).await
    }

    /// Tests a server configuration without saving it
    pub async fn test_server(&self, config: MCPServerConfig) -> MCPTestResult {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            "Testing MCP server connection"
        );

        MCPClient::test_connection(config).await
    }
}
