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

use crate::mcp::http_auth::build_auth_headers;
use crate::mcp::redact::redact_headers;
use crate::mcp::secrets::load_mcp_secret;
use crate::mcp::{
    JsonRpcRequest, JsonRpcResponse, MCPError, MCPInitializeParams, MCPInitializeResult,
    MCPResourcesListResult, MCPResult, MCPToolCallParams, MCPToolCallResponse, MCPToolsListResult,
};
use crate::models::custom_provider::check_http_warning;
use crate::models::mcp::{MCPAuthType, MCPResource, MCPServerConfig, MCPServerStatus, MCPTool};
use crate::security::KeyStore;
use reqwest::Client;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Default timeout for HTTP operations (30 seconds)
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 30000;

/// Minimum delay between consecutive HTTP requests to the same server (ms).
/// Prevents rate-limiting on shared hosting (e.g. o2switch Tiger Protect).
const HTTP_THROTTLE_DELAY_MS: u64 = 500;

/// Shared HTTP client for connection pooling
///
/// Reuses TCP/TLS connections across all MCPHttpHandle instances.
/// Configured with:
/// - 5 idle connections per host
/// - 90 second idle timeout
/// - 30 second request timeout
static SHARED_HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .pool_max_idle_per_host(5)
        .pool_idle_timeout(Duration::from_secs(90))
        .timeout(Duration::from_millis(DEFAULT_HTTP_TIMEOUT_MS))
        .build()
        .unwrap_or_else(|e| {
            warn!("Failed to create optimized HTTP client: {e}, falling back to default");
            Client::new()
        })
});

/// Builds the HTTP `HeaderMap` for a server config (v1.2).
///
/// Loads the optional auth secret from the OS keychain, runs
/// [`build_auth_headers`], and surfaces any conflict warnings via
/// `tracing::warn!`. Pure auth path — no fallback on legacy env vars.
fn build_headers_from_config(config: &MCPServerConfig) -> MCPResult<reqwest::header::HeaderMap> {
    let auth_type = config.auth_type.unwrap_or(MCPAuthType::None);
    let metadata = config.auth_metadata.clone().unwrap_or_default();
    let extra = config.extra_headers.clone().unwrap_or_default();

    // Only hit the keychain when an auth header is actually needed. The
    // KeyStore constructor performs a few keyring round-trips; skip them
    // for the no-auth case (the most common stdio→http transition path).
    let secret = if auth_type == MCPAuthType::None {
        None
    } else {
        let keystore = KeyStore::new().map_err(|e| MCPError::InvalidConfig {
            field: "keystore".to_string(),
            reason: format!("failed to access keychain: {}", e),
        })?;
        load_mcp_secret(&keystore, &config.id)?
    };

    let (headers, warnings) = build_auth_headers(auth_type, &metadata, secret.as_ref(), &extra)?;

    if !warnings.is_empty() {
        for w in &warnings {
            warn!(
                server_id = %config.id,
                server_name = %config.name,
                "{}",
                w
            );
        }
    }

    debug!(
        server_id = %config.id,
        headers = ?redact_headers(&headers),
        "MCP HTTP handle headers built"
    );

    Ok(headers)
}

