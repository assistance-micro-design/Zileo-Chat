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
use crate::mcp::protocol::MCPServerCapabilities;
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
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

/// Default timeout for HTTP operations (30 seconds)
const DEFAULT_HTTP_TIMEOUT_MS: u64 = 30000;

/// Minimum delay between consecutive HTTP requests to the same host (ms).
///
/// Prevents rate-limiting on shared-hosting WAFs that ban-throttle around
/// 3 req/s on a 1 s sliding window. At 500 ms the burst centred on the 1 s
/// mark contains 3 requests (initialize + notif/initialized + tools/list)
/// which is exactly the trip threshold. 700 ms keeps every 1 s window at
/// ≤ 2 requests, with margin for clock skew and TLS jitter.
const HTTP_THROTTLE_DELAY_MS: u64 = 700;

/// Per-host throttle state, shared across every `MCPHttpHandle` that targets
/// the same hostname.
///
/// This prevents bursts when several handles hit the same host in quick
/// succession (e.g. "Test connection" followed by "Save server" each issue
/// 5 requests within 2 seconds — without a shared throttle they cumulate to
/// ~5 req/s for the same hostname, which trips shared-host rate limits.
///
/// Loopback hosts (`localhost`, `127.0.0.0/8`, `[::1]`) are not registered
/// here — see `get_host_throttle`.
static HOST_THROTTLES: LazyLock<Mutex<HashMap<String, Arc<Mutex<Instant>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Extracts the lowercased hostname from a URL string.
///
/// Returns `None` if the URL has no scheme or no host. Handles userinfo
/// (`user:pass@host`), explicit ports (`host:8080`), IPv6 brackets
/// (`[::1]:port`), and trailing path/query/fragment.
///
/// Delegates to `reqwest::Url` (the battle-tested `url` crate) for parsing,
/// then re-brackets IPv6 literals so `[::1]:8080` and `[::1]:9090` share a
/// single throttle key.
fn extract_host(url: &str) -> Option<String> {
    // Guard against the WHATWG URL parser collapsing `://` + extra slashes:
    // `https:///path` would otherwise yield host="path" instead of None.
    // We treat an empty authority as "no host" — the same semantic our
    // call sites rely on.
    let after_scheme = url.split_once("://")?.1;
    if after_scheme.starts_with('/') {
        return None;
    }
    let parsed = reqwest::Url::parse(url).ok()?;
    let host_str = parsed.host_str()?;
    if host_str.is_empty() {
        return None;
    }
    // url::Url normalises ASCII hosts to lowercase, but be defensive in
    // case a future version surfaces unicode/percent-encoded forms.
    let lowered = host_str.to_ascii_lowercase();
    // IPv6 literals contain `:` (segment separator). url::Url::host_str
    // historically strips the brackets; restore them so the throttle key
    // stays consistent with the source URL form.
    if lowered.contains(':') && !lowered.starts_with('[') {
        Some(format!("[{}]", lowered))
    } else {
        Some(lowered)
    }
}

/// Returns true for loopback hostnames that should bypass throttling.
///
/// Loopback hosts have no rate limit and slowing them down only hurts
/// local development (e.g. running an MCP server on `localhost:8080`).
fn is_loopback_host(host: &str) -> bool {
    host == "localhost" || host == "[::1]" || host.starts_with("127.")
}

