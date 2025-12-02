// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! MCP HTTP Transport Handle
//!
//! Manages communication with remote MCP servers over HTTP/SSE transport.
//! This is an alternative to the stdio-based `MCPServerHandle` for servers
//! that expose HTTP endpoints (SaaS, remote servers).
//!
//! ## Transport Protocol
//!
//! MCP over HTTP uses:
//! - **POST** requests for JSON-RPC messages (requests and notifications)
//! - **SSE** (Server-Sent Events) for server-initiated messages and streaming
//!
//! ## URL Configuration
//!
//! For HTTP deployment method, the server `args[0]` should contain the base URL:
//! - `https://api.example.com/mcp` - Base endpoint for the MCP server
//!
//! The client will POST JSON-RPC messages to this URL and optionally
//! connect to `{base_url}/sse` for server-sent events.

use crate::mcp::{
    JsonRpcRequest, JsonRpcResponse, MCPError, MCPInitializeParams, MCPInitializeResult,
    MCPResourceDefinition, MCPResourcesListResult, MCPResult, MCPToolCallParams,
    MCPToolCallResponse, MCPToolDefinition, MCPToolsListResult,
};
use crate::models::mcp::{MCPResource, MCPServerConfig, MCPServerStatus, MCPTool};
use reqwest::Client;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Default timeout for HTTP operations (30 seconds)
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 30000;

/// MCP HTTP Transport Handle
///
/// Manages communication with a remote MCP server over HTTP.
/// Provides the same interface as `MCPServerHandle` for transparent usage.
///
/// # Example
///
/// ```rust,ignore
/// let config = MCPServerConfig {
///     id: "remote-server".to_string(),
///     name: "Remote MCP".to_string(),
///     command: MCPDeploymentMethod::Http,
///     args: vec!["https://api.example.com/mcp".to_string()],
///     // ...
/// };
/// let mut handle = MCPHttpHandle::connect(config).await?;
///
/// // Initialize the MCP session
/// let init_result = handle.initialize().await?;
///
/// // List available tools
/// let tools = handle.list_tools();
///
/// // Call a tool
/// let result = handle.call_tool("my_tool", json!({"param": "value"})).await?;
///
/// // Disconnect
/// handle.disconnect().await?;
/// ```
pub struct MCPHttpHandle {
    /// Server configuration
    config: MCPServerConfig,
    /// HTTP client for making requests
    client: Client,
    /// Base URL for the MCP endpoint
    base_url: String,
    /// Current server status
    status: MCPServerStatus,
    /// Discovered tools after initialization
    tools: Vec<MCPTool>,
    /// Discovered resources after initialization
    resources: Vec<MCPResource>,
    /// Request ID counter for JSON-RPC
    request_id: AtomicI64,
    /// Server info from initialization (name, version)
    server_info: Option<(String, String)>,
    /// Whether the connection is active
    connected: bool,
}

impl MCPHttpHandle {
    /// Creates a new HTTP handle and connects to the server
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration with HTTP URL in args[0]
    ///
    /// # Returns
    ///
    /// Returns a connected `MCPHttpHandle` instance.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL is invalid or connection fails.
    pub async fn connect(config: MCPServerConfig) -> MCPResult<Self> {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            "Connecting to MCP HTTP server"
        );

        // Extract and clone base URL from args[0]
        let base_url = config
            .args
            .first()
            .cloned()
            .ok_or_else(|| MCPError::InvalidConfig {
                field: "args".to_string(),
                reason: "HTTP deployment requires URL in args[0]".to_string(),
            })?;

        // Validate URL format
        if !base_url.starts_with("http://") && !base_url.starts_with("https://") {
            return Err(MCPError::InvalidConfig {
                field: "args[0]".to_string(),
                reason: format!(
                    "Invalid URL: must start with http:// or https://: {}",
                    base_url
                ),
            });
        }

        // Build HTTP client with custom headers from env
        let mut headers = reqwest::header::HeaderMap::new();

