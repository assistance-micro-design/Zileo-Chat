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

//! MCP (Model Context Protocol) data models
//!
//! This module defines the data structures for MCP server configuration,
//! tool definitions, and tool call results. These types are shared between
//! the Rust backend and TypeScript frontend via Tauri IPC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Deployment method for MCP servers
///
/// Determines how the MCP server process is spawned:
/// - `Docker`: Runs in a Docker container (recommended for production)
/// - `Npx`: Runs via npx (Node.js package executor)
/// - `Uvx`: Runs via uvx (Python package executor with isolated environments)
/// - `Http`: Connects to a remote HTTP/SSE endpoint (SaaS, remote servers)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MCPDeploymentMethod {
    /// Docker container (e.g., `docker run -i image:tag`)
    Docker,
    /// Node.js npx (e.g., `npx -y @package/mcp`)
    Npx,
    /// Python uvx (e.g., `uvx package-name`)
    Uvx,
    /// Remote HTTP/SSE endpoint (e.g., `https://api.example.com/mcp`)
    Http,
}

impl std::fmt::Display for MCPDeploymentMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPDeploymentMethod::Docker => write!(f, "docker"),
            MCPDeploymentMethod::Npx => write!(f, "npx"),
            MCPDeploymentMethod::Uvx => write!(f, "uvx"),
            MCPDeploymentMethod::Http => write!(f, "http"),
        }
    }
}

/// HTTP authentication method for remote MCP servers (v1.2).
///
/// Only applicable when `command == MCPDeploymentMethod::Http`.
/// `None` means no authentication header is generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MCPAuthType {
    /// No HTTP authentication
    #[default]
    None,
    /// Authorization: Bearer <token>
    Bearer,
    /// Custom header (default `X-API-Key`) with secret value
    Apikey,
    /// Authorization: Basic base64(user:pass)
    Basic,
}

/// Non-sensitive auth metadata persisted alongside the server config (v1.2).
///
/// Secrets (token / value / password) are stored separately in the OS
/// keychain via [`MCPAuthSecret`]; this struct never carries secret data.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPAuthMetadata {
    /// Header name for `Apikey` mode (default `X-API-Key` if absent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub header_name: Option<String>,
    /// Username for `Basic` mode (the password is a secret).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
}

/// Auth secret payload — token / value / password depending on `MCPAuthType` (v1.2).
///
/// Sent only on create/update by the frontend, persisted in the OS keychain,
/// and reloaded by the HTTP transport at connect time. Never returned by
/// read commands.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPAuthSecret {
    /// Bearer token (when `auth_type == Bearer`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// API key value (when `auth_type == Apikey`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Basic auth password (when `auth_type == Basic`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// MCP server configuration
///
/// Contains all settings needed to spawn and connect to an MCP server.
/// This is the input type for creating/updating server configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerConfig {
    /// Technical ID (for database storage only)
    pub id: String,
    /// User-friendly name - MUST BE UNIQUE - used as identifier in MCPManager
    pub name: String,
    /// Whether the server is enabled and should start automatically
    pub enabled: bool,
    /// Deployment method (docker, npx, uvx)
    pub command: MCPDeploymentMethod,
    /// Command arguments (e.g., ["run", "-i", "image:tag"] for Docker)
    pub args: Vec<String>,
    /// Environment variables to pass to the server process
    pub env: HashMap<String, String>,
    /// Optional description of the server's purpose
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // ---- v1.2 (HTTP authentication) ----
    /// HTTP authentication type (HTTP deployment only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<MCPAuthType>,
    /// Non-sensitive auth metadata (header name, username).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_metadata: Option<MCPAuthMetadata>,
    /// Additional HTTP headers (cumulative with main auth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<HashMap<String, String>>,
}

/// Write payload for `create_mcp_server` / `update_mcp_server` (v1.2).
///
/// Carries the optional `auth_secret` next to the persisted config.
/// The command splits metadata (DB) and secret (keychain) before
/// persisting and never echoes the secret back.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerConfigWithSecret {
    /// Persisted server config (DB).
    #[serde(flatten)]
    pub config: MCPServerConfig,
    /// Optional auth secret (token / value / password).
    #[serde(default)]
    pub auth_secret: Option<MCPAuthSecret>,
}