/// Detects legacy env-based HTTP auth (`API_KEY` and/or `HEADER_*`) on a
/// HTTP server with `auth_type=None`. Used by the migration banner.
pub fn detect_legacy_http_auth_keys(config: &MCPServerConfig) -> Vec<String> {
    use crate::models::mcp::MCPDeploymentMethod;
    if config.command != MCPDeploymentMethod::Http {
        return Vec::new();
    }
    let auth_active = matches!(config.auth_type, Some(t) if t != MCPAuthType::None);
    if auth_active {
        // The user has already migrated; no banner needed.
        return Vec::new();
    }

    let mut keys: Vec<String> = config
        .env
        .keys()
        .filter(|k| k.as_str() == "API_KEY" || k.starts_with("HEADER_"))
        .cloned()
        .collect();
    keys.sort();
    keys
}

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
    /// Custom headers for this connection (API key, etc.)
    headers: reqwest::header::HeaderMap,
    /// Timestamp of last HTTP request (for throttling)
    last_request_time: Mutex<Instant>,
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

        if let Some(warning_msg) = check_http_warning(&base_url) {
            warn!(
                server_id = %config.id,
                server_name = %config.name,
                url = %base_url,
                "{}",
                warning_msg
            );
        }

        // Build the auth + extra headers from the v1.2 model. The legacy
        // env-based path (API_KEY / HEADER_*) was removed: a HTTP MCP
        // server with `auth_type=None` is now plain — env vars are no
        // longer interpreted by the transport.
        let headers = build_headers_from_config(&config)?;

        // Use shared client for connection pooling
        let client = SHARED_HTTP_CLIENT.clone();

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
            headers,
            last_request_time: Mutex::new(
                Instant::now() - Duration::from_millis(HTTP_THROTTLE_DELAY_MS),
            ),
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
        self.throttle().await;

        debug!(
            server_id = %self.config.id,
            base_url = %self.base_url,
            "Testing HTTP connectivity"
        );

        // Try a HEAD request to check if the endpoint is reachable
        let response = self
            .client
            .head(&self.base_url)
            .headers(self.headers.clone())
            .send()
            .await
            .map_err(|e| MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: format!("Failed to connect to HTTP endpoint: {}", e),
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
                .map(crate::mcp::helpers::convert_tool_definition)
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
                .map(crate::mcp::helpers::convert_resource_definition)
                .collect();

            debug!(
                server_id = %self.config.id,
                resources_count = self.resources.len(),
                "Refreshed resources list"
            );
        }

        Ok(())
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

    /// Sends a JSON-RPC request to the server
    /// Enforces minimum delay between consecutive HTTP requests to avoid rate-limiting.
    async fn throttle(&self) {
        let mut last_time = self.last_request_time.lock().await;
        let elapsed = last_time.elapsed();
        let min_delay = Duration::from_millis(HTTP_THROTTLE_DELAY_MS);
        if elapsed < min_delay {
            let wait = min_delay - elapsed;
            debug!(
                server_id = %self.config.id,
                wait_ms = wait.as_millis(),
                "Throttling HTTP request"
            );
            tokio::time::sleep(wait).await;
        }
        *last_time = Instant::now();
    }

    async fn send_request(&self, request: JsonRpcRequest) -> MCPResult<JsonRpcResponse> {
        self.throttle().await;

        debug!(
            server_id = %self.config.id,
            method = %request.method,
            request_id = ?request.id,
            "Sending HTTP request"
        );

        let response = self
            .client
            .post(&self.base_url)
            .headers(self.headers.clone())
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
        self.throttle().await;

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
            .headers(self.headers.clone())
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
            let server_id = self.config.id.clone();

            // Log that we're dropping without explicit disconnect
            // The shared HTTP client's connection pool will handle cleanup
            warn!(
                server_id = %server_id,
                "MCPHttpHandle dropped while connected - connection will timeout in pool"
            );

            // Mark as disconnected to prevent double-cleanup
            self.connected = false;
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
            auth_type: None,
            auth_metadata: None,
            extra_headers: None,
        }
    }

    /// Test-only entry point that bypasses the keychain (auth_type=None
    /// path only). Production code uses [`build_headers_from_config`].
    fn build_headers_for_tests(config: &MCPServerConfig) -> MCPResult<reqwest::header::HeaderMap> {
        let auth_type = config.auth_type.unwrap_or(MCPAuthType::None);
        let metadata = config.auth_metadata.clone().unwrap_or_default();
        let extra: HashMap<String, String> = config.extra_headers.clone().unwrap_or_default();
        let (headers, _) = build_auth_headers(auth_type, &metadata, None, &extra)?;
        Ok(headers)
    }

    #[test]
    fn test_detect_legacy_http_auth_keys_with_api_key_only() {
        let mut config = create_test_http_config();
        config.env.insert("API_KEY".to_string(), "abc".to_string());
        let keys = detect_legacy_http_auth_keys(&config);
        assert_eq!(keys, vec!["API_KEY"]);
    }

    #[test]
    fn test_detect_legacy_http_auth_keys_with_header_prefix() {
        let mut config = create_test_http_config();
        config
            .env
            .insert("HEADER_X-Trace".to_string(), "abc".to_string());
        config
            .env
            .insert("HEADER_X-Tenant".to_string(), "42".to_string());
        let mut keys = detect_legacy_http_auth_keys(&config);
        keys.sort();
        assert_eq!(keys, vec!["HEADER_X-Tenant", "HEADER_X-Trace"]);
    }

    #[test]
    fn test_detect_legacy_http_auth_keys_skips_when_auth_active() {
        let mut config = create_test_http_config();
        config.env.insert("API_KEY".to_string(), "abc".to_string());
        config.auth_type = Some(MCPAuthType::Bearer);
        // The user has migrated; legacy keys are no longer flagged.
        assert!(detect_legacy_http_auth_keys(&config).is_empty());
    }

    #[test]
    fn test_detect_legacy_http_auth_keys_skips_stdio_servers() {
        let mut config = create_test_http_config();
        config.command = crate::models::mcp::MCPDeploymentMethod::Docker;
        config.env.insert("API_KEY".to_string(), "abc".to_string());
        // Stdio servers (Docker/NPX/UVX) are unaffected by the v1.2 change.
        assert!(detect_legacy_http_auth_keys(&config).is_empty());
    }

    #[test]
    fn test_build_headers_from_config_no_auth_ignores_legacy_env() {
        let mut config = create_test_http_config();
        // Even with legacy env vars present, no Authorization header
        // is generated — that's the breaking change of v1.2.
        config
            .env
            .insert("API_KEY".to_string(), "should-be-ignored".to_string());
        config.env.insert(
            "HEADER_X-Trace".to_string(),
            "should-be-ignored".to_string(),
        );
        let headers = build_headers_for_tests(&config).unwrap();
        assert!(headers.is_empty());
    }

    #[test]
    fn test_build_headers_from_config_extra_headers_only() {
        let mut config = create_test_http_config();
        config.extra_headers = Some({
            let mut h = HashMap::new();
            h.insert("X-Tenant".to_string(), "42".to_string());
            h
        });
        let headers = build_headers_for_tests(&config).unwrap();
        assert_eq!(headers.get("X-Tenant").unwrap(), "42");
        assert!(headers.get(reqwest::header::AUTHORIZATION).is_none());
    }

    // HTTP warning integration tests
    #[test]
    fn test_http_warning_for_remote_http_url() {
        let result = check_http_warning("http://remote-api.com/mcp");
        assert!(result.is_some());
        assert!(result.unwrap().contains("HTTPS"));
    }

    #[test]
    fn test_no_http_warning_for_https_url() {
        let result = check_http_warning("https://remote-api.com/mcp");
        assert!(result.is_none());
    }

    #[test]
    fn test_no_http_warning_for_localhost() {
        assert!(check_http_warning("http://localhost:3000/mcp").is_none());
        assert!(check_http_warning("http://127.0.0.1:8080/mcp").is_none());
        assert!(check_http_warning("http://[::1]:3000/mcp").is_none());
    }
}
