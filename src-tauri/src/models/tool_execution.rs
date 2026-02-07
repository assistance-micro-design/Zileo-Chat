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

//! Tool execution models for persistence.
//!
//! This module provides types for storing and retrieving tool execution logs
//! for both local tools (MemoryTool, TodoTool) and MCP tools.
//!
//! Phase 3: Tool Execution Persistence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Deserialize a JSON string from DB back into serde_json::Value.
/// Handles both string (new format) and object (legacy format) inputs.
fn deserialize_json_string<'de, D>(deserializer: D) -> Result<serde_json::Value, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => serde_json::from_str(&s).map_err(serde::de::Error::custom),
        // Legacy: if DB still has object type data, pass through as-is
        other => Ok(other),
    }
}

/// Serialize serde_json::Value to a JSON string for DB storage.
fn serialize_as_json_string<S>(value: &serde_json::Value, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&s)
}

/// Tool type indicating execution context
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ToolType {
    /// Local tool (e.g., MemoryTool, TodoTool)
    Local,
    /// MCP tool (executed via MCP server)
    Mcp,
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolType::Local => write!(f, "local"),
            ToolType::Mcp => write!(f, "mcp"),
        }
    }
}

/// Tool execution entity representing a single tool call.
///
/// Records both successful and failed executions with timing and results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecution {
    /// Unique identifier (UUID)
    pub id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Associated message ID (the assistant message during which this tool was called)
    pub message_id: String,
    /// Agent ID that executed the tool
    pub agent_id: String,
    /// Tool type (local or mcp)
    pub tool_type: ToolType,
    /// Tool name (e.g., "MemoryTool", "TodoTool", "mcp_serena__find_symbol")
    pub tool_name: String,
    /// MCP server name (only for MCP tools)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    /// Input parameters passed to the tool (stored as JSON string in DB, deserialized on read)
    #[serde(deserialize_with = "deserialize_json_string")]
    pub input_params: serde_json::Value,
    /// Output result from the tool (stored as JSON string in DB, deserialized on read)
    #[serde(deserialize_with = "deserialize_json_string")]
    pub output_result: serde_json::Value,
    /// Whether the execution was successful
    pub success: bool,
    /// Error message if execution failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Iteration number within the tool execution loop (0-indexed)
    pub iteration: u32,
    /// Timestamp when the execution was recorded
    pub created_at: DateTime<Utc>,
}

/// Payload for creating a new tool execution record.
///
/// ID and created_at are generated server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionCreate {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Associated message ID
    pub message_id: String,
    /// Agent ID
    pub agent_id: String,
    /// Tool type as string ("local" or "mcp")
    pub tool_type: String,
    /// Tool name
    pub tool_name: String,
    /// MCP server name (only for MCP tools)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    /// Input parameters (serialized as JSON string for SCHEMAFULL storage)
    #[serde(serialize_with = "serialize_as_json_string")]
    pub input_params: serde_json::Value,
    /// Output result (serialized as JSON string for SCHEMAFULL storage)
    #[serde(serialize_with = "serialize_as_json_string")]
    pub output_result: serde_json::Value,
    /// Success status
    pub success: bool,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Iteration number
    pub iteration: u32,
}

impl ToolExecutionCreate {
    /// Creates a new local tool execution record.
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn local(
        workflow_id: String,
        message_id: String,
        agent_id: String,
        tool_name: String,
        input_params: serde_json::Value,
        output_result: serde_json::Value,
        success: bool,
        error_message: Option<String>,
        duration_ms: u64,
        iteration: u32,
    ) -> Self {
        Self {
            workflow_id,
            message_id,
            agent_id,
            tool_type: "local".to_string(),
            tool_name,
            server_name: None,
            input_params,
            output_result,
            success,
            error_message,
            duration_ms,
            iteration,
        }
    }