/// Legacy HTTP auth warning surfaced at startup (v1.2).
///
/// Emitted for HTTP MCP servers still configured with the deprecated
/// env-based auth (`API_KEY` or `HEADER_*`) so the UI can offer a
/// migration banner.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyHttpAuthWarning {
    /// MCP server ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Detected legacy env var keys (`API_KEY` and/or `HEADER_*`)
    pub env_keys: Vec<String>,
}

/// MCP server status
///
/// Represents the current lifecycle state of an MCP server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MCPServerStatus {
    /// Server is not running
    #[default]
    Stopped,
    /// Server is in the process of starting
    Starting,
    /// Server is running and accepting requests
    Running,
    /// Server encountered an error
    Error,
    /// Server process is running but client is disconnected
    Disconnected,
}

impl std::fmt::Display for MCPServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPServerStatus::Stopped => write!(f, "stopped"),
            MCPServerStatus::Starting => write!(f, "starting"),
            MCPServerStatus::Running => write!(f, "running"),
            MCPServerStatus::Error => write!(f, "error"),
            MCPServerStatus::Disconnected => write!(f, "disconnected"),
        }
    }
}

/// MCP tool definition
///
/// Describes a tool exposed by an MCP server, including its name,
/// description, and JSON Schema for input parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTool {
    /// Tool name (used to invoke the tool)
    pub name: String,
    /// Human-readable description of what the tool does
    pub description: String,
    /// JSON Schema describing the tool's input parameters
    pub input_schema: serde_json::Value,
}

/// MCP resource definition
///
/// Describes a resource exposed by an MCP server.
/// Resources are read-only data sources that can be accessed by URI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResource {
    /// Resource URI (unique identifier within the server)
    pub uri: String,
    /// Human-readable resource name
    pub name: String,
    /// Optional description of the resource
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional MIME type of the resource content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Complete MCP server entity
///
/// Combines configuration with runtime state (status, discovered tools/resources).
/// This is returned from Tauri commands to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServer {
    /// Server configuration
    #[serde(flatten)]
    pub config: MCPServerConfig,
    /// Current runtime status
    pub status: MCPServerStatus,
    /// Tools discovered after initialization
    pub tools: Vec<MCPTool>,
    /// Resources discovered after initialization
    pub resources: Vec<MCPResource>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Response from create/update MCP server commands.
///
/// Wraps `MCPServer` with an optional security warning (e.g., HTTP usage).
/// Mirrors the `CustomProviderResponse` pattern from custom_provider.rs.
#[derive(Debug, Clone, Serialize)]
pub struct MCPServerResponse {
    /// Server data
    pub server: MCPServer,
    /// Optional security warning (e.g., HTTP without TLS on non-localhost)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// MCP server connection test result
///
/// Returned after testing an MCP server configuration.
/// Contains success status, discovered capabilities, and latency metrics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPTestResult {
    /// Whether the connection test succeeded
    pub success: bool,
    /// Human-readable result message
    pub message: String,
    /// Tools discovered during test
    pub tools: Vec<MCPTool>,
    /// Resources discovered during test
    pub resources: Vec<MCPResource>,
    /// Connection latency in milliseconds
    pub latency_ms: u64,
}

/// MCP tool call request
///
/// Request to execute a tool on an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolCallRequest {
    /// Name of the MCP server
    pub server_name: String,
    /// Name of the tool to invoke
    pub tool_name: String,
    /// Tool arguments (must conform to tool's input_schema)
    pub arguments: serde_json::Value,
}

/// MCP tool call result
///
/// Result of executing a tool on an MCP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolCallResult {
    /// Whether the tool call succeeded
    pub success: bool,
    /// Tool output content
    pub content: serde_json::Value,
    /// Error message if the call failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
}

/// MCP tool call log entry
///
/// Stored in the database for auditing and debugging purposes.
#[cfg(test)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPCallLog {
    /// Unique log entry ID
    pub id: String,
    /// Associated workflow ID (if called from a workflow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// MCP server name
    pub server_name: String,
    /// Tool name that was called
    pub tool_name: String,
    /// Parameters passed to the tool (stored as JSON string in DB)
    #[serde(deserialize_with = "crate::models::serde_utils::deserialize_json_string")]
    pub params: serde_json::Value,
    /// Result returned by the tool (stored as JSON string in DB)
    #[serde(deserialize_with = "crate::models::serde_utils::deserialize_json_string")]
    pub result: serde_json::Value,
    /// Whether the call succeeded
    pub success: bool,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Timestamp of the call
    pub timestamp: DateTime<Utc>,
}