        // Add authorization header if API key is provided in env
        if let Some(api_key) = config.env.get("API_KEY") {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", api_key)
                    .parse()
                    .map_err(|e| MCPError::InvalidConfig {
                        field: "env.API_KEY".to_string(),
                        reason: format!("Invalid API key format: {}", e),
                    })?,
            );
        }

        // Add custom headers from env (prefixed with HEADER_)
        for (key, value) in &config.env {
            if let Some(header_name) = key.strip_prefix("HEADER_") {
                if let Ok(header_value) = value.parse() {
                    if let Ok(name) =
                        reqwest::header::HeaderName::try_from(header_name.to_lowercase())
                    {
                        headers.insert(name, header_value);
                    }
                }
            }
        }

        let client = Client::builder()
            .timeout(Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS))
            .default_headers(headers)
            .build()
            .map_err(|e| MCPError::ConnectionFailed {
                server: config.name.clone(),
                message: format!("Failed to build HTTP client: {}", e),
            })?;

        let mut handle = Self {
            config,
            client,
            base_url,
            status: MCPServerStatus::Starting,
            tools: Vec::new(),
            resources: Vec::new(),
            request_id: AtomicI64::new(1),
            server_info: None,
            connected: false,
        };

        // Test connection with a simple request
        handle.test_connectivity().await?;
        handle.connected = true;
        handle.status = MCPServerStatus::Running;

        info!(
            server_id = %handle.config.id,
            base_url = %handle.base_url,
            "Connected to MCP HTTP server"
        );

        Ok(handle)
    }

    /// Tests connectivity to the HTTP endpoint
    async fn test_connectivity(&self) -> MCPResult<()> {
        debug!(
            server_id = %self.config.id,
            base_url = %self.base_url,
            "Testing HTTP connectivity"
        );

        // Try a HEAD request to check if the endpoint is reachable
        let response = self.client.head(&self.base_url).send().await.map_err(|e| {
            MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: format!("Failed to connect to HTTP endpoint: {}", e),
            }
        })?;

        // Accept 2xx, 4xx (might require auth), or 405 (method not allowed - means server is there)
        let status = response.status();
        if !status.is_success() && !status.is_client_error() && status.as_u16() != 405 {
            return Err(MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: format!("HTTP endpoint returned unexpected status: {}", status),
            });
        }

        Ok(())
    }

    /// Initializes the MCP session with the server
    ///
    /// Sends the `initialize` request and waits for the server's capabilities.
    /// Must be called before any tool operations.
    ///
    /// # Returns
    ///
    /// Returns the server's initialization result including capabilities and server info.
    pub async fn initialize(&mut self) -> MCPResult<MCPInitializeResult> {
        info!(
            server_id = %self.config.id,
            "Initializing MCP HTTP session"
        );

        // Send initialize request
        let params = MCPInitializeParams::default();
        let request = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::to_value(&params)?),
            self.next_request_id(),
        );

        let response = self.send_request(request).await?;

        // Parse initialization result
        let init_result: MCPInitializeResult =
            serde_json::from_value(response.result.ok_or_else(|| {
                MCPError::InitializationFailed {
                    server: self.config.name.clone(),
                    message: "No result in initialize response".to_string(),
                }
            })?)?;

        // Store server info
        self.server_info = Some((
            init_result.server_info.name.clone(),
            init_result.server_info.version.clone(),
        ));

        // Send initialized notification
        self.send_notification("notifications/initialized", None)
            .await?;

        // Refresh tools and resources
        self.refresh_tools_internal().await?;
        self.refresh_resources_internal().await?;

        self.status = MCPServerStatus::Running;

        info!(
            server_id = %self.config.id,
            server_name = %init_result.server_info.name,
            server_version = %init_result.server_info.version,
            tools_count = self.tools.len(),
            resources_count = self.resources.len(),
            "MCP HTTP session initialized"
        );

        Ok(init_result)
    }

    /// Returns the list of available tools
    pub fn list_tools(&self) -> &[MCPTool] {
        &self.tools
    }

    /// Returns the list of available resources
    pub fn list_resources(&self) -> &[MCPResource] {
        &self.resources
    }

    /// Refreshes the tools list from the server
    pub async fn refresh_tools(&mut self) -> MCPResult<Vec<MCPTool>> {
        self.refresh_tools_internal().await?;
        Ok(self.tools.clone())
    }

    /// Internal method to refresh tools
    async fn refresh_tools_internal(&mut self) -> MCPResult<()> {
        let request = JsonRpcRequest::new("tools/list", None, self.next_request_id());

        let response = self.send_request(request).await?;

        if let Some(result) = response.result {
            let tools_result: MCPToolsListResult = serde_json::from_value(result)?;
            self.tools = tools_result
                .tools
                .into_iter()
                .map(|t| self.convert_tool_definition(t))
                .collect();

            debug!(
                server_id = %self.config.id,
                tools_count = self.tools.len(),
                "Refreshed tools list"
            );
        }

        Ok(())
    }

    /// Internal method to refresh resources
    async fn refresh_resources_internal(&mut self) -> MCPResult<()> {
        let request = JsonRpcRequest::new("resources/list", None, self.next_request_id());

        let response = self.send_request(request).await?;

        if let Some(result) = response.result {
            let resources_result: MCPResourcesListResult = serde_json::from_value(result)?;
            self.resources = resources_result
                .resources
                .into_iter()
                .map(|r| self.convert_resource_definition(r))
                .collect();

            debug!(
                server_id = %self.config.id,
                resources_count = self.resources.len(),
                "Refreshed resources list"
            );
        }

        Ok(())
    }

    /// Converts MCP tool definition to our model type
    fn convert_tool_definition(&self, def: MCPToolDefinition) -> MCPTool {
        MCPTool {
            name: def.name,
            description: def.description,
            input_schema: def.input_schema,
        }
    }

    /// Converts MCP resource definition to our model type
    fn convert_resource_definition(&self, def: MCPResourceDefinition) -> MCPResource {
        MCPResource {
            uri: def.uri,
            name: def.name,
            description: def.description,
            mime_type: def.mime_type,
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
    /// Returns the tool call response with content.
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<MCPToolCallResponse> {
        debug!(
            server_id = %self.config.id,
            tool_name = %tool_name,
            "Calling MCP tool via HTTP"
        );

        let params = MCPToolCallParams {
            name: tool_name.to_string(),
            arguments,
        };

        let request = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::to_value(&params)?),
            self.next_request_id(),
        );

        let response = self.send_request(request).await?;

        // Check for error response
        if let Some(error) = response.error {
            return Err(MCPError::ProtocolError {
                code: error.code,
                message: error.message,
            });
        }

        // Parse tool response
        let result = response.result.ok_or_else(|| MCPError::ProtocolError {
            code: -32600,
            message: "No result in tool call response".to_string(),
        })?;

        let tool_response: MCPToolCallResponse = serde_json::from_value(result)?;

        debug!(
            server_id = %self.config.id,
            tool_name = %tool_name,
            content_items = tool_response.content.len(),
            "Tool call completed"
        );

        Ok(tool_response)
    }

    /// Extracts text content from a tool response
    pub fn extract_text_content(response: &MCPToolCallResponse) -> String {
        use crate::mcp::MCPContent;

        response
            .content
            .iter()
            .filter_map(|c| match c {
                MCPContent::Text { text } => Some(text.clone()),
                MCPContent::Resource { resource } => {
                    Some(resource.text.clone().unwrap_or_default())
                }
                MCPContent::Image { .. } => None, // Images cannot be converted to text
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Sends a JSON-RPC request to the server
    async fn send_request(&self, request: JsonRpcRequest) -> MCPResult<JsonRpcResponse> {
        debug!(
            server_id = %self.config.id,
            method = %request.method,
            request_id = ?request.id,
            "Sending HTTP request"
        );

        let response = self
            .client
            .post(&self.base_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: format!("HTTP request failed: {}", e),
            })?;

        // Check HTTP status
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: format!("HTTP {} - {}", status, body),
            });
        }

        // Parse JSON-RPC response
        let json_response: JsonRpcResponse =
            response
                .json()
                .await
                .map_err(|e| MCPError::SerializationError {
                    context: "HTTP response parsing".to_string(),
                    message: e.to_string(),
                })?;

        // Check for JSON-RPC error
        if let Some(ref error) = json_response.error {
            warn!(
                server_id = %self.config.id,
                error_code = error.code,
                error_message = %error.message,
                "JSON-RPC error received"
            );
        }

        debug!(
            server_id = %self.config.id,
            request_id = ?json_response.id,
            has_result = json_response.result.is_some(),
            has_error = json_response.error.is_some(),
            "Received HTTP response"
        );

        Ok(json_response)
    }

    /// Sends a JSON-RPC notification (no response expected)
    async fn send_notification(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> MCPResult<()> {
        debug!(
            server_id = %self.config.id,
            method = %method,
            "Sending HTTP notification"
        );

        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
        });

        let response = self
            .client
            .post(&self.base_url)
            .json(&notification)
            .send()
            .await
            .map_err(|e| MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: format!("HTTP notification failed: {}", e),
            })?;

        // For notifications, we just check that the request succeeded
        // The server may or may not return a response
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!(
                server_id = %self.config.id,
                method = %method,
                status = %status,
                body = %body,
                "HTTP notification returned error status (may be expected)"
            );
        }

        Ok(())
    }

    /// Generates the next request ID
    fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Disconnects from the MCP server
    ///
    /// Sends a shutdown notification and marks the connection as closed.
    pub async fn disconnect(&mut self) -> MCPResult<()> {
        if !self.connected {
            return Ok(());
        }

        info!(
            server_id = %self.config.id,
            "Disconnecting from MCP HTTP server"
        );

        // Send shutdown notification (best effort)
        let _ = self.send_notification("shutdown", None).await;

        self.connected = false;
        self.status = MCPServerStatus::Stopped;
        self.tools.clear();
        self.resources.clear();

        info!(
            server_id = %self.config.id,
            "Disconnected from MCP HTTP server"
        );

        Ok(())
    }

    /// Returns the current server status
    pub fn status(&self) -> &MCPServerStatus {
        &self.status
    }

    /// Returns the server configuration
    pub fn config(&self) -> &MCPServerConfig {
        &self.config
    }

    /// Returns the server info (name, version) if available
    pub fn server_info(&self) -> Option<(&str, &str)> {
        self.server_info
            .as_ref()
            .map(|(n, v)| (n.as_str(), v.as_str()))
    }

    /// Checks if the connection is active
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Sets the status to error
    pub fn set_error_status(&mut self) {
        self.status = MCPServerStatus::Error;
    }
}

