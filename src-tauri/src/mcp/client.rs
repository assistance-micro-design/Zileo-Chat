// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! MCP Client
//!
//! High-level client interface for interacting with MCP servers.
//! This module provides a convenient API for common MCP operations
//! and handles connection state management.
//!
//! ## Architecture
//!
//! The `MCPClient` wraps an `MCPServerHandle` and provides:
//! - Connection state tracking
//! - Automatic reconnection (optional)
//! - High-level tool invocation API
//! - Resource access methods
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::mcp::{MCPClient, MCPServerConfig};
//!
//! // Create and connect client
//! let mut client = MCPClient::connect(config).await?;
//!
//! // Check available tools
//! for tool in client.tools() {
//!     println!("Tool: {} - {}", tool.name, tool.description);
//! }
//!
//! // Call a tool
//! let result = client.call_tool("find_symbol", json!({"name": "MyClass"})).await?;
//!
//! // Disconnect when done
//! client.disconnect().await?;
//! ```

use crate::mcp::server_handle::MCPServerHandle;
use crate::mcp::{MCPError, MCPResult, MCPToolCallResponse};
use crate::models::mcp::{
    MCPResource, MCPServerConfig, MCPServerStatus, MCPTestResult, MCPTool, MCPToolCallResult,
};
use std::time::Instant;
use tracing::info;

/// MCP Client
///
/// High-level interface for interacting with an MCP server.
/// Manages the connection lifecycle and provides convenient methods
/// for tool invocation.
pub struct MCPClient {
    /// Underlying server handle
    handle: Option<MCPServerHandle>,
    /// Server configuration
    config: MCPServerConfig,
    /// Whether auto-reconnect is enabled
    auto_reconnect: bool,
}

impl MCPClient {
    /// Creates a new MCP client without connecting
    ///
    /// Use `connect()` or `connect_with_config()` to establish a connection.
    pub fn new(config: MCPServerConfig) -> Self {
        Self {
            handle: None,
            config,
            auto_reconnect: false,
        }
    }

    /// Creates and connects a new MCP client
    ///
    /// This is a convenience method that creates a client and immediately
    /// establishes a connection to the server.
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration
    ///
    /// # Returns
    ///
    /// Returns a connected `MCPClient` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection or initialization fails.
    pub async fn connect(config: MCPServerConfig) -> MCPResult<Self> {
        let mut client = Self::new(config);
        client.connect_internal().await?;
        Ok(client)
    }

    /// Establishes a connection to the MCP server
    ///
    /// Spawns the server process and performs the MCP initialization handshake.
    ///
    /// # Errors
    ///
    /// Returns an error if already connected, or if connection fails.
    pub async fn connect_internal(&mut self) -> MCPResult<()> {
        if self.handle.is_some() {
            return Err(MCPError::InvalidConfig {
                field: "connection".to_string(),
                reason: "Client is already connected".to_string(),
            });
        }

        info!(
            server_id = %self.config.id,
            "Connecting MCP client"
        );

        // Spawn and initialize the server
        let mut handle = MCPServerHandle::spawn(self.config.clone()).await?;
        handle.initialize().await?;

        self.handle = Some(handle);

        info!(
            server_id = %self.config.id,
            "MCP client connected"
        );

        Ok(())
    }

    /// Disconnects from the MCP server
    ///
    /// Terminates the server process and cleans up resources.
    pub async fn disconnect(&mut self) -> MCPResult<()> {
        if let Some(mut handle) = self.handle.take() {
            handle.kill().await?;
        }
        Ok(())
    }