/// MCP tool call log entry for database creation
///
/// This struct omits the `timestamp` field to let SurrealDB's
/// `DEFAULT time::now()` generate the timestamp server-side.
/// This avoids the DateTime<Utc> serialization issue where serde
/// produces RFC3339 strings but SurrealDB expects native datetime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPCallLogCreate {
    /// Unique log entry ID
    pub id: String,
    /// Associated workflow ID (if called from a workflow)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// MCP server name
    pub server_name: String,
    /// Tool name that was called
    pub tool_name: String,
    /// Parameters passed to the tool (serialized as JSON string for SCHEMAFULL storage)
    #[serde(serialize_with = "crate::models::serde_utils::serialize_as_json_string")]
    pub params: serde_json::Value,
    /// Result returned by the tool (serialized as JSON string for SCHEMAFULL storage)
    #[serde(serialize_with = "crate::models::serde_utils::serialize_as_json_string")]
    pub result: serde_json::Value,
    /// Whether the call succeeded
    pub success: bool,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    // NOTE: `timestamp` field omitted - SurrealDB generates via DEFAULT time::now()
}

/// MCP server database record
///
/// Used for database persistence. Converts command enum to string
/// for SurrealDB compatibility.
///
/// IMPORTANT: `env`, `auth_metadata` and `extra_headers` are stored as
/// JSON strings (not objects) to work around SurrealDB SCHEMAFULL filtering
/// of nested object fields (ERR_SURREAL_001). The strings are deserialized
/// back to typed structs when reading from the database.
#[derive(Debug, Clone, Serialize)]
pub struct MCPServerCreate {
    /// Server name
    pub name: String,
    /// Whether the server is enabled
    pub enabled: bool,
    /// Deployment method (as string for SurrealDB)
    pub command: String,
    /// Command arguments
    pub args: Vec<String>,
    /// Environment variables as JSON string (to bypass SurrealDB SCHEMAFULL filtering)
    pub env: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    // ---- v1.2 (HTTP authentication) ----
    /// Authentication type as serialized string (`none` / `bearer` / `apikey` / `basic`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<String>,
    /// Auth metadata serialized as JSON string (or `None` when no auth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_metadata: Option<String>,
    /// Extra headers serialized as JSON string (or `None` when empty).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_headers: Option<String>,
}

