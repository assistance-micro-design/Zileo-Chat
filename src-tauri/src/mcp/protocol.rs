// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! JSON-RPC 2.0 and MCP Protocol Types
//!
//! This module implements the JSON-RPC 2.0 specification for MCP communication.
//! Reference: https://modelcontextprotocol.io/specification/2025-06-18

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP Protocol version supported by this implementation
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// Application name used in MCP client info
pub const MCP_CLIENT_NAME: &str = "Zileo-Chat-3";

/// Application version used in MCP client info
pub const MCP_CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");

// =============================================================================
// JSON-RPC 2.0 Core Types
// =============================================================================

/// JSON-RPC request ID
///
/// Can be a number, string, or null according to the JSON-RPC 2.0 spec.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    /// Numeric ID
    Number(i64),
    /// String ID
    String(String),
    /// Null ID (for notifications)
    #[default]
    Null,
}

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Method name to invoke
    pub method: String,
    /// Optional method parameters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    /// Request ID for correlation
    pub id: JsonRpcId,
}

impl JsonRpcRequest {
    /// Creates a new JSON-RPC request with a numeric ID
    pub fn new(method: &str, params: Option<Value>, id: i64) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: JsonRpcId::Number(id),
        }
    }

    /// Creates a new JSON-RPC notification (no ID, no response expected)
    pub fn notification(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: JsonRpcId::Null,
        }
    }
}

/// JSON-RPC 2.0 Response
///
/// Note: In JSON-RPC 2.0, notifications (server-to-client messages) may not have an `id` field.
/// We make it optional with a default value to handle such cases gracefully.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// JSON-RPC version (always "2.0")
    pub jsonrpc: String,
    /// Result on success
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error on failure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    /// Request ID for correlation (optional for notifications)
    #[serde(default)]
    pub id: Option<JsonRpcId>,
}

impl JsonRpcResponse {
    /// Checks if the response is an error
    pub fn is_error(&self) -> bool {
        self.error.is_some()
    }

    /// Extracts the result value, returning an error if present
    pub fn into_result(self) -> Result<Value, JsonRpcError> {
        if let Some(err) = self.error {
            Err(err)
        } else {
            Ok(self.result.unwrap_or(Value::Null))
        }
    }
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Error code
    pub code: i32,
    /// Human-readable error message
    pub message: String,
    /// Optional additional error data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC error codes
impl JsonRpcError {
    /// Parse error (-32700)
    pub fn parse_error(message: &str) -> Self {
        Self {
            code: -32700,
            message: message.to_string(),
            data: None,
        }
    }

    /// Invalid Request (-32600)
    pub fn invalid_request(message: &str) -> Self {
        Self {
            code: -32600,
            message: message.to_string(),
            data: None,
        }
    }

    /// Method not found (-32601)
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("Method '{}' not found", method),
            data: None,
        }
    }

    /// Invalid params (-32602)
    pub fn invalid_params(message: &str) -> Self {
        Self {
            code: -32602,
            message: message.to_string(),
            data: None,
        }
    }

    /// Internal error (-32603)
    pub fn internal_error(message: &str) -> Self {
        Self {
            code: -32603,
            message: message.to_string(),
            data: None,
        }
    }
}

// =============================================================================
// MCP Initialize Types
// =============================================================================

/// MCP Initialize request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPInitializeParams {
    /// Protocol version the client supports
    pub protocol_version: String,
    /// Client capabilities
    pub capabilities: MCPClientCapabilities,
    /// Client information
    pub client_info: MCPClientInfo,
}

impl Default for MCPInitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: MCPClientCapabilities::default(),
            client_info: MCPClientInfo::default(),
        }
    }
}

/// MCP Client capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MCPClientCapabilities {
    /// Roots capability (file system access)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub roots: Option<RootsCapability>,
    /// Sampling capability (LLM sampling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sampling: Option<SamplingCapability>,
}

/// Roots capability configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootsCapability {
    /// Whether the client supports listing roots
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Sampling capability configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SamplingCapability {}