    /// Creates a new MCP tool execution record.
    #[allow(dead_code)]
    #[allow(clippy::too_many_arguments)]
    pub fn mcp(
        workflow_id: String,
        message_id: String,
        agent_id: String,
        tool_name: String,
        server_name: String,
        input_params: serde_json::Value,
        output_result: serde_json::Value,
        success: bool,
        error_message: Option<String>,
        duration_ms: u64,
        iteration: u32,
    ) -> Self {
        Self {
            workflow_id,
            message_id,
            agent_id,
            tool_type: "mcp".to_string(),
            tool_name,
            server_name: Some(server_name),
            input_params,
            output_result,
            success,
            error_message,
            duration_ms,
            iteration,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_type_serialization() {
        let local = ToolType::Local;
        let json = serde_json::to_string(&local).unwrap();
        assert_eq!(json, "\"local\"");

        let mcp = ToolType::Mcp;
        let json = serde_json::to_string(&mcp).unwrap();
        assert_eq!(json, "\"mcp\"");
    }

    #[test]
    fn test_tool_type_deserialization() {
        let local: ToolType = serde_json::from_str("\"local\"").unwrap();
        assert!(matches!(local, ToolType::Local));

        let mcp: ToolType = serde_json::from_str("\"mcp\"").unwrap();
        assert!(matches!(mcp, ToolType::Mcp));
    }

    #[test]
    fn test_tool_type_display() {
        assert_eq!(ToolType::Local.to_string(), "local");
        assert_eq!(ToolType::Mcp.to_string(), "mcp");
    }

    #[test]
    fn test_tool_execution_serialization() {
        let execution = ToolExecution {
            id: "exec_001".to_string(),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_001".to_string(),
            agent_id: "agent_001".to_string(),
            tool_type: ToolType::Local,
            tool_name: "MemoryTool".to_string(),
            server_name: None,
            input_params: serde_json::json!({"operation": "add", "content": "test"}),
            output_result: serde_json::json!({"success": true, "id": "mem_001"}),
            success: true,
            error_message: None,
            duration_ms: 150,
            iteration: 0,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&execution).unwrap();
        assert!(json.contains("\"tool_type\":\"local\""));
        assert!(json.contains("\"tool_name\":\"MemoryTool\""));
        assert!(json.contains("\"success\":true"));
        assert!(!json.contains("\"server_name\"")); // Should be skipped when None
    }

    #[test]
    fn test_tool_execution_with_mcp() {
        let execution = ToolExecution {
            id: "exec_002".to_string(),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_002".to_string(),
            agent_id: "agent_001".to_string(),
            tool_type: ToolType::Mcp,
            tool_name: "find_symbol".to_string(),
            server_name: Some("serena".to_string()),
            input_params: serde_json::json!({"name": "MyClass"}),
            output_result: serde_json::json!({"found": true, "line": 42}),
            success: true,
            error_message: None,
            duration_ms: 500,
            iteration: 1,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&execution).unwrap();
        assert!(json.contains("\"tool_type\":\"mcp\""));
        assert!(json.contains("\"server_name\":\"serena\""));
    }

    #[test]
    fn test_tool_execution_with_error() {
        let execution = ToolExecution {
            id: "exec_003".to_string(),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_003".to_string(),
            agent_id: "agent_001".to_string(),
            tool_type: ToolType::Local,
            tool_name: "TodoTool".to_string(),
            server_name: None,
            input_params: serde_json::json!({"operation": "get", "id": "invalid"}),
            output_result: serde_json::json!({}),
            success: false,
            error_message: Some("Task not found".to_string()),
            duration_ms: 10,
            iteration: 0,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&execution).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error_message\":\"Task not found\""));
    }

    #[test]
    fn test_tool_execution_create_local() {
        let create = ToolExecutionCreate::local(
            "wf_001".to_string(),
            "msg_001".to_string(),
            "agent_001".to_string(),
            "MemoryTool".to_string(),
            serde_json::json!({"operation": "add"}),
            serde_json::json!({"success": true}),
            true,
            None,
            100,
            0,
        );

        assert_eq!(create.tool_type, "local");
        assert!(create.server_name.is_none());
    }

    #[test]
    fn test_tool_execution_create_mcp() {
        let create = ToolExecutionCreate::mcp(
            "wf_001".to_string(),
            "msg_001".to_string(),
            "agent_001".to_string(),
            "find_symbol".to_string(),
            "serena".to_string(),
            serde_json::json!({"name": "MyClass"}),
            serde_json::json!({"found": true}),
            true,
            None,
            200,
            1,
        );

        assert_eq!(create.tool_type, "mcp");
        assert_eq!(create.server_name, Some("serena".to_string()));
    }
}