impl MCPServerCreate {
    /// Creates a new MCPServerCreate from MCPServerConfig
    pub fn from_config(config: &MCPServerConfig) -> Self {
        let auth_type = config
            .auth_type
            .filter(|t| *t != MCPAuthType::None)
            .and_then(|t| serde_json::to_value(t).ok())
            .and_then(|v| v.as_str().map(String::from));

        let auth_metadata = config
            .auth_metadata
            .as_ref()
            .filter(|m| m.header_name.is_some() || m.username.is_some())
            .and_then(|m| serde_json::to_string(m).ok());

        let extra_headers = config
            .extra_headers
            .as_ref()
            .filter(|h| !h.is_empty())
            .and_then(|h| serde_json::to_string(h).ok());

        Self {
            name: config.name.clone(),
            enabled: config.enabled,
            command: config.command.to_string(),
            args: config.args.clone(),
            // Serialize env HashMap to JSON string to bypass SurrealDB SCHEMAFULL filtering
            env: serde_json::to_string(&config.env).unwrap_or_else(|_| "{}".to_string()),
            description: config.description.clone(),
            auth_type,
            auth_metadata,
            extra_headers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deployment_method_serialization() {
        let docker = MCPDeploymentMethod::Docker;
        let json = serde_json::to_string(&docker).unwrap();
        assert_eq!(json, "\"docker\"");

        let npx = MCPDeploymentMethod::Npx;
        let json = serde_json::to_string(&npx).unwrap();
        assert_eq!(json, "\"npx\"");

        let uvx = MCPDeploymentMethod::Uvx;
        let json = serde_json::to_string(&uvx).unwrap();
        assert_eq!(json, "\"uvx\"");
    }

    #[test]
    fn test_deployment_method_deserialization() {
        let docker: MCPDeploymentMethod = serde_json::from_str("\"docker\"").unwrap();
        assert_eq!(docker, MCPDeploymentMethod::Docker);

        let npx: MCPDeploymentMethod = serde_json::from_str("\"npx\"").unwrap();
        assert_eq!(npx, MCPDeploymentMethod::Npx);

        let uvx: MCPDeploymentMethod = serde_json::from_str("\"uvx\"").unwrap();
        assert_eq!(uvx, MCPDeploymentMethod::Uvx);
    }

    #[test]
    fn test_server_status_serialization() {
        let running = MCPServerStatus::Running;
        let json = serde_json::to_string(&running).unwrap();
        assert_eq!(json, "\"running\"");

        let stopped = MCPServerStatus::Stopped;
        let json = serde_json::to_string(&stopped).unwrap();
        assert_eq!(json, "\"stopped\"");
    }

    #[test]
    fn test_server_config_serialization() {
        let config = MCPServerConfig {
            id: "serena".to_string(),
            name: "Serena".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec![
                "run".to_string(),
                "-i".to_string(),
                "serena:latest".to_string(),
            ],
            env: HashMap::from([("DEBUG".to_string(), "true".to_string())]),
            description: Some("Code analysis server".to_string()),
            auth_type: None,
            auth_metadata: None,
            extra_headers: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: MCPServerConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, config.id);
        assert_eq!(deserialized.name, config.name);
        assert_eq!(deserialized.enabled, config.enabled);
        assert_eq!(deserialized.command, config.command);
        assert_eq!(deserialized.args, config.args);
        assert_eq!(deserialized.description, config.description);
    }

    #[test]
    fn test_mcp_tool_serialization() {
        let tool = MCPTool {
            name: "find_symbol".to_string(),
            description: "Find a symbol in the codebase".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {"type": "string"},
                    "path": {"type": "string"}
                },
                "required": ["name"]
            }),
        };

