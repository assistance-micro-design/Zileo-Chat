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
    /// Sub-agent execution started
    SubAgentStart,
    /// Sub-agent execution progress update
    SubAgentProgress,
    /// Sub-agent execution completed
    SubAgentComplete,
    /// Sub-agent execution error
    SubAgentError,
    /// Task created
    TaskCreate,
    /// Task updated
    TaskUpdate,
    /// Task completed
    TaskComplete,
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
    /// Sub-agent ID (for sub_agent_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_agent_id: Option<String>,
    /// Sub-agent name (for sub_agent_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_agent_name: Option<String>,
    /// Parent agent ID (for sub_agent_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
    /// Sub-agent metrics (for sub_agent_complete chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<SubAgentStreamMetrics>,
    /// Progress percentage 0-100 (for sub_agent_progress chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
    /// Task ID (for task_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    /// Task name (for task_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_name: Option<String>,
    /// Task status (for task_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_status: Option<String>,
    /// Task priority (for task_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_priority: Option<u8>,
    /// Token count for this chunk (incremental)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_delta: Option<usize>,
    /// Cumulative token count (running total)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_total: Option<usize>,
}

/// Metrics included in sub-agent complete events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgentStreamMetrics {
    /// Execution duration in milliseconds
    pub duration_ms: u64,
    /// Input tokens consumed
    pub tokens_input: u64,
    /// Output tokens generated
    pub tokens_output: u64,
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
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a new token chunk with token counts
    #[allow(dead_code)]
    pub fn token_with_counts(
        workflow_id: String,
        content: String,
        tokens_delta: usize,
        tokens_total: usize,
    ) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::Token,
            content: Some(content),
            tool: None,
            duration: None,
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: Some(tokens_delta),
            tokens_total: Some(tokens_total),
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
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
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
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
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
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
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
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a sub-agent start event chunk.
    ///
    /// Emitted when a sub-agent begins execution after validation approval.
    pub fn sub_agent_start(
        workflow_id: String,
        sub_agent_id: String,
        sub_agent_name: String,
        parent_agent_id: String,
        task_description: String,
    ) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::SubAgentStart,
            content: Some(task_description),
            tool: None,
            duration: None,
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a sub-agent progress event chunk.
    ///
    /// Emitted periodically during sub-agent execution to report progress.
    /// Currently not used but defined for future implementation.
    #[allow(dead_code)]
    pub fn sub_agent_progress(
        workflow_id: String,
        sub_agent_id: String,
        sub_agent_name: String,
        parent_agent_id: String,
        progress: u8,
        status_message: Option<String>,
    ) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::SubAgentProgress,
            content: status_message,
            tool: None,
            duration: None,
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            metrics: None,
            progress: Some(progress.min(100)),
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a sub-agent complete event chunk.
    ///
    /// Emitted when a sub-agent successfully completes execution with its report.
    pub fn sub_agent_complete(
        workflow_id: String,
        sub_agent_id: String,
        sub_agent_name: String,
        parent_agent_id: String,
        report: String,
        metrics: SubAgentStreamMetrics,
    ) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::SubAgentComplete,
            content: Some(report),
            tool: None,
            duration: Some(metrics.duration_ms),
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            metrics: Some(metrics),
            progress: Some(100),
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a sub-agent error event chunk.
    ///
    /// Emitted when a sub-agent execution fails.
    pub fn sub_agent_error(
        workflow_id: String,
        sub_agent_id: String,
        sub_agent_name: String,
        parent_agent_id: String,
        error_message: String,
        duration_ms: u64,
    ) -> Self {
        Self {
            workflow_id,
            chunk_type: ChunkType::SubAgentError,
            content: Some(error_message),
            tool: None,
            duration: Some(duration_ms),
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            metrics: None,
            progress: None,
            task_id: None,
            task_name: None,
            task_status: None,
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a task create event chunk.
    ///
    /// Emitted when a new task is created.
    #[allow(dead_code)]
    pub fn task_create(
        workflow_id: impl Into<String>,
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        priority: u8,
    ) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            chunk_type: ChunkType::TaskCreate,
            content: None,
            tool: None,
            duration: None,
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: Some(task_id.into()),
            task_name: Some(task_name.into()),
            task_status: Some("pending".to_string()),
            task_priority: Some(priority),
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a task update event chunk.
    ///
    /// Emitted when a task status is updated.
    #[allow(dead_code)]
    pub fn task_update(
        workflow_id: impl Into<String>,
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            chunk_type: ChunkType::TaskUpdate,
            content: None,
            tool: None,
            duration: None,
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: Some(task_id.into()),
            task_name: Some(task_name.into()),
            task_status: Some(status.into()),
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
        }
    }

    /// Creates a task complete event chunk.
    ///
    /// Emitted when a task is completed.
    #[allow(dead_code)]
    pub fn task_complete(
        workflow_id: impl Into<String>,
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        duration: Option<u64>,
    ) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            chunk_type: ChunkType::TaskComplete,
            content: None,
            tool: None,
            duration,
            sub_agent_id: None,
            sub_agent_name: None,
            parent_agent_id: None,
            metrics: None,
            progress: None,
            task_id: Some(task_id.into()),
            task_name: Some(task_name.into()),
            task_status: Some("completed".to_string()),
            task_priority: None,
            tokens_delta: None,
            tokens_total: None,
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

/// Validation request details for human-in-the-loop approval
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequiredEvent {
    /// Validation request ID (for approve/reject calls)
    pub validation_id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Type of sub-agent operation
    pub operation_type: SubAgentOperationType,
    /// Operation description
    pub operation: String,
    /// Risk level assessment
    pub risk_level: String,
    /// Additional details about the operation
    pub details: serde_json::Value,
}

/// Type of sub-agent operation requiring validation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentOperationType {
    /// Spawning a new temporary sub-agent
    Spawn,
    /// Delegating to an existing agent
    Delegate,
    /// Parallel batch execution
    ParallelBatch,
}

