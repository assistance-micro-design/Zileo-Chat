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

//! MCP Server Handle
//!
//! Manages the lifecycle of an external MCP server process, including
//! process spawning, stdin/stdout communication, and cleanup.
//!
//! ## Process Management
//!
//! This module handles three deployment methods:
//! - **Docker**: `docker run -i image:tag`
//! - **NPX**: `npx -y @package/mcp`
//! - **UVX**: `uvx package-name`
//!
//! ## Communication
//!
//! Communication uses JSON-RPC 2.0 over stdio:
//! - Requests are written to the process stdin
//! - Responses are read from the process stdout
//! - Each message is a single JSON line

use crate::mcp::{
    JsonRpcRequest, JsonRpcResponse, MCPContent, MCPError, MCPInitializeParams,
    MCPInitializeResult, MCPResourceDefinition, MCPResourcesListResult, MCPResult,
    MCPToolCallParams, MCPToolCallResponse, MCPToolDefinition, MCPToolsListResult,
};
use crate::models::mcp::{
    MCPDeploymentMethod, MCPResource, MCPServerConfig, MCPServerStatus, MCPTool,
};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Default timeout for MCP operations (30 seconds)
const DEFAULT_TIMEOUT_MS: u64 = 30000;

/// MCP Server Handle
///
/// Manages a single MCP server process and provides methods for
/// JSON-RPC communication.
///
/// # Example
///
/// ```rust,ignore
/// let config = MCPServerConfig { /* ... */ };
/// let mut handle = MCPServerHandle::spawn(config).await?;
///
/// // Initialize the MCP session
/// let init_result = handle.initialize().await?;
/// println!("Connected to: {}", init_result.server_info.name);
///
/// // List available tools
/// let tools = handle.list_tools().await?;
///
/// // Call a tool
/// let result = handle.call_tool("find_symbol", json!({"name": "MyClass"})).await?;
///
/// // Cleanup
/// handle.kill().await?;
/// ```
pub struct MCPServerHandle {
    /// Server configuration
    config: MCPServerConfig,
    /// Child process handle
    child: Option<Child>,
    /// Process stdin for writing requests
    stdin: Option<Mutex<ChildStdin>>,
    /// Process stdout reader for reading responses (Arc for spawn_blocking)
    stdout_reader: Option<Arc<std::sync::Mutex<BufReader<ChildStdout>>>>,
    /// Current server status
    status: MCPServerStatus,
    /// Discovered tools after initialization
    tools: Vec<MCPTool>,
    /// Discovered resources after initialization
    resources: Vec<MCPResource>,
    /// Request ID counter for JSON-RPC
    request_id: AtomicI64,
    /// Server info from initialization
    server_info: Option<(String, String)>,
}