        let json = serde_json::to_string(&tool).unwrap();
        let deserialized: MCPTool = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.name, tool.name);
        assert_eq!(deserialized.description, tool.description);
    }

    #[test]
    fn test_mcp_resource_serialization() {
        let resource = MCPResource {
            uri: "file:///path/to/file.rs".to_string(),
            name: "file.rs".to_string(),
            description: Some("A Rust source file".to_string()),
            mime_type: Some("text/x-rust".to_string()),
        };

        let json = serde_json::to_string(&resource).unwrap();
        let deserialized: MCPResource = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.uri, resource.uri);
        assert_eq!(deserialized.name, resource.name);
        assert_eq!(deserialized.description, resource.description);
        assert_eq!(deserialized.mime_type, resource.mime_type);
    }

    #[test]
    fn test_tool_call_request_serialization() {
        let request = MCPToolCallRequest {
            server_name: "serena".to_string(),
            tool_name: "find_symbol".to_string(),
            arguments: serde_json::json!({"name": "MyClass"}),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: MCPToolCallRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.server_name, request.server_name);
        assert_eq!(deserialized.tool_name, request.tool_name);
    }

    #[test]
    fn test_tool_call_result_serialization() {
        let result = MCPToolCallResult {
            success: true,
            content: serde_json::json!({"found": true, "location": "src/main.rs:42"}),
            error: None,
            duration_ms: 150,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: MCPToolCallResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.success, result.success);
        assert_eq!(deserialized.duration_ms, result.duration_ms);
        assert!(deserialized.error.is_none());
    }

    #[test]
    fn test_tool_call_result_with_error() {
        let result = MCPToolCallResult {
            success: false,
            content: serde_json::Value::Null,
            error: Some("Tool execution failed: timeout".to_string()),
            duration_ms: 30000,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"error\":\"Tool execution failed: timeout\""));

        let deserialized: MCPToolCallResult = serde_json::from_str(&json).unwrap();
        assert!(!deserialized.success);
        assert!(deserialized.error.is_some());
    }

    #[test]
    fn test_test_result_serialization() {
        let result = MCPTestResult {
            success: true,
            message: "Connection successful".to_string(),
            tools: vec![MCPTool {
                name: "test_tool".to_string(),
                description: "A test tool".to_string(),
                input_schema: serde_json::json!({}),
            }],
            resources: vec![],
            latency_ms: 50,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: MCPTestResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.success, result.success);
        assert_eq!(deserialized.message, result.message);
        assert_eq!(deserialized.tools.len(), 1);
        assert_eq!(deserialized.latency_ms, 50);
    }

    #[test]
    fn test_mcp_server_create_from_config() {
        let config = MCPServerConfig {
            id: "test".to_string(),
            name: "Test Server".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Npx,
            args: vec!["-y".to_string(), "@test/mcp".to_string()],
            env: HashMap::from([("API_KEY".to_string(), "secret".to_string())]),
            description: None,
            auth_type: None,
            auth_metadata: None,
            extra_headers: None,
        };

        let create = MCPServerCreate::from_config(&config);

        assert_eq!(create.name, "Test Server");
        assert_eq!(create.command, "npx");
        assert!(create.enabled);
        assert!(create.description.is_none());
        assert!(create.auth_type.is_none());
        assert!(create.auth_metadata.is_none());
        assert!(create.extra_headers.is_none());
    }

    #[test]
    fn test_auth_type_serialization() {
        assert_eq!(
            serde_json::to_string(&MCPAuthType::None).unwrap(),
            "\"none\""
        );
        assert_eq!(
            serde_json::to_string(&MCPAuthType::Bearer).unwrap(),
            "\"bearer\""
        );
        assert_eq!(
            serde_json::to_string(&MCPAuthType::Apikey).unwrap(),
            "\"apikey\""
        );
        assert_eq!(
            serde_json::to_string(&MCPAuthType::Basic).unwrap(),
            "\"basic\""
        );

        let bearer: MCPAuthType = serde_json::from_str("\"bearer\"").unwrap();
        assert_eq!(bearer, MCPAuthType::Bearer);
        let apikey: MCPAuthType = serde_json::from_str("\"apikey\"").unwrap();
        assert_eq!(apikey, MCPAuthType::Apikey);
    }

    #[test]
    fn test_auth_metadata_camelcase_serialization() {
        let metadata = MCPAuthMetadata {
            header_name: Some("X-API-Key".to_string()),
            username: Some("alice".to_string()),
        };
        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("\"headerName\":\"X-API-Key\""));
        assert!(json.contains("\"username\":\"alice\""));

        // Round-trip
        let parsed: MCPAuthMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.header_name.as_deref(), Some("X-API-Key"));
        assert_eq!(parsed.username.as_deref(), Some("alice"));
    }

    #[test]
    fn test_auth_metadata_skip_none() {
        let metadata = MCPAuthMetadata::default();
        let json = serde_json::to_string(&metadata).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn test_auth_secret_camelcase_serialization() {
        let secret = MCPAuthSecret {
            token: Some("sk-abc".to_string()),
            value: None,
            password: None,
        };
        let json = serde_json::to_string(&secret).unwrap();
        assert!(json.contains("\"token\":\"sk-abc\""));
        assert!(!json.contains("value"));
        assert!(!json.contains("password"));

        // Round-trip with empty default
        let empty: MCPAuthSecret = serde_json::from_str("{}").unwrap();
        assert!(empty.token.is_none());
        assert!(empty.value.is_none());
        assert!(empty.password.is_none());
    }

    #[test]
    fn test_server_config_with_auth_serialization() {
        let config = MCPServerConfig {
            id: "srv1".to_string(),
            name: "Remote".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Http,
            args: vec!["https://api.example.com/mcp".to_string()],
            env: HashMap::new(),
            description: None,
            auth_type: Some(MCPAuthType::Bearer),
            auth_metadata: Some(MCPAuthMetadata {
                header_name: None,
                username: None,
            }),
            extra_headers: Some(HashMap::from([("X-Tenant".to_string(), "42".to_string())])),
        };

        let json = serde_json::to_string(&config).unwrap();
        // camelCase via rename_all
        assert!(json.contains("\"authType\":\"bearer\""));
        assert!(json.contains("\"authMetadata\""));
        assert!(json.contains("\"extraHeaders\""));

        // Round-trip
        let parsed: MCPServerConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.auth_type, Some(MCPAuthType::Bearer));
        assert!(parsed.auth_metadata.is_some());
        assert_eq!(
            parsed
                .extra_headers
                .as_ref()
                .and_then(|h| h.get("X-Tenant"))
                .map(String::as_str),
            Some("42")
        );
    }

    #[test]
    fn test_server_config_without_auth_omits_fields() {
        let config = MCPServerConfig {
            id: "srv1".to_string(),
            name: "Stdio".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Docker,
            args: vec!["run".to_string()],
            env: HashMap::new(),
            description: None,
            auth_type: None,
            auth_metadata: None,
            extra_headers: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("authType"));
        assert!(!json.contains("authMetadata"));
        assert!(!json.contains("extraHeaders"));
    }

    #[test]
    fn test_server_config_with_secret_flatten_deserialization() {
        let payload = r#"{
            "id": "srv1",
            "name": "Remote",
            "enabled": true,
            "command": "http",
            "args": ["https://api.example.com"],
            "env": {},
            "authType": "bearer",
            "authSecret": { "token": "sk-secret" }
        }"#;

        let parsed: MCPServerConfigWithSecret = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed.config.id, "srv1");
        assert_eq!(parsed.config.auth_type, Some(MCPAuthType::Bearer));
        assert_eq!(
            parsed.auth_secret.as_ref().and_then(|s| s.token.as_deref()),
            Some("sk-secret")
        );
    }

    #[test]
    fn test_server_create_from_config_with_auth() {
        let config = MCPServerConfig {
            id: "srv1".to_string(),
            name: "Remote".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Http,
            args: vec!["https://api.example.com/mcp".to_string()],
            env: HashMap::new(),
            description: None,
            auth_type: Some(MCPAuthType::Apikey),
            auth_metadata: Some(MCPAuthMetadata {
                header_name: Some("X-Custom-Key".to_string()),
                username: None,
            }),
            extra_headers: Some(HashMap::from([("X-Tenant".to_string(), "42".to_string())])),
        };

        let create = MCPServerCreate::from_config(&config);
        assert_eq!(create.auth_type.as_deref(), Some("apikey"));

        let metadata_str = create.auth_metadata.expect("metadata json");
        assert!(metadata_str.contains("\"headerName\":\"X-Custom-Key\""));

        let headers_str = create.extra_headers.expect("headers json");
        assert!(headers_str.contains("X-Tenant"));
    }

    #[test]
    fn test_server_create_from_config_with_none_auth_skips_fields() {
        let config = MCPServerConfig {
            id: "srv1".to_string(),
            name: "Remote".to_string(),
            enabled: true,
            command: MCPDeploymentMethod::Http,
            args: vec!["https://api.example.com/mcp".to_string()],
            env: HashMap::new(),
            description: None,
            auth_type: Some(MCPAuthType::None),
            auth_metadata: None,
            extra_headers: None,
        };

        let create = MCPServerCreate::from_config(&config);
        // None must be persisted as absent (not as "none" string) to keep DB clean
        assert!(create.auth_type.is_none());
        assert!(create.auth_metadata.is_none());
        assert!(create.extra_headers.is_none());
    }

    #[test]
    fn test_legacy_http_auth_warning_serialization() {
        let warning = LegacyHttpAuthWarning {
            id: "abc-123".to_string(),
            name: "Remote".to_string(),
            env_keys: vec!["API_KEY".to_string(), "HEADER_X-Trace".to_string()],
        };
        let json = serde_json::to_string(&warning).unwrap();
        assert!(json.contains("\"envKeys\""));
        assert!(json.contains("\"id\":\"abc-123\""));
    }
}

/// MCP latency percentile metrics
///
/// Provides performance statistics for MCP server tool calls,
/// including p50, p95, and p99 latency percentiles.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPLatencyMetrics {
    /// MCP server name
    pub server_name: String,
    /// 50th percentile latency in milliseconds (median)
    pub p50_ms: f64,
    /// 95th percentile latency in milliseconds
    pub p95_ms: f64,
    /// 99th percentile latency in milliseconds
    pub p99_ms: f64,
    /// Total number of tool calls in the time window
    pub total_calls: i64,
}