impl std::fmt::Display for SubAgentOperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SubAgentOperationType::Spawn => write!(f, "spawn"),
            SubAgentOperationType::Delegate => write!(f, "delegate"),
            SubAgentOperationType::ParallelBatch => write!(f, "parallel_batch"),
        }
    }
}

/// Validation response event (approval or rejection)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResponseEvent {
    /// Validation request ID
    pub validation_id: String,
    /// Whether approved (true) or rejected (false)
    pub approved: bool,
    /// Rejection reason if not approved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Event names for Tauri event emitters
#[allow(dead_code)]
pub mod events {
    /// Streaming chunk event name
    pub const WORKFLOW_STREAM: &str = "workflow_stream";
    /// Workflow completion event name
    pub const WORKFLOW_COMPLETE: &str = "workflow_complete";
    /// Validation required event name (sub-agent operations)
    pub const VALIDATION_REQUIRED: &str = "validation_required";
    /// Validation response event name (approved/rejected)
    pub const VALIDATION_RESPONSE: &str = "validation_response";
    /// Sub-agent start event name
    pub const SUB_AGENT_START: &str = "sub_agent_start";
    /// Sub-agent progress event name
    pub const SUB_AGENT_PROGRESS: &str = "sub_agent_progress";
    /// Sub-agent complete event name
    pub const SUB_AGENT_COMPLETE: &str = "sub_agent_complete";
    /// Sub-agent error event name
    pub const SUB_AGENT_ERROR: &str = "sub_agent_error";
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

    #[test]
    fn test_sub_agent_chunk_type_serialization() {
        let chunk_type = ChunkType::SubAgentStart;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"sub_agent_start\"");