/// Extracts a cooldown in seconds from a 429 response.
///
/// Tries, in order:
/// 1. The HTTP `Retry-After` header — RFC 9110 allows either a delta-seconds
///    integer or an HTTP-date. We honour the integer form; the date form is
///    silently ignored (callers fall back to the default cooldown).
/// 2. A `<meta name="retry-after" content="N" />` tag in the response body.
///    Some shared-host WAFs emit this even though it is not standard.
///
/// Returns `None` when neither source yields a parseable integer.
fn parse_retry_after_secs(headers: &reqwest::header::HeaderMap, body: &str) -> Option<u64> {
    if let Some(raw) = headers.get(reqwest::header::RETRY_AFTER) {
        if let Ok(s) = raw.to_str() {
            if let Ok(n) = s.trim().parse::<u64>() {
                return Some(n);
            }
        }
    }

    // Fallback: scan the HTML body for `<meta name="retry-after" content="N">`.
    // We do a tolerant string scan rather than pull in an HTML parser; the
    // tag is short and quote-style varies (single, double, none).
    //
    // Cap at 8 KiB — WAF 429 pages are tiny, and a pathological multi-MB
    // body would otherwise allocate a full lowercased copy. Walk back to a
    // char boundary so the slice stays valid UTF-8.
    const SCAN_LIMIT: usize = 8 * 1024;
    let scan = if body.len() <= SCAN_LIMIT {
        body
    } else {
        let mut end = SCAN_LIMIT;
        while end > 0 && !body.is_char_boundary(end) {
            end -= 1;
        }
        &body[..end]
    };

    // Single allocation: lowercase once, drive both the `name=` lookup and
    // the `content=` lookup off the same buffer.
    let lower = scan.to_ascii_lowercase();
    let needle = "name=\"retry-after\"";
    let alt = "name='retry-after'";
    let idx = lower.find(needle).or_else(|| lower.find(alt))?;
    let after = &scan[idx..];
    let content_idx = lower[idx..].find("content=")?;
    let after_content = &after[content_idx + "content=".len()..];
    let bytes = after_content.as_bytes();
    let (start, quote) = match bytes.first()? {
        b'"' => (1, b'"'),
        b'\'' => (1, b'\''),
        _ => (0, b' '), // unquoted: read until whitespace or `>`
    };
    let rest = &after_content[start..];
    let end = rest
        .bytes()
        .position(|b| b == quote || (quote == b' ' && (b == b' ' || b == b'>' || b == b'/')))
        .unwrap_or(rest.len());
    rest[..end].trim().parse::<u64>().ok()
}