/// Client information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPClientInfo {
    /// Client application name
    pub name: String,
    /// Client application version
    pub version: String,
}

impl Default for MCPClientInfo {
    fn default() -> Self {
        Self {
            name: MCP_CLIENT_NAME.to_string(),
            version: MCP_CLIENT_VERSION.to_string(),
        }
    }
}

/// MCP Initialize response result
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPInitializeResult {
    /// Protocol version the server supports
    pub protocol_version: String,
    /// Server capabilities
    pub capabilities: MCPServerCapabilities,
    /// Server information
    pub server_info: MCPServerInfo,
}

/// MCP Server capabilities
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MCPServerCapabilities {
    /// Tools capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolsCapability>,
    /// Resources capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourcesCapability>,
    /// Prompts capability
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompts: Option<PromptsCapability>,
}

/// Tools capability configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsCapability {
    /// Whether the server supports tool list changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Resources capability configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesCapability {
    /// Whether the server supports resource subscriptions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<bool>,
    /// Whether the server supports resource list changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Prompts capability configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptsCapability {
    /// Whether the server supports prompt list changes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_changed: Option<bool>,
}

/// Server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
}

// =============================================================================
// MCP Tools Types
// =============================================================================

/// MCP Tool definition from server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    #[serde(default)]
    pub description: String,
    /// Input JSON Schema
    pub input_schema: Value,
}

/// MCP tools/list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolsListResult {
    /// List of available tools
    pub tools: Vec<MCPToolDefinition>,
}

/// MCP tools/call request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPToolCallParams {
    /// Tool name to invoke
    pub name: String,
    /// Tool arguments
    #[serde(default)]
    pub arguments: Value,
}

/// MCP tools/call response
///
/// Note: Some MCP servers may return empty or null responses. We handle this by
/// making content optional with a default empty vector.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPToolCallResponse {
    /// Response content (defaults to empty if not provided)
    #[serde(default)]
    pub content: Vec<MCPContent>,
    /// Whether the response indicates an error
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// MCP Content types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum MCPContent {
    /// Text content
    Text {
        /// Text value
        text: String,
    },
    /// Image content (base64 encoded)
    Image {
        /// Base64-encoded image data
        data: String,
        /// MIME type of the image
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Resource reference
    Resource {
        /// Resource content
        resource: MCPResourceContent,
    },
}

/// MCP Resource content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPResourceContent {
    /// Resource URI
    pub uri: String,
    /// Resource MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Resource text content (if text-based)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Resource binary content (base64, if binary)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

// =============================================================================
// MCP Resources Types
// =============================================================================