        let chunk_type = ChunkType::SubAgentProgress;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"sub_agent_progress\"");

        let chunk_type = ChunkType::SubAgentComplete;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"sub_agent_complete\"");

        let chunk_type = ChunkType::SubAgentError;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"sub_agent_error\"");
    }

    #[test]
    fn test_stream_chunk_sub_agent_start() {
        let chunk = StreamChunk::sub_agent_start(
            "wf_001".to_string(),
            "sub_123".to_string(),
            "Analyzer".to_string(),
            "parent_456".to_string(),
            "Analyze the codebase".to_string(),
        );
        assert_eq!(chunk.chunk_type, ChunkType::SubAgentStart);
        assert_eq!(chunk.sub_agent_id, Some("sub_123".to_string()));
        assert_eq!(chunk.sub_agent_name, Some("Analyzer".to_string()));
        assert_eq!(chunk.parent_agent_id, Some("parent_456".to_string()));
        assert_eq!(chunk.content, Some("Analyze the codebase".to_string()));
        assert!(chunk.metrics.is_none());
        assert!(chunk.progress.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"sub_agent_start\""));
        assert!(json.contains("\"sub_agent_id\":\"sub_123\""));
    }

    #[test]
    fn test_stream_chunk_sub_agent_progress() {
        let chunk = StreamChunk::sub_agent_progress(
            "wf_001".to_string(),
            "sub_123".to_string(),
            "Analyzer".to_string(),
            "parent_456".to_string(),
            50,
            Some("Processing files...".to_string()),
        );
        assert_eq!(chunk.chunk_type, ChunkType::SubAgentProgress);
        assert_eq!(chunk.progress, Some(50));
        assert_eq!(chunk.content, Some("Processing files...".to_string()));

        // Test clamping to 100
        let chunk_over = StreamChunk::sub_agent_progress(
            "wf_001".to_string(),
            "sub_123".to_string(),
            "Analyzer".to_string(),
            "parent_456".to_string(),
            150,
            None,
        );
        assert_eq!(chunk_over.progress, Some(100));
    }

    #[test]
    fn test_stream_chunk_sub_agent_complete() {
        let metrics = SubAgentStreamMetrics {
            duration_ms: 2500,
            tokens_input: 500,
            tokens_output: 1000,
        };
        let chunk = StreamChunk::sub_agent_complete(
            "wf_001".to_string(),
            "sub_123".to_string(),
            "Analyzer".to_string(),
            "parent_456".to_string(),
            "# Analysis Report\n\nFindings here...".to_string(),
            metrics,
        );
        assert_eq!(chunk.chunk_type, ChunkType::SubAgentComplete);
        assert_eq!(chunk.progress, Some(100));
        assert!(chunk.metrics.is_some());
        let m = chunk.metrics.as_ref().unwrap();
        assert_eq!(m.duration_ms, 2500);
        assert_eq!(m.tokens_input, 500);
        assert_eq!(m.tokens_output, 1000);

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"sub_agent_complete\""));
        assert!(json.contains("\"duration_ms\":2500"));
    }

    #[test]
    fn test_stream_chunk_sub_agent_error() {
        let chunk = StreamChunk::sub_agent_error(
            "wf_001".to_string(),
            "sub_123".to_string(),
            "Analyzer".to_string(),
            "parent_456".to_string(),
            "Connection timeout".to_string(),
            1500,
        );
        assert_eq!(chunk.chunk_type, ChunkType::SubAgentError);
        assert_eq!(chunk.content, Some("Connection timeout".to_string()));
        assert_eq!(chunk.duration, Some(1500));
        assert!(chunk.metrics.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"sub_agent_error\""));
        assert!(json.contains("Connection timeout"));
    }

    #[test]
    fn test_sub_agent_stream_metrics_serialization() {
        let metrics = SubAgentStreamMetrics {
            duration_ms: 3000,
            tokens_input: 250,
            tokens_output: 800,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"duration_ms\":3000"));
        assert!(json.contains("\"tokens_input\":250"));
        assert!(json.contains("\"tokens_output\":800"));
    }

    #[test]
    fn test_stream_chunk_with_tokens() {
        // Test token chunk without counts (default)
        let chunk = StreamChunk::token("wf_001".to_string(), "Hello".to_string());
        assert!(chunk.tokens_delta.is_none());
        assert!(chunk.tokens_total.is_none());

        // Test that optional fields are not serialized when None
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(!json.contains("tokens_delta"));
        assert!(!json.contains("tokens_total"));

        // Test token chunk with counts
        let chunk_with_tokens =
            StreamChunk::token_with_counts("wf_001".to_string(), "Hello".to_string(), 5, 100);
        assert_eq!(chunk_with_tokens.tokens_delta, Some(5));
        assert_eq!(chunk_with_tokens.tokens_total, Some(100));
        assert_eq!(chunk_with_tokens.chunk_type, ChunkType::Token);
        assert_eq!(chunk_with_tokens.content, Some("Hello".to_string()));

        // Test serialization includes token fields when present
        let json_with_tokens = serde_json::to_string(&chunk_with_tokens).unwrap();
        assert!(json_with_tokens.contains("\"tokens_delta\":5"));
        assert!(json_with_tokens.contains("\"tokens_total\":100"));
    }
}