impl MCPServerHandle {
    /// Spawns a new MCP server process
    ///
    /// # Arguments
    ///
    /// * `config` - Server configuration specifying deployment method and arguments
    ///
    /// # Returns
    ///
    /// Returns a new `MCPServerHandle` with the process started but not initialized.
    /// Call `initialize()` to complete the MCP handshake.
    ///
    /// # Errors
    ///
    /// Returns `MCPError::ProcessSpawnFailed` if the process cannot be started.
    pub async fn spawn(config: MCPServerConfig) -> MCPResult<Self> {
        info!(
            server_id = %config.id,
            server_name = %config.name,
            command = %config.command,
            "Spawning MCP server process"
        );

        let (command, args) = Self::build_command(&config)?;

        debug!(
            command = %command,
            args = ?args,
            "Executing MCP server command"
        );

        // Build and spawn the process
        let mut cmd = Command::new(&command);
        cmd.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Set environment variables
        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn().map_err(|e| MCPError::ProcessSpawnFailed {
            command: format!("{} {}", command, args.join(" ")),
            message: e.to_string(),
        })?;

        // Take ownership of stdin/stdout
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| MCPError::ProcessSpawnFailed {
                command: command.clone(),
                message: "Failed to capture stdin".to_string(),
            })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| MCPError::ProcessSpawnFailed {
                command: command.clone(),
                message: "Failed to capture stdout".to_string(),
            })?;

        info!(
            server_id = %config.id,
            pid = ?child.id(),
            "MCP server process started"
        );

        Ok(Self {
            config,
            child: Some(child),
            stdin: Some(Mutex::new(stdin)),
            stdout_reader: Some(Arc::new(std::sync::Mutex::new(BufReader::new(stdout)))),
            status: MCPServerStatus::Starting,
            tools: Vec::new(),
            resources: Vec::new(),
            request_id: AtomicI64::new(1),
            server_info: None,
        })
    }

    /// Builds the command and arguments based on deployment method
    fn build_command(config: &MCPServerConfig) -> MCPResult<(String, Vec<String>)> {
        if config.args.is_empty() {
            return Err(MCPError::InvalidConfig {
                field: "args".to_string(),
                reason: "Command arguments cannot be empty".to_string(),
            });
        }

        let (cmd, args) = match config.command {
            MCPDeploymentMethod::Docker => {
                // Docker: args should be ["run", "-i", "image:tag", ...]
                ("docker".to_string(), config.args.clone())
            }
            MCPDeploymentMethod::Npx => {
                // NPX: args should be ["-y", "@package/mcp", ...]
                ("npx".to_string(), config.args.clone())
            }
            MCPDeploymentMethod::Uvx => {
                // UVX: args should be ["package-name", ...]
                ("uvx".to_string(), config.args.clone())
            }
            MCPDeploymentMethod::Http => {
                // HTTP servers use HTTP/SSE transport, not stdio
                // This handler only supports process-based servers
                return Err(MCPError::InvalidConfig {
                    field: "command".to_string(),
                    reason: "HTTP deployment requires HTTP transport handler (not implemented yet)"
                        .to_string(),
                });
            }
        };

        Ok((cmd, args))
    }

    /// Initializes the MCP session with the server
    ///
    /// Sends the `initialize` request and waits for the server's capabilities.
    /// Must be called before any tool operations.
    ///
    /// # Returns
    ///
    /// Returns the server's initialization result including capabilities and server info.
    ///
    /// # Errors
    ///
    /// Returns `MCPError::InitializationFailed` if the handshake fails.
    pub async fn initialize(&mut self) -> MCPResult<MCPInitializeResult> {
        info!(
            server_id = %self.config.id,
            "Initializing MCP session"
        );

        // Send initialize request
        let params = MCPInitializeParams::default();
        let request = JsonRpcRequest::new(
            "initialize",
            Some(serde_json::to_value(&params)?),
            self.next_request_id(),
        );

        let response = self.send_request(request).await?;
        let result = response
            .into_result()
            .map_err(|e| MCPError::InitializationFailed {
                server: self.config.name.clone(),
                message: e.message,
            })?;

        let init_result: MCPInitializeResult =
            serde_json::from_value(result).map_err(|e| MCPError::InitializationFailed {
                server: self.config.name.clone(),
                message: format!("Invalid initialize response: {}", e),
            })?;

        // Store server info
        self.server_info = Some((
            init_result.server_info.name.clone(),
            init_result.server_info.version.clone(),
        ));

        // Send initialized notification
        let notification = JsonRpcRequest::notification("notifications/initialized", None);
        self.send_notification(notification).await?;

        // Discover tools and resources if supported
        if init_result.capabilities.tools.is_some() {
            match self.list_tools_internal().await {
                Ok(tools) => self.tools = tools,
                Err(e) => warn!(
                    server_id = %self.config.id,
                    error = %e,
                    "Failed to list tools during initialization"
                ),
            }
        }

        if init_result.capabilities.resources.is_some() {
            match self.list_resources_internal().await {
                Ok(resources) => self.resources = resources,
                Err(e) => warn!(
                    server_id = %self.config.id,
                    error = %e,
                    "Failed to list resources during initialization"
                ),
            }
        }

        self.status = MCPServerStatus::Running;

        info!(
            server_id = %self.config.id,
            server_name = %init_result.server_info.name,
            server_version = %init_result.server_info.version,
            tools_count = self.tools.len(),
            resources_count = self.resources.len(),
            "MCP session initialized"
        );

        Ok(init_result)
    }

    /// Lists available tools from the server
    ///
    /// # Returns
    ///
    /// Returns the cached list of tools discovered during initialization.
    /// To refresh, call `refresh_tools()`.
    pub fn list_tools(&self) -> &[MCPTool] {
        &self.tools
    }

    /// Lists available resources from the server
    ///
    /// # Returns
    ///
    /// Returns the cached list of resources discovered during initialization.
    pub fn list_resources(&self) -> &[MCPResource] {
        &self.resources
    }

    /// Refreshes the tools list from the server
    pub async fn refresh_tools(&mut self) -> MCPResult<Vec<MCPTool>> {
        self.tools = self.list_tools_internal().await?;
        Ok(self.tools.clone())
    }

    /// Internal method to fetch tools from server
    async fn list_tools_internal(&mut self) -> MCPResult<Vec<MCPTool>> {
        let request = JsonRpcRequest::new("tools/list", None, self.next_request_id());
        let response = self.send_request(request).await?;

        let result = response
            .into_result()
            .map_err(|e| MCPError::ProtocolError {
                code: e.code,
                message: e.message,
            })?;

        let tools_result: MCPToolsListResult = serde_json::from_value(result)?;

        Ok(tools_result
            .tools
            .into_iter()
            .map(|t| self.convert_tool_definition(t))
            .collect())
    }

    /// Internal method to fetch resources from server
    async fn list_resources_internal(&mut self) -> MCPResult<Vec<MCPResource>> {
        let request = JsonRpcRequest::new("resources/list", None, self.next_request_id());
        let response = self.send_request(request).await?;

        let result = response
            .into_result()
            .map_err(|e| MCPError::ProtocolError {
                code: e.code,
                message: e.message,
            })?;

        let resources_result: MCPResourcesListResult = serde_json::from_value(result)?;

        Ok(resources_result
            .resources
            .into_iter()
            .map(|r| self.convert_resource_definition(r))
            .collect())
    }

    /// Converts a protocol tool definition to the model type
    fn convert_tool_definition(&self, def: MCPToolDefinition) -> MCPTool {
        MCPTool {
            name: def.name,
            description: def.description,
            input_schema: def.input_schema,
        }
    }

    /// Converts a protocol resource definition to the model type
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
    /// * `name` - Name of the tool to invoke
    /// * `arguments` - Tool arguments as JSON value
    ///
    /// # Returns
    ///
    /// Returns the tool's response content.
    ///
    /// # Errors
    ///
    /// Returns `MCPError::ToolNotFound` if the tool doesn't exist,
    /// or `MCPError::ProtocolError` if the call fails.
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> MCPResult<MCPToolCallResponse> {
        // Verify server is running
        if self.status != MCPServerStatus::Running {
            return Err(MCPError::ServerNotRunning {
                server: self.config.name.clone(),
                status: self.status.to_string(),
            });
        }

        debug!(
            server_id = %self.config.id,
            tool_name = %name,
            "Calling MCP tool"
        );

        let params = MCPToolCallParams {
            name: name.to_string(),
            arguments,
        };

        let request = JsonRpcRequest::new(
            "tools/call",
            Some(serde_json::to_value(&params)?),
            self.next_request_id(),
        );

        let response = self.send_request(request).await?;

        let result = response
            .into_result()
            .map_err(|e| MCPError::ProtocolError {
                code: e.code,
                message: e.message,
            })?;

        // Handle null or empty responses from MCP servers
        let tool_response: MCPToolCallResponse = if result.is_null() {
            MCPToolCallResponse::default()
        } else {
            serde_json::from_value(result)?
        };

        debug!(
            server_id = %self.config.id,
            tool_name = %name,
            is_error = ?tool_response.is_error,
            content_count = tool_response.content.len(),
            "MCP tool call completed"
        );

        Ok(tool_response)
    }

    /// Extracts text content from an MCP tool response
    ///
    /// Convenience method to get all text content from a tool response.
    pub fn extract_text_content(response: &MCPToolCallResponse) -> String {
        response
            .content
            .iter()
            .filter_map(|c| match c {
                MCPContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Sends a JSON-RPC request and waits for response with timeout.
    ///
    /// The read operation uses `spawn_blocking` with a timeout to prevent
    /// indefinite blocking if the MCP server doesn't respond.
    async fn send_request(&mut self, request: JsonRpcRequest) -> MCPResult<JsonRpcResponse> {
        let stdin = self.stdin.as_ref().ok_or(MCPError::ConnectionFailed {
            server: self.config.name.clone(),
            message: "Process stdin not available".to_string(),
        })?;

        let stdout_reader = self
            .stdout_reader
            .as_ref()
            .ok_or(MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: "Process stdout not available".to_string(),
            })?
            .clone();

        // Serialize request
        let mut request_json = serde_json::to_string(&request)?;
        request_json.push('\n');

        debug!(
            server_id = %self.config.id,
            method = %request.method,
            id = ?request.id,
            "Sending JSON-RPC request"
        );

        // Write request to stdin
        {
            let mut stdin_guard = stdin.lock().await;
            stdin_guard
                .write_all(request_json.as_bytes())
                .map_err(|e| MCPError::IoError {
                    context: "writing request".to_string(),
                    message: e.to_string(),
                })?;
            stdin_guard.flush().map_err(|e| MCPError::IoError {
                context: "flushing stdin".to_string(),
                message: e.to_string(),
            })?;
        }

        // Read response from stdout with timeout
        // We use spawn_blocking because read_line is a blocking I/O operation
        let server_name = self.config.name.clone();
        let server_id = self.config.id.clone();
        let timeout_duration = Duration::from_millis(DEFAULT_TIMEOUT_MS);

        let read_result = tokio::time::timeout(timeout_duration, async {
            tokio::task::spawn_blocking(move || {
                let mut stdout_guard = stdout_reader
                    .lock()
                    .map_err(|e| MCPError::IoError {
                        context: "locking stdout".to_string(),
                        message: e.to_string(),
                    })?;
                let mut response_line = String::new();
                stdout_guard
                    .read_line(&mut response_line)
                    .map_err(|e| MCPError::IoError {
                        context: "reading response".to_string(),
                        message: e.to_string(),
                    })?;
                Ok::<String, MCPError>(response_line)
            })
            .await
            .map_err(|e| MCPError::IoError {
                context: "spawn_blocking join".to_string(),
                message: e.to_string(),
            })?
        })
        .await;

        let response_line = match read_result {
            Ok(inner_result) => inner_result?,
            Err(_) => {
                warn!(
                    server_id = %server_id,
                    timeout_ms = DEFAULT_TIMEOUT_MS,
                    "MCP request timed out"
                );
                return Err(MCPError::Timeout {
                    operation: format!("waiting for response from server '{}'", server_name),
                    timeout_ms: DEFAULT_TIMEOUT_MS,
                });
            }
        };

        if response_line.is_empty() {
            return Err(MCPError::ConnectionFailed {
                server: self.config.name.clone(),
                message: "Server closed connection".to_string(),
            });
        }

        debug!(
            server_id = %self.config.id,
            response_bytes = response_line.len(),
            "Received JSON-RPC response"
        );

        let response: JsonRpcResponse = serde_json::from_str(&response_line)?;
        Ok(response)
    }

    /// Sends a JSON-RPC notification (no response expected)
    async fn send_notification(&mut self, notification: JsonRpcRequest) -> MCPResult<()> {
        let stdin = self.stdin.as_ref().ok_or(MCPError::ConnectionFailed {
            server: self.config.name.clone(),
            message: "Process stdin not available".to_string(),
        })?;

        let mut request_json = serde_json::to_string(&notification)?;
        request_json.push('\n');

        debug!(
            server_id = %self.config.id,
            method = %notification.method,
            "Sending JSON-RPC notification"
        );

        let mut stdin_guard = stdin.lock().await;
        stdin_guard
            .write_all(request_json.as_bytes())
            .map_err(|e| MCPError::IoError {
                context: "writing notification".to_string(),
                message: e.to_string(),
            })?;
        stdin_guard.flush().map_err(|e| MCPError::IoError {
            context: "flushing stdin".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    /// Gets the next request ID
    fn next_request_id(&self) -> i64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Kills the server process
    ///
    /// Terminates the process and cleans up resources.
    pub async fn kill(&mut self) -> MCPResult<()> {
        info!(
            server_id = %self.config.id,
            "Stopping MCP server process"
        );

        // Drop stdin/stdout to signal EOF
        self.stdin.take();
        self.stdout_reader.take();

        // Kill the process if still running
        if let Some(mut child) = self.child.take() {
            match child.kill() {
                Ok(_) => {
                    // Wait for process to exit
                    let _ = child.wait();
                    info!(
                        server_id = %self.config.id,
                        "MCP server process terminated"
                    );
                }
                Err(e) => {
                    // Process might have already exited
                    warn!(
                        server_id = %self.config.id,
                        error = %e,
                        "Failed to kill MCP server process (may have already exited)"
                    );
                }
            }
        }

        self.status = MCPServerStatus::Stopped;
        self.tools.clear();
        self.resources.clear();

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

    /// Returns the server info (name, version) if initialized
    pub fn server_info(&self) -> Option<(&str, &str)> {
        self.server_info
            .as_ref()
            .map(|(n, v)| (n.as_str(), v.as_str()))
    }

    /// Checks if the server process is still running
    pub fn is_process_alive(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    self.status = MCPServerStatus::Disconnected;
                    false
                }
                Ok(None) => true, // Still running
                Err(_) => {
                    self.status = MCPServerStatus::Error;
                    false
                }
            }
        } else {
            false
        }
    }

    /// Sets the server status to error with logging
    pub fn set_error_status(&mut self, message: &str) {
        error!(
            server_id = %self.config.id,
            error = %message,
            "MCP server entered error state"
        );
        self.status = MCPServerStatus::Error;
    }
}

impl Drop for MCPServerHandle {
    fn drop(&mut self) {
        // Ensure process is killed when handle is dropped
        self.stdin.take();
        self.stdout_reader.take();

        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_config() -> MCPServerConfig {
        MCPServerConfig {
            id: "test_server".to_string(),
            name: "Test Server".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec![
                "run".to_string(),
                "-i".to_string(),
                "--rm".to_string(),
                "test:latest".to_string(),
            ],
            env: HashMap::new(),
            description: Some("Test server for unit tests".to_string()),
        }
    }

    #[test]
    fn test_build_command_docker() {
        let config = create_test_config();
        let (cmd, args) = MCPServerHandle::build_command(&config).unwrap();

        assert_eq!(cmd, "docker");
        assert_eq!(args, vec!["run", "-i", "--rm", "test:latest"]);
    }

    #[test]
    fn test_build_command_npx() {
        let mut config = create_test_config();
        config.command = MCPDeploymentMethod::Npx;
        config.args = vec!["-y".to_string(), "@test/mcp".to_string()];

        let (cmd, args) = MCPServerHandle::build_command(&config).unwrap();

        assert_eq!(cmd, "npx");
        assert_eq!(args, vec!["-y", "@test/mcp"]);
    }

    #[test]
    fn test_build_command_uvx() {
        let mut config = create_test_config();
        config.command = MCPDeploymentMethod::Uvx;
        config.args = vec!["test-package".to_string()];

        let (cmd, args) = MCPServerHandle::build_command(&config).unwrap();

        assert_eq!(cmd, "uvx");
        assert_eq!(args, vec!["test-package"]);
    }

    #[test]
    fn test_build_command_empty_args_fails() {
        let mut config = create_test_config();
        config.args = vec![];

        let result = MCPServerHandle::build_command(&config);
        assert!(result.is_err());

        match result {
            Err(MCPError::InvalidConfig { field, reason: _ }) => {
                assert_eq!(field, "args");
            }
            _ => panic!("Expected InvalidConfig error"),
        }
    }

    #[test]
    fn test_extract_text_content() {
        let response = MCPToolCallResponse {
            content: vec![
                MCPContent::Text {
                    text: "First line".to_string(),
                },
                MCPContent::Text {
                    text: "Second line".to_string(),
                },
            ],
            is_error: None,
        };

        let text = MCPServerHandle::extract_text_content(&response);
        assert_eq!(text, "First line\nSecond line");
    }

    #[test]
    fn test_extract_text_content_filters_non_text() {
        let response = MCPToolCallResponse {
            content: vec![
                MCPContent::Text {
                    text: "Text content".to_string(),
                },
                MCPContent::Image {
                    data: "base64data".to_string(),
                    mime_type: "image/png".to_string(),
                },
            ],
            is_error: None,
        };

        let text = MCPServerHandle::extract_text_content(&response);
        assert_eq!(text, "Text content");
    }

    #[test]
    fn test_next_request_id_increments() {
        let config = create_test_config();
        let handle = MCPServerHandle {
            config,
            child: None,
            stdin: None,
            stdout_reader: None,
            status: MCPServerStatus::Stopped,
            tools: Vec::new(),
            resources: Vec::new(),
            request_id: AtomicI64::new(1),
            server_info: None,
        };

        assert_eq!(handle.next_request_id(), 1);
        assert_eq!(handle.next_request_id(), 2);
        assert_eq!(handle.next_request_id(), 3);
    }
}