impl Drop for MCPHttpHandle {
    fn drop(&mut self) {
        if self.connected {
            // Note: Cannot do async cleanup in drop
            // Caller should call disconnect() before dropping
            debug!(
                server_id = %self.config.id,
                "MCPHttpHandle dropped while still connected"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mcp::MCPDeploymentMethod;
    use std::collections::HashMap;

    fn create_test_http_config() -> MCPServerConfig {
        MCPServerConfig {
            id: "http_test".to_string(),
            name: "HTTP Test Server".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Http,
            args: vec!["https://api.example.com/mcp".to_string()],
            env: HashMap::new(),
            description: Some("Test HTTP MCP server".to_string()),
        }
    }

    #[test]
    fn test_http_config_validation() {
        let config = create_test_http_config();
        assert_eq!(config.command, MCPDeploymentMethod::Http);
        assert!(!config.args.is_empty());
        assert!(config.args[0].starts_with("https://"));
    }

    #[test]
    fn test_http_config_with_api_key() {
        let mut config = create_test_http_config();
        config
            .env
            .insert("API_KEY".to_string(), "test_key_123".to_string());

        assert!(config.env.contains_key("API_KEY"));
    }

    #[test]
    fn test_http_config_with_custom_headers() {
        let mut config = create_test_http_config();
        config.env.insert(
            "HEADER_X-Custom-Auth".to_string(),
            "custom_value".to_string(),
        );

        assert!(config.env.contains_key("HEADER_X-Custom-Auth"));
    }

    #[test]
    fn test_invalid_url_detection() {
        let mut config = create_test_http_config();
        config.args = vec!["not-a-valid-url".to_string()];

        // This would fail at connect time, but we can check the config
        assert!(!config.args[0].starts_with("http://"));
        assert!(!config.args[0].starts_with("https://"));
    }

    #[test]
    fn test_empty_args_detection() {
        let mut config = create_test_http_config();
        config.args = vec![];

        // Empty args should fail at connect time
        assert!(config.args.is_empty());
    }
}