/// MCP Resource definition from server
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPResourceDefinition {
    /// Resource URI
    pub uri: String,
    /// Resource name
    pub name: String,
    /// Resource description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Resource MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// MCP resources/list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MCPResourcesListResult {
    /// List of available resources
    pub resources: Vec<MCPResourceDefinition>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest::new("tools/list", None, 1);
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"tools/list\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_json_rpc_request_with_params() {
        let params = serde_json::json!({"name": "test_tool"});
        let request = JsonRpcRequest::new("tools/call", Some(params), 42);
        let json = serde_json::to_string(&request).unwrap();

        assert!(json.contains("\"params\":{\"name\":\"test_tool\"}"));
        assert!(json.contains("\"id\":42"));
    }

    #[test]
    fn test_json_rpc_notification() {
        let notification = JsonRpcRequest::notification("notifications/initialized", None);
        let json = serde_json::to_string(&notification).unwrap();

        assert!(json.contains("\"id\":null"));
    }

    #[test]
    fn test_json_rpc_response_success() {
        let json = r#"{
            "jsonrpc": "2.0",
            "result": {"tools": []},
            "id": 1
        }"#;

        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(!response.is_error());
        assert!(response.result.is_some());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let json = r#"{
            "jsonrpc": "2.0",
            "error": {"code": -32601, "message": "Method not found"},
            "id": 1
        }"#;

        let response: JsonRpcResponse = serde_json::from_str(json).unwrap();
        assert!(response.is_error());
        assert_eq!(response.error.unwrap().code, -32601);
    }

    #[test]
    fn test_json_rpc_error_constructors() {
        let parse_err = JsonRpcError::parse_error("Invalid JSON");
        assert_eq!(parse_err.code, -32700);

        let method_err = JsonRpcError::method_not_found("unknown");
        assert_eq!(method_err.code, -32601);
        assert!(method_err.message.contains("unknown"));

        let internal_err = JsonRpcError::internal_error("Something went wrong");
        assert_eq!(internal_err.code, -32603);
    }

    #[test]
    fn test_mcp_initialize_params_default() {
        let params = MCPInitializeParams::default();
        let json = serde_json::to_string(&params).unwrap();

        assert!(json.contains(MCP_PROTOCOL_VERSION));
        assert!(json.contains(MCP_CLIENT_NAME));
    }

    #[test]
    fn test_mcp_initialize_result_deserialization() {
        let json = r#"{
            "protocolVersion": "2025-06-18",
            "capabilities": {
                "tools": {"listChanged": true},
                "resources": {"subscribe": false}
            },
            "serverInfo": {
                "name": "test-server",
                "version": "1.0.0"
            }
        }"#;

        let result: MCPInitializeResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.protocol_version, "2025-06-18");
        assert_eq!(result.server_info.name, "test-server");
        assert!(result.capabilities.tools.is_some());
    }

    #[test]
    fn test_mcp_tool_definition_deserialization() {
        let json = r#"{
            "name": "find_symbol",
            "description": "Find a symbol in the codebase",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": {"type": "string"}
                }
            }
        }"#;

        let tool: MCPToolDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "find_symbol");
        assert!(!tool.description.is_empty());
    }

    #[test]
    fn test_mcp_tools_list_result() {
        let json = r#"{
            "tools": [
                {
                    "name": "tool1",
                    "description": "First tool",
                    "inputSchema": {}
                },
                {
                    "name": "tool2",
                    "description": "Second tool",
                    "inputSchema": {}
                }
            ]
        }"#;

        let result: MCPToolsListResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.tools.len(), 2);
    }

    #[test]
    fn test_mcp_tool_call_params_serialization() {
        let params = MCPToolCallParams {
            name: "find_symbol".to_string(),
            arguments: serde_json::json!({"name": "MyClass"}),
        };

        let json = serde_json::to_string(&params).unwrap();
        assert!(json.contains("\"name\":\"find_symbol\""));
        assert!(json.contains("\"arguments\""));
    }

    #[test]
    fn test_mcp_content_text() {
        let content = MCPContent::Text {
            text: "Hello, world!".to_string(),
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Hello, world!\""));
    }

    #[test]
    fn test_mcp_content_image() {
        let content = MCPContent::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };

        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"mimeType\":\"image/png\""));
    }

    #[test]
    fn test_mcp_tool_call_response() {
        let json = r#"{
            "content": [
                {"type": "text", "text": "Result text"}
            ],
            "isError": false
        }"#;

        let response: MCPToolCallResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.content.len(), 1);
        assert_eq!(response.is_error, Some(false));
    }

    #[test]
    fn test_mcp_resource_definition() {
        let json = r#"{
            "uri": "file:///path/to/file",
            "name": "file.rs",
            "description": "A Rust file",
            "mimeType": "text/x-rust"
        }"#;

        let resource: MCPResourceDefinition = serde_json::from_str(json).unwrap();
        assert_eq!(resource.uri, "file:///path/to/file");
        assert_eq!(resource.name, "file.rs");
        assert!(resource.description.is_some());
        assert!(resource.mime_type.is_some());
    }

    #[test]
    fn test_json_rpc_id_variants() {
        // Number ID
        let id_num: JsonRpcId = serde_json::from_str("42").unwrap();
        assert_eq!(id_num, JsonRpcId::Number(42));

        // String ID
        let id_str: JsonRpcId = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(id_str, JsonRpcId::String("abc".to_string()));

        // Null ID
        let id_null: JsonRpcId = serde_json::from_str("null").unwrap();
        assert_eq!(id_null, JsonRpcId::Null);
    }
}
