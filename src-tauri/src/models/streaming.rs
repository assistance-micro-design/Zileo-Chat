// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Streaming event types for real-time workflow execution.
//!
//! These types are synchronized with TypeScript frontend types (src/types/streaming.ts)
//! to ensure type safety for Tauri event streaming.

use serde::{Deserialize, Serialize};

/// Type of streaming chunk content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
    /// Token from LLM response
    Token,
    /// Tool execution started
    ToolStart,
    /// Tool execution completed
    ToolEnd,
    /// Reasoning/thinking step
    Reasoning,
    /// Error occurred
    Error,
}

/// Streaming chunk emitted during workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Type of chunk content
    pub chunk_type: ChunkType,
    /// Text content (for token/reasoning/error chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Tool name (for tool_start/tool_end chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    /// Duration in milliseconds (for tool_end chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u64>,
}

impl StreamChunk {
    /// Creates a new token chunk
    pub fn token(workflow_id: String, content: String) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::Token,
            content: Some(content),
            tool: None,
            duration: None,
        }
    }

    /// Creates a new tool start chunk
    pub fn tool_start(workflow_id: String, tool: String) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::ToolStart,
            content: None,
            tool: Some(tool),
            duration: None,
        }
    }

    /// Creates a new tool end chunk
    pub fn tool_end(workflow_id: String, tool: String, duration: u64) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::ToolEnd,
            content: None,
            tool: Some(tool),
            duration: Some(duration),
        }
    }

    /// Creates a new reasoning chunk
    pub fn reasoning(workflow_id: String, content: String) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::Reasoning,
            content: Some(content),
            tool: None,
            duration: None,
        }
    }

    /// Creates a new error chunk
    pub fn error(workflow_id: String, error: String) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::Error,
            content: Some(error),
            tool: None,
            duration: None,
        }
    }
}

/// Workflow completion status
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompletionStatus {
    /// Workflow completed successfully
    Completed,
    /// Workflow encountered an error
    Error,
    /// Workflow was cancelled by user
    Cancelled,
}

/// Event emitted when workflow execution completes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowComplete {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Final workflow status
    pub status: CompletionStatus,
    /// Error message if status is 'error'
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl WorkflowComplete {
    /// Creates a successful completion event
    pub fn success(workflow_id: String) -> Self {
        Self {
            workflow_id,
            status: CompletionStatus::Completed,
            error: None,
        }
    }

    /// Creates an error completion event
    pub fn failed(workflow_id: String, error: String) -> Self {
        Self {
            workflow_id,
            status: CompletionStatus::Error,
            error: Some(error),
        }
    }

    /// Creates a cancelled completion event
    pub fn cancelled(workflow_id: String) -> Self {
        Self {
            workflow_id,
            status: CompletionStatus::Cancelled,
            error: None,
        }
    }
}

/// Event names for Tauri event emitters
pub mod events {
    /// Streaming chunk event name
    pub const WORKFLOW_STREAM: &str = "workflow_stream";
    /// Workflow completion event name
    pub const WORKFLOW_COMPLETE: &str = "workflow_complete";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type_serialization() {
        let chunk_type = ChunkType::Token;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"token\"");

        let chunk_type = ChunkType::ToolStart;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"tool_start\"");

        let chunk_type = ChunkType::ToolEnd;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"tool_end\"");
    }

    #[test]
    fn test_stream_chunk_token() {
        let chunk = StreamChunk::token("wf_001".to_string(), "Hello".to_string());
        assert_eq!(chunk.chunk_type, ChunkType::Token);
        assert_eq!(chunk.content, Some("Hello".to_string()));
        assert!(chunk.tool.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"token\""));
        assert!(json.contains("\"content\":\"Hello\""));
    }

    #[test]
    fn test_stream_chunk_tool() {
        let chunk = StreamChunk::tool_start("wf_001".to_string(), "search".to_string());
        assert_eq!(chunk.chunk_type, ChunkType::ToolStart);
        assert_eq!(chunk.tool, Some("search".to_string()));
        assert!(chunk.content.is_none());

        let chunk = StreamChunk::tool_end("wf_001".to_string(), "search".to_string(), 150);
        assert_eq!(chunk.chunk_type, ChunkType::ToolEnd);
        assert_eq!(chunk.duration, Some(150));
    }

    #[test]
    fn test_stream_chunk_error() {
        let chunk = StreamChunk::error("wf_001".to_string(), "Connection failed".to_string());
        assert_eq!(chunk.chunk_type, ChunkType::Error);
        assert_eq!(chunk.content, Some("Connection failed".to_string()));
    }

    #[test]
    fn test_workflow_complete_success() {
        let complete = WorkflowComplete::success("wf_001".to_string());
        assert_eq!(complete.status, CompletionStatus::Completed);
        assert!(complete.error.is_none());

        let json = serde_json::to_string(&complete).unwrap();
        assert!(json.contains("\"status\":\"completed\""));
        assert!(!json.contains("\"error\""));
    }

    #[test]
    fn test_workflow_complete_failed() {
        let complete = WorkflowComplete::failed("wf_001".to_string(), "Timeout".to_string());
        assert_eq!(complete.status, CompletionStatus::Error);
        assert_eq!(complete.error, Some("Timeout".to_string()));

        let json = serde_json::to_string(&complete).unwrap();
        assert!(json.contains("\"status\":\"error\""));
        assert!(json.contains("\"error\":\"Timeout\""));
    }

    #[test]
    fn test_completion_status_serialization() {
        let status = CompletionStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");

        let status = CompletionStatus::Error;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"error\"");

        let status = CompletionStatus::Cancelled;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"cancelled\"");
    }

    #[test]
    fn test_workflow_complete_cancelled() {
        let complete = WorkflowComplete::cancelled("wf_001".to_string());
        assert_eq!(complete.status, CompletionStatus::Cancelled);
        assert!(complete.error.is_none());

        let json = serde_json::to_string(&complete).unwrap();
        assert!(json.contains("\"status\":\"cancelled\""));
        assert!(!json.contains("\"error\""));
    }
}