/// Returns the shared throttle mutex for `host`, creating it lazily.
///
/// Returns `None` for loopback hosts (no throttling needed).
async fn get_host_throttle(host: &str) -> Option<Arc<Mutex<Instant>>> {
    if is_loopback_host(host) {
        return None;
    }
    let mut map = HOST_THROTTLES.lock().await;
    let arc = map
        .entry(host.to_string())
        .or_insert_with(|| {
            Arc::new(Mutex::new(
                Instant::now() - Duration::from_millis(HTTP_THROTTLE_DELAY_MS),
            ))
        })
        .clone();
    Some(arc)
}

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
    /// Lowercased hostname extracted from `base_url`, used as the key into
    /// `HOST_THROTTLES` so all handles targeting the same host share a single
    /// rate-limit cadence. Empty for loopback hosts (throttling bypassed).
    host: String,
    /// Capabilities advertised by the server in its `initialize` response.
    /// Used to skip `tools/list` and `resources/list` calls when the server
    /// announces it does not support those capabilities — saves 1-2 HTTP
    /// requests per connect on rate-limited hosts.
    capabilities: MCPServerCapabilities,
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

        // Extract the hostname so this handle joins the per-host throttle
        // cadence. An unparseable URL falls back to an empty key — the
        // throttle still works (it just degenerates to per-empty-host
        // sharing), and the URL would have failed validation above anyway.
        let host = extract_host(&base_url).unwrap_or_default();

        let handle = Self {
            config,
            client,
            base_url,
            status: MCPServerStatus::Starting,
            tools: Vec::new(),
            resources: Vec::new(),
            request_id: AtomicI64::new(1),
            server_info: None,
            connected: true,
            headers,
            host,
            capabilities: MCPServerCapabilities::default(),
        };

        // No prelude HEAD request: it isn't part of the MCP wire protocol
        // and burns a request against shared-host rate limits. The
        // subsequent `initialize` POST already validates that the endpoint
        // speaks MCP. Mark connected so the Drop impl doesn't whine if
        // `initialize` fails (caller handles the error and the handle is
        // dropped immediately after).

        info!(
            server_id = %handle.config.id,
            base_url = %handle.base_url,
            "Prepared MCP HTTP handle"
        );

        Ok(handle)
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

        // Store server info + capabilities so we can skip irrelevant
        // follow-up calls (saves 1-2 HTTP requests per connect on
        // rate-limited hosts).
        self.server_info = Some((
            init_result.server_info.name.clone(),
            init_result.server_info.version.clone(),
        ));
        self.capabilities = init_result.capabilities.clone();

        // Send initialized notification
        self.send_notification("notifications/initialized", None)
            .await?;

        // Only fetch tools/resources the server actually advertises. Per the
        // MCP spec, calling `tools/list` or `resources/list` against a server
        // that didn't declare the capability is undefined — and on a shared
        // host that ban-throttles per-IP, every wasted POST counts toward
        // the budget.
        if self.capabilities.tools.is_some() {
            self.refresh_tools_internal().await?;
        } else {
            // info! (not debug!) so spec-non-compliant servers that omit
            // `capabilities.tools` while actually serving `tools/list` are
            // diagnosable in production logs without a debug build.
            info!(
                server_id = %self.config.id,
                "Skipping tools/list — server did not advertise tools capability"
            );
        }
        if self.capabilities.resources.is_some() {
            self.refresh_resources_internal().await?;
        } else {
            info!(
                server_id = %self.config.id,
                "Skipping resources/list — server did not advertise resources capability"
            );
        }

        self.status = MCPServerStatus::Running;

        info!(
            server_id = %self.config.id,
            server_name = %init_result.server_info.name,
            server_version = %init_result.server_info.version,
            tools_count = self.tools.len(),
            resources_count = self.resources.len(),
            tools_supported = self.capabilities.tools.is_some(),
            resources_supported = self.capabilities.resources.is_some(),
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

    /// Refreshes the tools list from the server.
    ///
    /// Returns the cached (empty) list without contacting the server when
    /// the server did not advertise the `tools` capability in `initialize`.
    /// This avoids burning a request on a method the server has explicitly
    /// said it does not implement (and avoids a needless 429 against
    /// rate-limited shared hosts).
    pub async fn refresh_tools(&mut self) -> MCPResult<Vec<MCPTool>> {
        if self.capabilities.tools.is_none() {
            debug!(
                server_id = %self.config.id,
                "refresh_tools no-op — server did not advertise tools capability"
            );
            return Ok(self.tools.clone());
        }
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

    /// Enforces minimum delay between consecutive HTTP requests to the same
    /// host. The cadence is shared across all `MCPHttpHandle` instances that
    /// target the same hostname so concurrent flows (test connection +
    /// register, parallel `load_from_db()` startup, etc.) cannot bypass the
    /// per-host rate limit. Loopback hosts are exempted.
    async fn throttle(&self) {
        let throttle = match get_host_throttle(&self.host).await {
            Some(t) => t,
            None => return,
        };

        let mut last_time = throttle.lock().await;
        let elapsed = last_time.elapsed();
        let min_delay = Duration::from_millis(HTTP_THROTTLE_DELAY_MS);
        if elapsed < min_delay {
            let wait = min_delay - elapsed;
            debug!(
                server_id = %self.config.id,
                host = %self.host,
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
            let response_headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            // Surface 429 as a typed RateLimited error so the UI can render
            // a clean cooldown message instead of upstream HTML.
            if status.as_u16() == 429 {
                let retry_after_secs = parse_retry_after_secs(&response_headers, &body);
                return Err(MCPError::RateLimited {
                    server: self.config.name.clone(),
                    retry_after_secs,
                });
            }
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
        // The server may or may not return a response. The one exception
        // is 429: that's the host rate-limiting us, and silently swallowing
        // it would let the init sequence continue and burn more requests
        // into a ban window.
        let status = response.status();
        if status.as_u16() == 429 {
            let response_headers = response.headers().clone();
            let body = response.text().await.unwrap_or_default();
            let retry_after_secs = parse_retry_after_secs(&response_headers, &body);
            return Err(MCPError::RateLimited {
                server: self.config.name.clone(),
                retry_after_secs,
            });
        }
        if !status.is_success() {
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

    // -------- parse_retry_after_secs --------

    fn empty_headers() -> reqwest::header::HeaderMap {
        reqwest::header::HeaderMap::new()
    }

    fn headers_with_retry_after(value: &str) -> reqwest::header::HeaderMap {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_str(value).unwrap(),
        );
        h
    }

    #[test]
    fn test_parse_retry_after_from_http_header_seconds() {
        let h = headers_with_retry_after("120");
        assert_eq!(parse_retry_after_secs(&h, ""), Some(120));
    }

    #[test]
    fn test_parse_retry_after_trims_whitespace() {
        let h = headers_with_retry_after("  42  ");
        assert_eq!(parse_retry_after_secs(&h, ""), Some(42));
    }

    #[test]
    fn test_parse_retry_after_http_date_returns_none() {
        // RFC 9110 also allows HTTP-date; we don't honour it.
        let h = headers_with_retry_after("Wed, 21 Oct 2026 07:28:00 GMT");
        assert_eq!(parse_retry_after_secs(&h, ""), None);
    }

    #[test]
    fn test_parse_retry_after_meta_double_quotes() {
        let body = r#"<html><head><meta name="retry-after" content="240" /></head></html>"#;
        assert_eq!(parse_retry_after_secs(&empty_headers(), body), Some(240));
    }

    #[test]
    fn test_parse_retry_after_meta_single_quotes() {
        let body = r#"<html><meta name='retry-after' content='90'/></html>"#;
        assert_eq!(parse_retry_after_secs(&empty_headers(), body), Some(90));
    }

    #[test]
    fn test_parse_retry_after_meta_case_insensitive() {
        let body = r#"<META NAME="Retry-After" CONTENT="60">"#;
        assert_eq!(parse_retry_after_secs(&empty_headers(), body), Some(60));
    }

    #[test]
    fn test_parse_retry_after_header_takes_precedence_over_meta() {
        let h = headers_with_retry_after("10");
        let body = r#"<meta name="retry-after" content="240">"#;
        assert_eq!(parse_retry_after_secs(&h, body), Some(10));
    }

    #[test]
    fn test_parse_retry_after_returns_none_when_absent() {
        assert_eq!(parse_retry_after_secs(&empty_headers(), ""), None);
        assert_eq!(
            parse_retry_after_secs(&empty_headers(), "<html><body>nope</body></html>"),
            None
        );
    }

    #[test]
    fn test_parse_retry_after_meta_fallback_real_payload() {
        // Trimmed snapshot of a real shared-hosting WAF 429 page that emits
        // the cooldown only via <meta>, not via the Retry-After header.
        let body = r#"<!DOCTYPE HTML>
<html lang="en-US">
<head>
  <meta name="retry-after" content="240" />
</head>
<body>HTTP 429</body>
</html>"#;
        assert_eq!(parse_retry_after_secs(&empty_headers(), body), Some(240));
    }

    // -------- Per-host throttle helpers --------

    #[test]
    fn test_extract_host_https_simple() {
        assert_eq!(
            extract_host("https://api.example.com/mcp"),
            Some("api.example.com".to_string())
        );
    }

    #[test]
    fn test_extract_host_http_with_port() {
        assert_eq!(
            extract_host("http://example.com:8080/mcp"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn test_extract_host_strips_query_and_fragment() {
        assert_eq!(
            extract_host("https://host.tld/path?a=1#x"),
            Some("host.tld".to_string())
        );
    }

    #[test]
    fn test_extract_host_strips_userinfo() {
        assert_eq!(
            extract_host("https://user:pass@example.com/mcp"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn test_extract_host_lowercases() {
        assert_eq!(
            extract_host("https://Example.COM/mcp"),
            Some("example.com".to_string())
        );
    }

    #[test]
    fn test_extract_host_ipv6_with_port() {
        assert_eq!(
            extract_host("http://[::1]:8080/mcp"),
            Some("[::1]".to_string())
        );
    }

    #[test]
    fn test_extract_host_ipv6_without_port() {
        assert_eq!(
            extract_host("http://[2001:db8::1]/mcp"),
            Some("[2001:db8::1]".to_string())
        );
    }

    #[test]
    fn test_extract_host_no_scheme_returns_none() {
        assert_eq!(extract_host("example.com/mcp"), None);
    }

    #[test]
    fn test_extract_host_empty_authority_returns_none() {
        assert_eq!(extract_host("https:///path"), None);
    }

    #[test]
    fn test_is_loopback_localhost() {
        assert!(is_loopback_host("localhost"));
    }

    #[test]
    fn test_is_loopback_ipv4_loopback_range() {
        assert!(is_loopback_host("127.0.0.1"));
        assert!(is_loopback_host("127.1.2.3"));
    }

    #[test]
    fn test_is_loopback_ipv6() {
        assert!(is_loopback_host("[::1]"));
    }

    #[test]
    fn test_is_loopback_rejects_remote() {
        assert!(!is_loopback_host("example.com"));
        assert!(!is_loopback_host("192.168.1.1"));
        assert!(!is_loopback_host("0.0.0.0"));
    }

    #[tokio::test]
    async fn test_get_host_throttle_returns_none_for_loopback() {
        assert!(get_host_throttle("localhost").await.is_none());
        assert!(get_host_throttle("127.0.0.1").await.is_none());
        assert!(get_host_throttle("[::1]").await.is_none());
    }

    #[tokio::test]
    async fn test_get_host_throttle_shares_arc_for_same_host() {
        // Use a unique host name per test run to avoid interference with
        // other tests sharing the static HOST_THROTTLES map.
        let host = "throttle-share-test.invalid";
        let a = get_host_throttle(host).await.expect("non-loopback");
        let b = get_host_throttle(host).await.expect("non-loopback");
        assert!(
            Arc::ptr_eq(&a, &b),
            "two lookups for the same host must return the same Arc"
        );
    }

    #[tokio::test]
    async fn test_get_host_throttle_separates_different_hosts() {
        let a = get_host_throttle("host-a.invalid")
            .await
            .expect("non-loopback");
        let b = get_host_throttle("host-b.invalid")
            .await
            .expect("non-loopback");
        assert!(
            !Arc::ptr_eq(&a, &b),
            "different hosts must own independent throttles"
        );
    }

    #[tokio::test]
    async fn test_throttle_serializes_concurrent_calls_on_same_host() {
        // Two real handles targeting the same host must share the throttle
        // cadence: back-to-back `throttle()` calls in production code must
        // wait ~HTTP_THROTTLE_DELAY_MS between them.
        //
        // This test exercises `MCPHttpHandle::throttle()` directly (per the
        // project TDD rule "tests must exercise real production code") so a
        // regression in the locking, the host-key derivation, or the elapsed
        // arithmetic is caught here instead of slipping through.
        let mut config_a = create_test_http_config();
        config_a.id = "throttle-prod-a".to_string();
        config_a.args = vec!["https://throttle-prod-test.invalid/mcp".to_string()];
        let mut config_b = config_a.clone();
        config_b.id = "throttle-prod-b".to_string();

        // `auth_type=None` keeps `connect()` off the keychain, so this
        // works in headless CI without OS keyring access. No network is
        // touched until `initialize()`, which we don't call.
        let handle_a = MCPHttpHandle::connect(config_a).await.unwrap();
        let handle_b = MCPHttpHandle::connect(config_b).await.unwrap();

        // Both handles must derive the same host key.
        assert_eq!(handle_a.host, handle_b.host);
        assert!(!handle_a.host.is_empty());

        // Reset the shared cadence so the first throttle() doesn't wait
        // (otherwise a prior test on this host could leave a fresh stamp).
        let throttle = get_host_throttle(&handle_a.host)
            .await
            .expect("non-loopback host registers a throttle");
        {
            let mut t = throttle.lock().await;
            *t = Instant::now() - Duration::from_millis(HTTP_THROTTLE_DELAY_MS);
        }

        let start = Instant::now();
        handle_a.throttle().await; // First call: cadence is stale, no wait.
        handle_b.throttle().await; // Second call: must wait ~HTTP_THROTTLE_DELAY_MS.
        let total = start.elapsed();

        assert!(
            total >= Duration::from_millis(HTTP_THROTTLE_DELAY_MS - 50),
            "back-to-back throttle() on the same host must respect cadence (got {:?})",
            total
        );
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
