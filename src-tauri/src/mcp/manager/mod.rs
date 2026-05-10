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

mod db;
mod lifecycle;
mod tools;

#[cfg(test)]
mod tests;

use crate::db::DBClient;
use crate::mcp::circuit_breaker::CircuitBreaker;
use crate::mcp::client::MCPClient;
use crate::mcp::MCPResult;
use crate::models::mcp::{MCPServer, MCPServerStatus, MCPTool};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Tool cache TTL (1 hour)
pub(crate) const TOOL_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Maximum retry attempts for transient MCP errors
pub(crate) const MCP_MAX_RETRY_ATTEMPTS: u32 = 2;

/// Initial retry delay in milliseconds (doubles with each attempt)
pub(crate) const MCP_INITIAL_RETRY_DELAY_MS: u64 = 500;

/// MCP Manager
///
/// Manages the lifecycle of multiple MCP servers and provides
/// a unified interface for tool invocation.
///
/// # Thread Safety
///
/// The manager uses `RwLock` internally and is safe to share
/// across threads via `Arc<MCPManager>`.
pub struct MCPManager {
    /// Connected clients indexed by server name
    pub(crate) clients: RwLock<HashMap<String, MCPClient>>,
    /// Database client for persistence
    pub(crate) db: Arc<DBClient>,
    /// Tool cache with TTL (server_name -> (tools, cached_at))
    pub(crate) tool_cache: RwLock<HashMap<String, (Vec<MCPTool>, Instant)>>,
    /// Circuit breakers per server (server_name -> CircuitBreaker)
    pub(crate) circuit_breakers: RwLock<HashMap<String, CircuitBreaker>>,
    /// ID to Name lookup table for O(1) access (server_id -> server_name)
    pub(crate) id_to_name: RwLock<HashMap<String, String>>,
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
            tool_cache: RwLock::new(HashMap::new()),
            circuit_breakers: RwLock::new(HashMap::new()),
            id_to_name: RwLock::new(HashMap::new()),
        })
    }

    /// Loads server configurations from the database
    ///
    /// Queries all enabled servers from the database and connects them
    /// **in parallel** for faster startup. Registration in the internal
    /// registries is done sequentially after all connections complete.
    /// Servers that fail to connect are logged but don't prevent other
    /// servers from starting.
    pub async fn load_from_db(&self) -> MCPResult<()> {
        use crate::mcp::client::MCPClient;

        info!("Loading MCP servers from database");

        let configs = self.get_saved_configs().await?;
        let enabled_configs: Vec<_> = configs.into_iter().filter(|c| c.enabled).collect();

        info!(
            total_configs = enabled_configs.len(),
            "Found enabled MCP server configurations"
        );

        // Connect all MCP clients in parallel (the expensive part: process spawn + init)
        let connect_futures: Vec<_> = enabled_configs
            .into_iter()
            .map(|config| async move {
                let result = MCPClient::connect(config.clone()).await;
                (config, result)
            })
            .collect();

        let results = futures_util::future::join_all(connect_futures).await;

        // Register successful connections sequentially (cheap: HashMap inserts)
        for (config, result) in results {
            match result {
                Ok(client) => {
                    self.register_client(config, client).await;
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

    /// Lists all servers (both running and configured)
    ///
    /// Returns servers from both the active registry and database configurations.
    pub async fn list_servers(&self) -> MCPResult<Vec<MCPServer>> {
        let mut servers = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        // First, add running servers (HashMap is keyed by NAME, but we track by config.id)
        {
            let clients = self.clients.read().await;
            for (_name, client) in clients.iter() {
                // Track by ID for deduplication with database configs
                seen_ids.insert(client.config().id.clone());
                servers.push(MCPServer {
                    config: client.config().clone(),
                    status: client.status(),
                    tools: client.tools().to_vec(),
                    resources: client.resources().to_vec(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
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
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }

        Ok(servers)
    }

    /// Gets a server by ID
    ///
    /// Checks both running servers (via O(1) lookup) and configured servers (in database).
    pub async fn get_server(&self, id: &str) -> Option<MCPServer> {
        // O(1) lookup via id_to_name table
        let name = {
            let id_lookup = self.id_to_name.read().await;
            id_lookup.get(id).cloned()
        };

        // Check running servers first
        if let Some(name) = name {
            let clients = self.clients.read().await;
            if let Some(client) = clients.get(&name) {
                return Some(MCPServer {
                    config: client.config().clone(),
                    status: client.status(),
                    tools: client.tools().to_vec(),
                    resources: client.resources().to_vec(),
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
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
                    created_at: chrono::Utc::now(),
                    updated_at: chrono::Utc::now(),
                });
            }
        }

        None
    }

    /// Gets server names (for validation).
    pub async fn server_names(&self) -> Vec<String> {
        let clients = self.clients.read().await;
        clients.keys().cloned().collect()
    }

    /// Validates server names exist.
    pub async fn validate_server_names(&self, names: &[String]) -> Result<(), Vec<String>> {
        let clients = self.clients.read().await;
        let invalid: Vec<String> = names
            .iter()
            .filter(|name| !clients.contains_key(*name))
            .cloned()
            .collect();

        if invalid.is_empty() {
            Ok(())
        } else {
            Err(invalid)
        }
    }

    /// Gets the number of connected servers
    pub async fn connected_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Stops all running servers
    ///
    /// Called during application shutdown to cleanly terminate all processes.
    /// Wired into Tauri's `RunEvent::ExitRequested` in `main.rs`.
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
}