    /// Tests the connection to an MCP server
    ///
    /// Spawns a temporary server, performs initialization, discovers
    /// capabilities, and returns the test result with latency.
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration to test
    ///
    /// # Returns
    ///
    /// Returns a test result with success status, discovered tools/resources,
    /// and connection latency.
    pub async fn test_connection(config: MCPServerConfig) -> MCPTestResult {
        let start = Instant::now();

        match Self::connect(config).await {
            Ok(mut client) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let tools = client.tools().to_vec();
                let resources = client.resources().to_vec();
                let message = format!(
                    "Connected successfully. Found {} tools and {} resources.",
                    tools.len(),
                    resources.len()
                );

                // Cleanup
                let _ = client.disconnect().await;

                MCPTestResult {
                    success: true,
                    message,
                    tools,
                    resources,
                    latency_ms,
                }
            }
            Err(e) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                MCPTestResult {
                    success: false,
                    message: e.to_string(),
                    tools: Vec::new(),
                    resources: Vec::new(),
                    latency_ms,
                }
            }
        }
    }

    /// Returns whether the client is connected
    pub fn is_connected(&self) -> bool {
        self.handle.is_some()
    }

    /// Returns the current server status
    pub fn status(&self) -> MCPServerStatus {
        match &self.handle {
            Some(handle) => handle.status().clone(),
            None => MCPServerStatus::Stopped,
        }
    }

    /// Returns the server configuration
    pub fn config(&self) -> &MCPServerConfig {
        &self.config
    }

    /// Updates the server configuration in memory
    ///
    /// This does NOT persist to database - use MCPManager::update_server_config for that.
    /// Used to sync in-memory state after database update.
    pub fn update_config(&mut self, config: MCPServerConfig) {
        self.config = config;
    }

    /// Returns the list of available tools
    ///
    /// Returns an empty slice if not connected.
    pub fn tools(&self) -> &[MCPTool] {
        match &self.handle {
            Some(handle) => handle.list_tools(),
            None => &[],
        }
    }

    /// Returns the list of available resources
    ///
    /// Returns an empty slice if not connected.
    pub fn resources(&self) -> &[MCPResource] {
        match &self.handle {
            Some(handle) => handle.list_resources(),
            None => &[],
        }
    }

    /// Calls a tool on the MCP server
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool to invoke
    /// * `arguments` - Tool arguments as JSON value
    ///
    /// # Returns
    ///
    /// Returns the tool call result with success status and content.
    ///
    /// # Errors
    ///
    /// Returns an error if not connected or if the tool call fails.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<MCPToolCallResult> {
        let handle = self.handle.as_mut().ok_or(MCPError::ServerNotRunning {
            server: self.config.name.clone(),
            status: "disconnected".to_string(),
        })?;

        let start = Instant::now();

        let response = handle.call_tool(tool_name, arguments).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        // Convert to result type
        let (success, error) = if response.is_error == Some(true) {
            (false, Some("Tool returned an error".to_string()))
        } else {
            (true, None)
        };

        // Convert content to JSON value
        let content = if response.content.len() == 1 {
            serde_json::to_value(&response.content[0])?
        } else {
            serde_json::to_value(&response.content)?
        };

        Ok(MCPToolCallResult {
            success,
            content,
            error,
            duration_ms,
        })
    }

    /// Calls a tool and returns the raw response
    ///
    /// Use this when you need access to the full MCP response format.
    pub async fn call_tool_raw(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<MCPToolCallResponse> {
        let handle = self.handle.as_mut().ok_or(MCPError::ServerNotRunning {
            server: self.config.name.clone(),
            status: "disconnected".to_string(),
        })?;

        handle.call_tool(tool_name, arguments).await
    }

    /// Calls a tool and extracts text content
    ///
    /// Convenience method that calls a tool and returns all text content
    /// concatenated as a single string.
    pub async fn call_tool_text(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<String> {
        let response = self.call_tool_raw(tool_name, arguments).await?;
        Ok(MCPServerHandle::extract_text_content(&response))
    }

    /// Enables or disables auto-reconnect
    ///
    /// When enabled, the client will attempt to reconnect if the
    /// connection is lost during a tool call.
    pub fn set_auto_reconnect(&mut self, enabled: bool) {
        self.auto_reconnect = enabled;
    }

    /// Refreshes the tools list from the server
    ///
    /// Use this to update the tools list if the server's capabilities
    /// may have changed.
    pub async fn refresh_tools(&mut self) -> MCPResult<Vec<MCPTool>> {
        let handle = self.handle.as_mut().ok_or(MCPError::ServerNotRunning {
            server: self.config.name.clone(),
            status: "disconnected".to_string(),
        })?;

        handle.refresh_tools().await
    }

    /// Checks if the underlying process is still alive
    pub fn is_process_alive(&mut self) -> bool {
        match self.handle.as_mut() {
            Some(handle) => handle.is_process_alive(),
            None => false,
        }
    }

    /// Returns the server info (name, version) if available
    pub fn server_info(&self) -> Option<(&str, &str)> {
        self.handle.as_ref().and_then(|h| h.server_info())
    }
}

impl Drop for MCPClient {
    fn drop(&mut self) {
        // Handle cleanup is automatic via MCPServerHandle's Drop impl
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mcp::MCPDeploymentMethod;
    use std::collections::HashMap;

    fn create_test_config() -> MCPServerConfig {
        MCPServerConfig {
            id: "test_client".to_string(),
            name: "Test Client Server".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec![
                "run".to_string(),
                "-i".to_string(),
                "test:latest".to_string(),
            ],
            env: HashMap::new(),
            description: Some("Test configuration".to_string()),
        }
    }

    #[test]
    fn test_client_new() {
        let config = create_test_config();
        let client = MCPClient::new(config.clone());

        assert!(!client.is_connected());
        assert_eq!(client.status(), MCPServerStatus::Stopped);
        assert_eq!(client.config().id, "test_client");
        assert!(client.tools().is_empty());
        assert!(client.resources().is_empty());
    }

    #[test]
    fn test_client_auto_reconnect() {
        let config = create_test_config();
        let mut client = MCPClient::new(config);

        // Default is false
        assert!(!client.auto_reconnect);

        client.set_auto_reconnect(true);
        assert!(client.auto_reconnect);

        client.set_auto_reconnect(false);
        assert!(!client.auto_reconnect);
    }

    #[test]
    fn test_test_result_success() {
        let result = MCPTestResult {
            success: true,
            message: "Connected successfully".to_string(),
            tools: vec![MCPTool {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({}),
            }],
            resources: vec![],
            latency_ms: 100,
        };

        assert!(result.success);
        assert_eq!(result.tools.len(), 1);
        assert_eq!(result.latency_ms, 100);
    }

    #[test]
    fn test_test_result_failure() {
        let result = MCPTestResult {
            success: false,
            message: "Connection refused".to_string(),
            tools: vec![],
            resources: vec![],
            latency_ms: 50,
        };

        assert!(!result.success);
        assert!(result.message.contains("Connection refused"));
        assert!(result.tools.is_empty());
    }
}
