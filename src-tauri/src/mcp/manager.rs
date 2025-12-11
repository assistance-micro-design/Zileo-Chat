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

use crate::db::DBClient;
use crate::mcp::circuit_breaker::CircuitBreaker;
use crate::mcp::client::MCPClient;
use crate::mcp::{MCPError, MCPResult};
use crate::models::mcp::{
    MCPCallLogCreate, MCPServer, MCPServerConfig, MCPServerCreate, MCPServerStatus, MCPTestResult,
    MCPTool, MCPToolCallRequest, MCPToolCallResult,
};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Tool cache TTL (1 hour)
const TOOL_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Default health check interval (5 minutes)
const DEFAULT_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(300);

/// Maximum retry attempts for transient MCP errors
const MCP_MAX_RETRY_ATTEMPTS: u32 = 2;

/// Initial retry delay in milliseconds (doubles with each attempt)
const MCP_INITIAL_RETRY_DELAY_MS: u64 = 500;

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
    /// Tool cache with TTL (server_name -> (tools, cached_at))
    tool_cache: RwLock<HashMap<String, (Vec<MCPTool>, Instant)>>,
    /// Circuit breakers per server (server_name -> CircuitBreaker)
    circuit_breakers: RwLock<HashMap<String, CircuitBreaker>>,
    /// ID to Name lookup table for O(1) access (server_id -> server_name)
    id_to_name: RwLock<HashMap<String, String>>,
    /// Shutdown signal sender for health check task
    health_check_shutdown: broadcast::Sender<()>,
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

        // Create shutdown channel for health check task (capacity 1 is enough)
        let (shutdown_tx, _) = broadcast::channel(1);

        Ok(Self {
            clients: RwLock::new(HashMap::new()),
            db,
            tool_cache: RwLock::new(HashMap::new()),
            circuit_breakers: RwLock::new(HashMap::new()),
            id_to_name: RwLock::new(HashMap::new()),
            health_check_shutdown: shutdown_tx,
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
    async fn spawn_server_internal(&self, config: MCPServerConfig) -> MCPResult<MCPServer> {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            env_count = config.env.len(),
            env_keys = ?config.env.keys().collect::<Vec<_>>(),
            "Spawning MCP server"
        );

        // NOTE: Caller (spawn_server or load_from_db) must verify name uniqueness before calling

        let name = config.name.clone();
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

        // Add to registry (keyed by NAME for functional identification)
        {
            let mut clients = self.clients.write().await;
            clients.insert(name.clone(), client);
        }

        // Add ID -> Name lookup for O(1) access (OPT-7)
        {
            let mut id_lookup = self.id_to_name.write().await;
            id_lookup.insert(id.clone(), name.clone());
        }

        // Create circuit breaker for this server (OPT-6)
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

        Ok(server)
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

        // O(1) lookup via id_to_name table (OPT-7)
        let name = {
            let id_lookup = self.id_to_name.read().await;
            id_lookup.get(id).cloned()
        }
        .ok_or_else(|| MCPError::ServerNotFound {
            server: id.to_string(),
        })?;

        let mut client = {
            let mut clients = self.clients.write().await;
            clients
                .remove(&name)
                .ok_or_else(|| MCPError::ServerNotFound {
                    server: id.to_string(),
                })?
        };

        // Cleanup lookup table and circuit breaker
        {
            let mut id_lookup = self.id_to_name.write().await;
            id_lookup.remove(id);
        }
        {
            let mut breakers = self.circuit_breakers.write().await;
            breakers.remove(&name);
        }

        client.disconnect().await?;

        info!(server_id = %id, server_name = %name, "MCP server stopped");

        Ok(())
    }

    /// Gets a server by ID
    ///
    /// Checks both running servers (via O(1) lookup) and configured servers (in database).
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to look up (uses O(1) lookup via id_to_name table)
    ///
    /// # Returns
    ///
    /// Returns the server state if found (running or stopped).
    pub async fn get_server(&self, id: &str) -> Option<MCPServer> {
        // O(1) lookup via id_to_name table (OPT-7)
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
    /// Uses circuit breaker pattern to prevent cascade failures.
    /// If the circuit is open (server unhealthy), the call will fail fast.
    ///
    /// # Arguments
    ///
    /// * `server_name` - The NAME of the MCP server (not ID)
    /// * `tool_name` - Name of the tool to invoke
    /// * `arguments` - Tool arguments as JSON value
    ///
    /// # Returns
    ///
    /// Returns the tool call result.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Server or tool doesn't exist
    /// - Circuit breaker is open (server unhealthy)
    /// - The call itself fails after all retry attempts
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

        // Check circuit breaker before making the call (OPT-6)
        {
            let mut breakers = self.circuit_breakers.write().await;
            if let Some(breaker) = breakers.get_mut(server_name) {
                if !breaker.allow_request() {
                    let remaining = breaker
                        .remaining_cooldown()
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    return Err(MCPError::CircuitBreakerOpen {
                        server: server_name.to_string(),
                        cooldown_remaining_secs: remaining,
                    });
                }
            }
        }

        let start = Instant::now();
        let mut last_error: Option<MCPError> = None;

        // Retry loop with exponential backoff
        for attempt in 0..=MCP_MAX_RETRY_ATTEMPTS {
            let result = {
                let mut clients = self.clients.write().await;
                // Clients are keyed by server NAME
                let client = clients
                    .get_mut(server_name)
                    .ok_or(MCPError::ServerNotFound {
                        server: server_name.to_string(),
                    })?;

                client.call_tool(tool_name, arguments.clone()).await
            };

            match result {
                Ok(call_result) => {
                    let duration_ms = start.elapsed().as_millis() as u64;

                    // Update circuit breaker on success
                    {
                        let mut breakers = self.circuit_breakers.write().await;
                        if let Some(breaker) = breakers.get_mut(server_name) {
                            breaker.record_success();
                        }
                    }

                    // Log successful call
                    let log_entry = MCPCallLogCreate {
                        id: Uuid::new_v4().to_string(),
                        workflow_id: None,
                        server_name: server_name.to_string(),
                        tool_name: tool_name.to_string(),
                        params: arguments.clone(),
                        result: call_result.content.clone(),
                        success: call_result.success,
                        duration_ms,
                    };

                    if let Err(e) = self.log_call(log_entry).await {
                        warn!(error = %e, "Failed to log MCP call to database");
                    }

                    if attempt > 0 {
                        info!(
                            server_name = %server_name,
                            tool_name = %tool_name,
                            attempt = attempt + 1,
                            "MCP tool call succeeded on retry"
                        );
                    }

                    return Ok(call_result);
                }
                Err(e) => {
                    // Check if error is retryable
                    let is_retryable = Self::is_retryable_error(&e);

                    if !is_retryable || attempt >= MCP_MAX_RETRY_ATTEMPTS {
                        // Non-retryable error or exhausted retries
                        let duration_ms = start.elapsed().as_millis() as u64;

                        // Update circuit breaker on failure
                        {
                            let mut breakers = self.circuit_breakers.write().await;
                            if let Some(breaker) = breakers.get_mut(server_name) {
                                breaker.record_failure();
                            }
                        }

                        // Invalidate tool cache on failure
                        self.invalidate_tool_cache(server_name).await;

                        // Log failed call
                        let log_entry = MCPCallLogCreate {
                            id: Uuid::new_v4().to_string(),
                            workflow_id: None,
                            server_name: server_name.to_string(),
                            tool_name: tool_name.to_string(),
                            params: arguments.clone(),
                            result: serde_json::Value::Null,
                            success: false,
                            duration_ms,
                        };

                        if let Err(log_err) = self.log_call(log_entry).await {
                            warn!(error = %log_err, "Failed to log MCP call to database");
                        }

                        if attempt > 0 {
                            return Err(MCPError::RetryExhausted {
                                server: server_name.to_string(),
                                attempts: attempt + 1,
                                last_error: e.to_string(),
                            });
                        }

                        return Err(e);
                    }

                    // Retryable error - wait and retry
                    let delay_ms = MCP_INITIAL_RETRY_DELAY_MS * 2_u64.pow(attempt);
                    warn!(
                        server_name = %server_name,
                        tool_name = %tool_name,
                        attempt = attempt + 1,
                        max_attempts = MCP_MAX_RETRY_ATTEMPTS + 1,
                        delay_ms = delay_ms,
                        error = %e,
                        "Retrying MCP tool call after transient error"
                    );

                    last_error = Some(e);
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
        }

        // Should not reach here, but just in case
        Err(last_error.unwrap_or_else(|| MCPError::IoError {
            context: "unexpected retry state".to_string(),
            message: "No error recorded during retry loop".to_string(),
        }))
    }

    /// Determines if an MCP error is retryable.
    ///
    /// Retryable errors are transient issues that might succeed on retry:
    /// - Timeout errors
    /// - Connection errors (temporary network issues)
    /// - IO errors
    ///
    /// Non-retryable errors should fail immediately:
    /// - Server not found
    /// - Invalid configuration
    /// - Protocol errors (malformed responses)
    /// - Circuit breaker open
    fn is_retryable_error(error: &MCPError) -> bool {
        matches!(
            error,
            MCPError::Timeout { .. } | MCPError::ConnectionFailed { .. } | MCPError::IoError { .. }
        )
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

    /// Lists tools available on a specific server by NAME.
    ///
    /// Uses a cache with 1-hour TTL to avoid redundant calls.
    /// Cache is automatically invalidated on tool call errors.
    ///
    /// # Arguments
    ///
    /// * `server_name` - Server NAME (e.g., "Serena", "Context7")
    ///
    /// # Returns
    ///
    /// Returns the list of tools, or empty list if server not found.
    pub async fn list_server_tools(&self, server_name: &str) -> Vec<MCPTool> {
        // Check cache first
        {
            let cache = self.tool_cache.read().await;
            if let Some((tools, cached_at)) = cache.get(server_name) {
                if cached_at.elapsed() < TOOL_CACHE_TTL {
                    debug!(server = %server_name, "Tool cache hit");
                    return tools.clone();
                }
            }
        }

        // Cache miss or expired - fetch from client
        debug!(server = %server_name, "Tool cache miss, fetching from client");
        let clients = self.clients.read().await;
        let tools = clients
            .get(server_name)
            .map(|c| c.tools().to_vec())
            .unwrap_or_default();

        // Update cache
        if !tools.is_empty() {
            let mut cache = self.tool_cache.write().await;
            cache.insert(server_name.to_string(), (tools.clone(), Instant::now()));
        }

        tools
    }

    /// Invalidates the tool cache for a specific server.
    ///
    /// Call this when a tool call fails to force a refresh on next access.
    pub async fn invalidate_tool_cache(&self, server_name: &str) {
        let mut cache = self.tool_cache.write().await;
        cache.remove(server_name);
        debug!(server = %server_name, "Tool cache invalidated");
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

        debug!(
            server_id = %config.id,
            env_count = config.env.len(),
            env_keys = ?config.env.keys().collect::<Vec<_>>(),
            "Saving MCP server config to database"
        );

        self.db
            .create("mcp_server", &config.id, create_data)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "save server config".to_string(),
                message: e.to_string(),
            })?;

        info!(
            server_id = %config.id,
            env_count = config.env.len(),
            "Server configuration saved to database"
        );

        Ok(())
    }

    /// Updates a server configuration in the database
    pub async fn update_server_config(&self, config: &MCPServerConfig) -> MCPResult<()> {
        // Serialize each field to JSON for the query
        // Using explicit SET like other working update functions (agent, task, etc.)
        let name_json = serde_json::to_string(&config.name)?;
        let command_json = serde_json::to_string(&config.command)?;
        let args_json = serde_json::to_string(&config.args)?;
        // env is stored as a JSON string (to bypass SurrealDB SCHEMAFULL filtering)
        // First serialize HashMap to JSON string, then encode that string for SQL
        let env_str = serde_json::to_string(&config.env)?; // {"KEY":"value"}
        let env_json = serde_json::to_string(&env_str)?; // "{\"KEY\":\"value\"}"
        let description_json = match &config.description {
            Some(desc) => serde_json::to_string(desc)?,
            None => "NONE".to_string(),
        };

        let query = format!(
            "UPDATE mcp_server:`{}` SET \
                name = {}, \
                enabled = {}, \
                command = {}, \
                args = {}, \
                env = {}, \
                description = {}, \
                updated_at = time::now()",
            config.id,
            name_json,
            config.enabled,
            command_json,
            args_json,
            env_json,
            description_json
        );

        self.db
            .execute(&query)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "update server config".to_string(),
                message: e.to_string(),
            })?;

        // Also update in-memory client if it exists
        {
            let mut clients = self.clients.write().await;
            if let Some(client) = clients.get_mut(&config.id) {
                client.update_config(config.clone());
                debug!(
                    server_id = %config.id,
                    "Server configuration updated in memory"
                );
            }
        }

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

        let _: Vec<serde_json::Value> =
            self.db
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
        use crate::mcp::helpers::{parse_deployment_method, parse_env_json};

        let query = "SELECT meta::id(id) AS id, name, enabled, command, args, env, description FROM mcp_server";

        let result: Vec<serde_json::Value> =
            self.db
                .query_json(query)
                .await
                .map_err(|e| MCPError::DatabaseError {
                    context: "get saved configs".to_string(),
                    message: e.to_string(),
                })?;

        info!(
            result_count = result.len(),
            "Retrieved MCP server configs from database"
        );

        let configs: Vec<MCPServerConfig> = result
            .into_iter()
            .filter_map(|v| {
                let server_id = v.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");

                // Use helper for deployment method parsing
                let command = match parse_deployment_method(v.get("command")) {
                    Some(method) => method,
                    None => {
                        warn!(
                            server_id = %server_id,
                            command_value = ?v.get("command"),
                            "Unknown deployment method, skipping server"
                        );
                        return None;
                    }
                };

                // Use helper for env parsing (with logging)
                let env_value = v.get("env");
                let env = parse_env_json(env_value);
                if !env.is_empty() {
                    debug!(
                        server_id = %server_id,
                        env_count = env.len(),
                        "Loaded env variables from database"
                    );
                }

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
    ///
    /// Uses execute_with_params instead of create() to avoid SurrealDB SDK 2.x
    /// deserialization issues with union types (array | object) in the result field.
    async fn log_call(&self, log: MCPCallLogCreate) -> MCPResult<()> {
        let json_data = serde_json::to_value(&log).map_err(|e| MCPError::DatabaseError {
            context: "log call serialization".to_string(),
            message: e.to_string(),
        })?;

        let query = format!("CREATE mcp_call_log:`{}` CONTENT $data", log.id);
        self.db
            .execute_with_params(&query, vec![("data".to_string(), json_data)])
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
    /// Also resets the circuit breaker for the server.
    ///
    /// # Arguments
    ///
    /// * `id` - Server ID to restart (uses O(1) lookup via id_to_name table)
    pub async fn restart_server(&self, id: &str) -> MCPResult<MCPServer> {
        info!(server_id = %id, "Restarting MCP server");

        // O(1) lookup via id_to_name table (OPT-7)
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

        // Stop if running (by ID)
        let _ = self.stop_server(id).await;

        // Spawn again (this will create fresh circuit breaker and id_to_name entry)
        self.spawn_server_internal(config).await
    }

    /// Gets the circuit breaker state for a server
    ///
    /// Returns None if the server doesn't have a circuit breaker (not running).
    pub async fn get_circuit_breaker_state(
        &self,
        server_name: &str,
    ) -> Option<crate::mcp::circuit_breaker::CircuitState> {
        let breakers = self.circuit_breakers.read().await;
        breakers.get(server_name).map(|b| b.state())
    }

    /// Resets the circuit breaker for a server
    ///
    /// Use with caution - typically only for manual intervention.
    pub async fn reset_circuit_breaker(&self, server_name: &str) -> bool {
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(server_name) {
            breaker.reset();
            true
        } else {
            false
        }
    }

    /// Starts periodic health checks for all connected servers (OPT-8)
    ///
    /// Spawns a background task that periodically checks server health
    /// using `list_tools()` as a health probe. Unhealthy servers will have
    /// their circuit breakers updated accordingly.
    ///
    /// # Arguments
    ///
    /// * `manager` - Arc reference to self (needed for background task)
    /// * `interval` - How often to check health (default: 5 minutes)
    ///
    /// # Returns
    ///
    /// Returns a `JoinHandle` for the background task.
    pub fn start_health_checks(
        manager: Arc<Self>,
        interval: Option<Duration>,
    ) -> tokio::task::JoinHandle<()> {
        let interval = interval.unwrap_or(DEFAULT_HEALTH_CHECK_INTERVAL);
        let mut shutdown_rx = manager.health_check_shutdown.subscribe();

        info!(
            interval_secs = interval.as_secs(),
            "Starting MCP health check task"
        );

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            // Skip the first immediate tick
            ticker.tick().await;

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        manager.check_all_servers_health().await;
                    }
                    _ = shutdown_rx.recv() => {
                        info!("Health check task received shutdown signal");
                        break;
                    }
                }
            }

            info!("Health check task stopped");
        })
    }

    /// Checks health of all connected servers
    ///
    /// Uses `list_tools()` as a health probe. Updates circuit breakers
    /// based on results.
    async fn check_all_servers_health(&self) {
        let server_names: Vec<String> = {
            let clients = self.clients.read().await;
            clients.keys().cloned().collect()
        };

        if server_names.is_empty() {
            debug!("No servers to health check");
            return;
        }

        debug!(
            server_count = server_names.len(),
            "Running health checks for MCP servers"
        );

        for name in server_names {
            self.check_server_health(&name).await;
        }
    }

    /// Checks health of a single server
    ///
    /// Uses `refresh_tools()` as a health probe (makes actual network call)
    /// and updates circuit breaker.
    async fn check_server_health(&self, server_name: &str) {
        let result = {
            let mut clients = self.clients.write().await;
            if let Some(client) = clients.get_mut(server_name) {
                // Use refresh_tools as health probe - it makes a real network call
                match client.refresh_tools().await {
                    Ok(tools) => {
                        debug!(
                            server = %server_name,
                            tool_count = tools.len(),
                            "Health check passed"
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!(
                            server = %server_name,
                            error = %e,
                            "Health check failed"
                        );
                        Err(e)
                    }
                }
            } else {
                // Server was removed during iteration
                return;
            }
        };

        // Update circuit breaker based on result
        let mut breakers = self.circuit_breakers.write().await;
        if let Some(breaker) = breakers.get_mut(server_name) {
            match result {
                Ok(()) => breaker.record_success(),
                Err(_) => breaker.record_failure(),
            }
        }
    }

    /// Stops the health check background task
    ///
    /// Sends a shutdown signal to the health check task.
    pub fn stop_health_checks(&self) {
        info!("Stopping MCP health check task");
        // Ignore send error if no receivers (task already stopped)
        let _ = self.health_check_shutdown.send(());
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
        // Test MCPCallLogCreate serialization (timestamp omitted - generated by SurrealDB)
        let log = MCPCallLogCreate {
            id: "log_123".to_string(),
            workflow_id: Some("wf_456".to_string()),
            server_name: "serena".to_string(),
            tool_name: "find_symbol".to_string(),
            params: serde_json::json!({"name": "MyClass"}),
            result: serde_json::json!({"found": true}),
            success: true,
            duration_ms: 150,
        };

        let json = serde_json::to_string(&log).unwrap();
        assert!(json.contains("\"server_name\":\"serena\""));
        assert!(json.contains("\"tool_name\":\"find_symbol\""));
        assert!(json.contains("\"success\":true"));
        // Verify timestamp is NOT in the serialized output
        assert!(!json.contains("\"timestamp\":"));
    }
}
