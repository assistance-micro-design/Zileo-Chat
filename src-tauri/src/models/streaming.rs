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

use super::UserQuestionStreamPayload;

/// Type of streaming chunk content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkType {
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
    /// User question started (waiting for user response)
    UserQuestionStart,
    /// User question completed (answered, skipped, or timed out)
    UserQuestionComplete,
    /// Complete thinking block from reasoning model
    ThinkingBlock,
    /// Tool call completed with full input/output details
    ToolCallComplete,
    /// Complete response block with real tokens
    ResponseBlock,
}

/// Streaming chunk emitted during workflow execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Type of chunk content
    pub chunk_type: ChunkType,
    /// Text content (for reasoning/error/thinking_block/response_block chunks)
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
    /// Agent name associated with task (for task_* chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_agent_name: Option<String>,
    /// User question payload (for user_question_start chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_question: Option<UserQuestionStreamPayload>,
    /// Question ID (for user_question_complete chunks)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
    /// Token count for this chunk (incremental)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_delta: Option<usize>,
    /// Cumulative token count (running total)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_total: Option<usize>,
    /// Tool type: "local" or "mcp" (for tool_call_complete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_type: Option<String>,
    /// MCP server name (for tool_call_complete, only for MCP tools)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    /// Tool input parameters as JSON string (for tool_call_complete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_input: Option<String>,
    /// Tool output result as JSON string (for tool_call_complete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_output: Option<String>,
    /// Tool execution success/failure (for tool_call_complete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_success: Option<bool>,
    /// Input tokens count (for response_block)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_input: Option<usize>,
    /// Output tokens count (for response_block)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_output: Option<usize>,
    /// Cached input tokens - cache reads (for response_block, prompt caching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<usize>,
    /// Cache-write tokens (for response_block, prompt caching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<usize>,
    /// Thinking/reasoning tokens (for response_block, reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<usize>,
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
    /// Creates a base chunk with all optional fields set to None.
    ///
    /// Used internally by all constructors to avoid repeating 25+ None fields.
    fn base(workflow_id: impl Into<String>, chunk_type: ChunkType) -> Self {
        Self {
            workflow_id: workflow_id.into(),
            chunk_type,
            content: None,
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
            task_agent_name: None,
            user_question: None,
            question_id: None,
            tokens_delta: None,
            tokens_total: None,
            tool_type: None,
            server_name: None,
            tool_input: None,
            tool_output: None,
            tool_success: None,
            tokens_input: None,
            tokens_output: None,
            cached_tokens: None,
            cache_write_tokens: None,
            thinking_tokens: None,
        }
    }

    /// Creates a new tool start chunk.
    pub fn tool_start(workflow_id: String, tool: String) -> Self {
        Self {
            tool: Some(tool),
            ..Self::base(workflow_id, ChunkType::ToolStart)
        }
    }

    /// Creates a new reasoning chunk.
    pub fn reasoning(workflow_id: String, content: String) -> Self {
        Self {
            content: Some(content),
            ..Self::base(workflow_id, ChunkType::Reasoning)
        }
    }

    /// Creates a new error chunk.
    pub fn error(workflow_id: String, error: String) -> Self {
        Self {
            content: Some(error),
            ..Self::base(workflow_id, ChunkType::Error)
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
            content: Some(task_description),
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            ..Self::base(workflow_id, ChunkType::SubAgentStart)
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
        let duration_ms = metrics.duration_ms;
        Self {
            content: Some(report),
            duration: Some(duration_ms),
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            metrics: Some(metrics),
            progress: Some(100),
            ..Self::base(workflow_id, ChunkType::SubAgentComplete)
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
            content: Some(error_message),
            duration: Some(duration_ms),
            sub_agent_id: Some(sub_agent_id),
            sub_agent_name: Some(sub_agent_name),
            parent_agent_id: Some(parent_agent_id),
            ..Self::base(workflow_id, ChunkType::SubAgentError)
        }
    }

    /// Creates a task create event chunk.
    ///
    /// Emitted when a new task is created.
    pub fn task_create(
        workflow_id: impl Into<String>,
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        priority: u8,
        agent_name: Option<String>,
    ) -> Self {
        Self {
            task_id: Some(task_id.into()),
            task_name: Some(task_name.into()),
            task_status: Some("pending".to_string()),
            task_priority: Some(priority),
            task_agent_name: agent_name,
            ..Self::base(workflow_id, ChunkType::TaskCreate)
        }
    }

    /// Creates a task update event chunk.
    ///
    /// Emitted when a task status is updated.
    pub fn task_update(
        workflow_id: impl Into<String>,
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            task_id: Some(task_id.into()),
            task_name: Some(task_name.into()),
            task_status: Some(status.into()),
            ..Self::base(workflow_id, ChunkType::TaskUpdate)
        }
    }

    /// Creates a task complete event chunk.
    ///
    /// Emitted when a task is completed.
    pub fn task_complete(
        workflow_id: impl Into<String>,
        task_id: impl Into<String>,
        task_name: impl Into<String>,
        duration: Option<u64>,
    ) -> Self {
        Self {
            duration,
            task_id: Some(task_id.into()),
            task_name: Some(task_name.into()),
            task_status: Some("completed".to_string()),
            ..Self::base(workflow_id, ChunkType::TaskComplete)
        }
    }

    /// Creates a user question start chunk.
    ///
    /// Emitted when an agent asks a question to the user and waits for response.
    pub fn user_question_start(workflow_id: String, payload: UserQuestionStreamPayload) -> Self {
        Self {
            user_question: Some(payload),
            ..Self::base(workflow_id, ChunkType::UserQuestionStart)
        }
    }

    /// Creates a user question complete chunk.
    ///
    /// Emitted when a user question is answered, skipped, or timed out.
    pub fn user_question_complete(workflow_id: String, question_id: String) -> Self {
        Self {
            question_id: Some(question_id),
            ..Self::base(workflow_id, ChunkType::UserQuestionComplete)
        }
    }

    /// Creates a thinking block chunk from reasoning model output.
    ///
    /// Emitted when a reasoning model returns thinking content.
    pub fn thinking_block(workflow_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            content: Some(content.into()),
            ..Self::base(workflow_id, ChunkType::ThinkingBlock)
        }
    }

    /// Creates a tool call complete chunk with full input/output details.
    ///
    /// Replaces tool_end with enriched data for inline display.
    #[allow(clippy::too_many_arguments)]
    pub fn tool_call_complete(
        workflow_id: impl Into<String>,
        tool_name: impl Into<String>,
        tool_type: impl Into<String>,
        server_name: Option<String>,
        duration: u64,
        input: impl Into<String>,
        output: impl Into<String>,
        success: bool,
    ) -> Self {
        Self {
            tool: Some(tool_name.into()),
            tool_type: Some(tool_type.into()),
            server_name,
            duration: Some(duration),
            tool_input: Some(input.into()),
            tool_output: Some(output.into()),
            tool_success: Some(success),
            ..Self::base(workflow_id, ChunkType::ToolCallComplete)
        }
    }

    /// Creates a response block chunk with complete content and real tokens.
    ///
    /// Replaces progressive token streaming with a single complete response.
    pub fn response_block(
        workflow_id: impl Into<String>,
        content: impl Into<String>,
        tokens_input: usize,
        tokens_output: usize,
        cached_tokens: Option<usize>,
        cache_write_tokens: Option<usize>,
        thinking_tokens: Option<usize>,
    ) -> Self {
        Self {
            content: Some(content.into()),
            tokens_input: Some(tokens_input),
            tokens_output: Some(tokens_output),
            cached_tokens,
            cache_write_tokens,
            thinking_tokens,
            ..Self::base(workflow_id, ChunkType::ResponseBlock)
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

/// Validation request details for human-in-the-loop approval.
///
/// Used for all validation types: sub-agent, tool, and MCP operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequiredEvent {
    /// Validation request ID (for approve/reject calls)
    pub validation_id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Validation type (e.g. "sub_agent", "tool", "mcp", "file_op", "db_op")
    pub validation_type: String,
    /// Operation description
    pub operation: String,
    /// Risk level assessment
    pub risk_level: String,
    /// Additional details about the operation
    pub details: serde_json::Value,
}

/// Resolution emitted when a validation request is resolved server-side.
///
/// Sent specifically when the backend resolves a validation without a user
/// decision (timeout). The frontend uses this to close the validation modal
/// once the configured `timeout_seconds` is reached so it stops being the
/// authoritative source of truth for the timer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResolvedEvent {
    /// Validation request ID that was resolved
    pub validation_id: String,
    /// Resolution outcome ("approved", "rejected", "skipped")
    pub resolution: String,
    /// Source of the resolution ("timeout")
    pub source: String,
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

/// Event names for Tauri event emitters.
pub mod events {
    /// Streaming chunk event name
    pub const WORKFLOW_STREAM: &str = "workflow_stream";
    /// Workflow completion event name
    pub const WORKFLOW_COMPLETE: &str = "workflow_complete";
    /// Validation required event name (sub-agent operations)
    pub const VALIDATION_REQUIRED: &str = "validation_required";
    /// Validation resolved event name (server-side resolution, e.g. timeout)
    pub const VALIDATION_RESOLVED: &str = "validation_resolved";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type_serialization() {
        assert_eq!(
            serde_json::to_string(&ChunkType::ToolStart).unwrap(),
            "\"tool_start\""
        );
        assert_eq!(
            serde_json::to_string(&ChunkType::ToolEnd).unwrap(),
            "\"tool_end\""
        );
        assert_eq!(
            serde_json::to_string(&ChunkType::ThinkingBlock).unwrap(),
            "\"thinking_block\""
        );
        assert_eq!(
            serde_json::to_string(&ChunkType::ToolCallComplete).unwrap(),
            "\"tool_call_complete\""
        );
        assert_eq!(
            serde_json::to_string(&ChunkType::ResponseBlock).unwrap(),
            "\"response_block\""
        );
    }

    #[test]
    fn test_stream_chunk_tool() {
        let chunk = StreamChunk::tool_start("wf_001".to_string(), "search".to_string());
        assert_eq!(chunk.chunk_type, ChunkType::ToolStart);
        assert_eq!(chunk.tool, Some("search".to_string()));
        assert!(chunk.content.is_none());
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
    fn test_user_question_chunk_type_serialization() {
        let chunk_type = ChunkType::UserQuestionStart;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"user_question_start\"");

        let chunk_type = ChunkType::UserQuestionComplete;
        let json = serde_json::to_string(&chunk_type).unwrap();
        assert_eq!(json, "\"user_question_complete\"");
    }

    #[test]
    fn test_stream_chunk_user_question_start() {
        let payload = UserQuestionStreamPayload {
            question_id: "q_001".to_string(),
            question: "Which database?".to_string(),
            question_type: "checkbox".to_string(),
            options: None,
            text_placeholder: None,
            text_required: false,
            context: Some("We need to choose a DB".to_string()),
        };
        let chunk = StreamChunk::user_question_start("wf_001".to_string(), payload);
        assert_eq!(chunk.chunk_type, ChunkType::UserQuestionStart);
        assert!(chunk.user_question.is_some());
        let uq = chunk.user_question.as_ref().unwrap();
        assert_eq!(uq.question_id, "q_001");
        assert_eq!(uq.question, "Which database?");
        assert!(chunk.question_id.is_none());
        assert!(chunk.content.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"user_question_start\""));
        assert!(json.contains("\"user_question\""));
        // Inside the payload, fields are camelCase due to UserQuestionStreamPayload serde rename
        assert!(json.contains("\"questionId\":\"q_001\""));
    }

    #[test]
    fn test_stream_chunk_user_question_complete() {
        let chunk = StreamChunk::user_question_complete("wf_001".to_string(), "q_001".to_string());
        assert_eq!(chunk.chunk_type, ChunkType::UserQuestionComplete);
        assert_eq!(chunk.question_id, Some("q_001".to_string()));
        assert!(chunk.user_question.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"user_question_complete\""));
        assert!(json.contains("\"question_id\":\"q_001\""));
    }

    #[test]
    fn test_optional_fields_skipped_when_none() {
        let chunk = StreamChunk::reasoning("wf_001".to_string(), "Analyzing...".to_string());
        let json = serde_json::to_string(&chunk).unwrap();
        assert!(!json.contains("user_question"));
        assert!(!json.contains("question_id"));
        assert!(!json.contains("tool_type"));
        assert!(!json.contains("server_name"));
        assert!(!json.contains("tool_input"));
        assert!(!json.contains("tool_output"));
        assert!(!json.contains("tool_success"));
        assert!(!json.contains("tokens_input"));
        assert!(!json.contains("tokens_output"));
    }

    #[test]
    fn test_stream_chunk_thinking_block() {
        let chunk = StreamChunk::thinking_block("wf_001", "Let me reason about this...");
        assert_eq!(chunk.chunk_type, ChunkType::ThinkingBlock);
        assert_eq!(
            chunk.content,
            Some("Let me reason about this...".to_string())
        );
        assert!(chunk.tool.is_none());
        assert!(chunk.tool_input.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"thinking_block\""));
        assert!(json.contains("Let me reason about this..."));
        // New fields should be omitted when None
        assert!(!json.contains("tool_input"));
        assert!(!json.contains("tokens_input"));
    }

    #[test]
    fn test_stream_chunk_tool_call_complete() {
        let chunk = StreamChunk::tool_call_complete(
            "wf_001",
            "MemoryTool",
            "local",
            None,
            150,
            r#"{"query": "find docs"}"#,
            r#"{"results": ["doc1", "doc2"]}"#,
            true,
        );
        assert_eq!(chunk.chunk_type, ChunkType::ToolCallComplete);
        assert_eq!(chunk.tool, Some("MemoryTool".to_string()));
        assert_eq!(chunk.tool_type, Some("local".to_string()));
        assert!(chunk.server_name.is_none());
        assert_eq!(chunk.duration, Some(150));
        assert_eq!(
            chunk.tool_input,
            Some(r#"{"query": "find docs"}"#.to_string())
        );
        assert_eq!(
            chunk.tool_output,
            Some(r#"{"results": ["doc1", "doc2"]}"#.to_string())
        );
        assert_eq!(chunk.tool_success, Some(true));
        assert!(chunk.tokens_input.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"tool_call_complete\""));
        assert!(json.contains("\"tool_type\":\"local\""));
        assert!(!json.contains("\"server_name\"")); // Skipped when None
        assert!(json.contains("\"tool_input\""));
        assert!(json.contains("\"tool_output\""));
        assert!(json.contains("\"tool_success\":true"));
    }

    #[test]
    fn test_stream_chunk_tool_call_complete_mcp() {
        let chunk = StreamChunk::tool_call_complete(
            "wf_001",
            "find_symbol",
            "mcp",
            Some("serena".to_string()),
            200,
            r#"{"name": "MyClass"}"#,
            r#"{"found": true}"#,
            true,
        );
        assert_eq!(chunk.tool_type, Some("mcp".to_string()));
        assert_eq!(chunk.server_name, Some("serena".to_string()));

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"tool_type\":\"mcp\""));
        assert!(json.contains("\"server_name\":\"serena\""));
    }

    #[test]
    fn test_stream_chunk_tool_call_complete_failure() {
        let chunk = StreamChunk::tool_call_complete(
            "wf_001",
            "BadTool",
            "local",
            None,
            50,
            "{}",
            r#"{"error": "Connection refused"}"#,
            false,
        );
        assert_eq!(chunk.tool_success, Some(false));
    }

    #[test]
    fn test_stream_chunk_response_block() {
        let chunk =
            StreamChunk::response_block("wf_001", "The answer is 42.", 100, 25, None, None, None);
        assert_eq!(chunk.chunk_type, ChunkType::ResponseBlock);
        assert_eq!(chunk.content, Some("The answer is 42.".to_string()));
        assert_eq!(chunk.tokens_input, Some(100));
        assert_eq!(chunk.tokens_output, Some(25));
        assert!(chunk.tool.is_none());
        assert!(chunk.tool_input.is_none());

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("\"chunk_type\":\"response_block\""));
        assert!(json.contains("\"tokens_input\":100"));
        assert!(json.contains("\"tokens_output\":25"));
    }
}
