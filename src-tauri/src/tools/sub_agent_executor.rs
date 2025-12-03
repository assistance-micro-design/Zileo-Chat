// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Common execution logic for sub-agent tools.
//!
//! This module centralizes duplicated code across SpawnAgentTool, DelegateTaskTool,
//! and ParallelTasksTool to provide a unified interface for:
//! - Permission checks (primary agent only)
//! - Limit validation (MAX_SUB_AGENTS)
//! - Execution record lifecycle (create, update)
//! - Event emission (streaming)
//! - Metrics collection
//!
//! # Usage
//!
//! ```ignore
//! let executor = SubAgentExecutor::new(
//!     db.clone(),
//!     orchestrator.clone(),
//!     mcp_manager.clone(),
//!     app_handle.clone(),
//!     workflow_id.to_string(),
//!     parent_agent_id.to_string(),
//! );
//!
//! // Check permissions and limits
//! executor.check_primary_permission(is_primary, "spawn")?;
//! executor.check_limit(current_count, "spawn")?;
//!
//! // Create execution record
//! let execution_id = executor.create_execution_record(
//!     &sub_agent_id,
//!     "Sub-Agent Name",
//!     "Task prompt"
//! ).await?;
//!
//! // Execute with metrics
//! let result = executor.execute_with_metrics(&sub_agent_id, task).await;
//!
//! // Update record and emit events
//! executor.update_execution_record(&execution_id, &result).await;
//! executor.emit_complete_event(&sub_agent_id, "Sub-Agent Name", &result);
//! ```

use std::sync::Arc;
use std::time::Instant;

use serde_json::Value;
use tauri::Emitter;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::agents::core::agent::Task;
use crate::agents::core::orchestrator::AgentOrchestrator;
use crate::db::DBClient;
use crate::mcp::manager::MCPManager;
use crate::models::streaming::{events, StreamChunk, SubAgentOperationType, SubAgentStreamMetrics};
use crate::models::sub_agent::{
    constants::MAX_SUB_AGENTS, SubAgentExecutionCreate, SubAgentMetrics,
};
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{ToolError, ToolResult};

/// Result of sub-agent execution with metrics.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Markdown report from sub-agent
    pub report: String,
    /// Execution metrics
    pub metrics: SubAgentMetrics,
    /// Error message if failed
    pub error_message: Option<String>,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            success: false,
            report: String::new(),
            metrics: SubAgentMetrics {
                duration_ms: 0,
                tokens_input: 0,
                tokens_output: 0,
            },
            error_message: None,
        }
    }
}

/// Common executor for sub-agent operations.
///
/// Centralizes shared logic across SpawnAgentTool, DelegateTaskTool, and ParallelTasksTool
/// to reduce code duplication and ensure consistent behavior.
pub struct SubAgentExecutor {
    /// Database client for execution record management
    db: Arc<DBClient>,
    /// Orchestrator for agent execution
    orchestrator: Arc<AgentOrchestrator>,
    /// Optional MCP manager for tool routing
    mcp_manager: Option<Arc<MCPManager>>,
    /// Optional app handle for event emission
    app_handle: Option<tauri::AppHandle>,
    /// Workflow ID for scoping
    workflow_id: String,
    /// Parent agent ID (caller of sub-agent tools)
    parent_agent_id: String,
}

impl SubAgentExecutor {
    /// Creates a new executor.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `orchestrator` - Agent orchestrator for execution
    /// * `mcp_manager` - Optional MCP manager for tool routing
    /// * `app_handle` - Optional app handle for event emission
    /// * `workflow_id` - Workflow ID for scoping
    /// * `parent_agent_id` - ID of parent agent calling sub-agent tools
    pub fn new(
        db: Arc<DBClient>,
        orchestrator: Arc<AgentOrchestrator>,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<tauri::AppHandle>,
        workflow_id: String,
        parent_agent_id: String,
    ) -> Self {
        Self {
            db,
            orchestrator,
            mcp_manager,
            app_handle,
            workflow_id,
            parent_agent_id,
        }
    }

    /// Checks that the caller is the primary agent.
    ///
    /// Sub-agents cannot use sub-agent tools (single level hierarchy).
    ///
    /// # Arguments
    /// * `is_primary` - Whether the caller is the primary agent
    /// * `operation` - Operation name for error message
    ///
    /// # Returns
    /// * `Ok(())` - If caller is primary
    /// * `Err(ToolError::PermissionDenied)` - If caller is sub-agent
    pub fn check_primary_permission(is_primary: bool, operation: &str) -> ToolResult<()> {
        if !is_primary {
            return Err(ToolError::PermissionDenied(format!(
                "Only the primary workflow agent can {}. Sub-agents cannot use this operation.",
                operation
            )));
        }
        Ok(())
    }

    /// Checks the sub-agent limit.
    ///
    /// Maximum 3 sub-agent operations per workflow.
    ///
    /// # Arguments
    /// * `current_count` - Current number of active sub-agents
    /// * `operation` - Operation name for error message
    ///
    /// # Returns
    /// * `Ok(())` - If under limit
    /// * `Err(ToolError::ValidationFailed)` - If limit exceeded
    pub fn check_limit(current_count: usize, operation: &str) -> ToolResult<()> {
        if current_count >= MAX_SUB_AGENTS {
            return Err(ToolError::ValidationFailed(format!(
                "Maximum {} sub-agent operations reached for {}. Current: {}. Complete existing operations first.",
                MAX_SUB_AGENTS, operation, current_count
            )));
        }
        Ok(())
    }

    /// Requests human-in-the-loop validation.
    ///
    /// # Arguments
    /// * `operation_type` - Type of sub-agent operation
    /// * `description` - Human-readable operation description
    /// * `details` - Additional operation details (JSON)
    ///
    /// # Returns
    /// * `Ok(())` - If approved (or validation skipped)
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_validation(
        &self,
        operation_type: SubAgentOperationType,
        description: &str,
        details: Value,
    ) -> ToolResult<()> {
        let validation_helper = ValidationHelper::new(self.db.clone(), self.app_handle.clone());
        let risk_level = ValidationHelper::determine_risk_level(&operation_type);

        validation_helper
            .request_validation(
                &self.workflow_id,
                operation_type,
                description,
                details,
                risk_level,
            )
            .await
    }

    /// Creates an execution record in the database.
    ///
    /// # Arguments
    /// * `child_agent_id` - Sub-agent ID
    /// * `child_agent_name` - Sub-agent name
    /// * `prompt` - Task prompt
    ///
    /// # Returns
    /// * `Ok(String)` - Execution record ID
    /// * `Err(ToolError)` - Database error
    pub async fn create_execution_record(
        &self,
        child_agent_id: &str,
        child_agent_name: &str,
        prompt: &str,
    ) -> ToolResult<String> {
        let execution_id = Uuid::new_v4().to_string();

        let mut execution_create = SubAgentExecutionCreate::new(
            self.workflow_id.clone(),
            self.parent_agent_id.clone(),
            child_agent_id.to_string(),
            child_agent_name.to_string(),
            prompt.to_string(),
        );
        execution_create.status = "running".to_string();

        self.db
            .create("sub_agent_execution", &execution_id, execution_create)
            .await
            .map_err(|e| {
                ToolError::DatabaseError(format!("Failed to create execution record: {}", e))
            })?;

        debug!(
            execution_id = %execution_id,
            child_agent_id = %child_agent_id,
            "Created sub-agent execution record"
        );

        Ok(execution_id)
    }

    /// Executes an agent and collects metrics.
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID to execute
    /// * `task` - Task to execute
    ///
    /// # Returns
    /// * `ExecutionResult` - Result with success, report, metrics, and optional error
    pub async fn execute_with_metrics(&self, agent_id: &str, task: Task) -> ExecutionResult {
        let start_time = Instant::now();

        let result = self
            .orchestrator
            .execute_with_mcp(agent_id, task, self.mcp_manager.clone())
            .await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(report) => {
                info!(
                    agent_id = %agent_id,
                    duration_ms = duration_ms,
                    "Sub-agent execution completed successfully"
                );
                ExecutionResult {
                    success: true,
                    report: report.content,
                    metrics: SubAgentMetrics {
                        duration_ms,
                        tokens_input: report.metrics.tokens_input as u64,
                        tokens_output: report.metrics.tokens_output as u64,
                    },
                    error_message: None,
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                error!(
                    agent_id = %agent_id,
                    duration_ms = duration_ms,
                    error = %error_msg,
                    "Sub-agent execution failed"
                );
                ExecutionResult {
                    success: false,
                    report: format!("# Sub-Agent Error\n\nExecution failed: {}", error_msg),
                    metrics: SubAgentMetrics {
                        duration_ms,
                        tokens_input: 0,
                        tokens_output: 0,
                    },
                    error_message: Some(error_msg),
                }
            }
        }
    }

    /// Updates the execution record with the result.
    ///
    /// # Arguments
    /// * `execution_id` - Execution record ID
    /// * `result` - Execution result with success, report, metrics
    pub async fn update_execution_record(&self, execution_id: &str, result: &ExecutionResult) {
        let status = if result.success {
            "completed"
        } else {
            "failed"
        };
        let result_summary = if result.report.len() > 200 {
            format!("{}...", &result.report[..200])
        } else {
            result.report.clone()
        };

        let result_summary_json =
            serde_json::to_string(&result_summary).unwrap_or_else(|_| "null".to_string());
        let error_message_json = result
            .error_message
            .as_ref()
            .map(|s| serde_json::to_string(s).unwrap_or_else(|_| "null".to_string()))
            .unwrap_or_else(|| "null".to_string());

        let update_query = format!(
            "UPDATE sub_agent_execution:`{}` SET \
             status = '{}', \
             duration_ms = {}, \
             tokens_input = {}, \
             tokens_output = {}, \
             result_summary = {}, \
             error_message = {}, \
             completed_at = time::now()",
            execution_id,
            status,
            result.metrics.duration_ms,
            result.metrics.tokens_input,
            result.metrics.tokens_output,
            result_summary_json,
            error_message_json,
        );

        if let Err(e) = self.db.execute(&update_query).await {
            warn!(
                execution_id = %execution_id,
                error = %e,
                "Failed to update execution record"
            );
        }
    }

    /// Emits a streaming event.
    ///
    /// # Arguments
    /// * `event_name` - Event name (e.g., events::WORKFLOW_STREAM)
    /// * `chunk` - Stream chunk to emit
    pub fn emit_event(&self, event_name: &str, chunk: &StreamChunk) {
        if let Some(ref handle) = self.app_handle {
            if let Err(e) = handle.emit(event_name, chunk) {
                warn!(
                    event = %event_name,
                    error = %e,
                    "Failed to emit sub-agent event"
                );
            }
        }
    }

    /// Emits execution start event.
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID
    /// * `agent_name` - Agent name
    /// * `prompt` - Task prompt
    pub fn emit_start_event(&self, agent_id: &str, agent_name: &str, prompt: &str) {
        let chunk = StreamChunk::sub_agent_start(
            self.workflow_id.clone(),
            agent_id.to_string(),
            agent_name.to_string(),
            self.parent_agent_id.clone(),
            prompt.to_string(),
        );
        self.emit_event(events::WORKFLOW_STREAM, &chunk);
    }

    /// Emits execution complete event.
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID
    /// * `agent_name` - Agent name
    /// * `result` - Execution result
    pub fn emit_complete_event(&self, agent_id: &str, agent_name: &str, result: &ExecutionResult) {
        if result.success {
            let chunk = StreamChunk::sub_agent_complete(
                self.workflow_id.clone(),
                agent_id.to_string(),
                agent_name.to_string(),
                self.parent_agent_id.clone(),
                result.report.clone(),
                SubAgentStreamMetrics {
                    duration_ms: result.metrics.duration_ms,
                    tokens_input: result.metrics.tokens_input,
                    tokens_output: result.metrics.tokens_output,
                },
            );
            self.emit_event(events::WORKFLOW_STREAM, &chunk);
        } else {
            let chunk = StreamChunk::sub_agent_error(
                self.workflow_id.clone(),
                agent_id.to_string(),
                agent_name.to_string(),
                self.parent_agent_id.clone(),
                result.error_message.clone().unwrap_or_default(),
                result.metrics.duration_ms,
            );
            self.emit_event(events::WORKFLOW_STREAM, &chunk);
        }
    }

    /// Generates a unique sub-agent ID.
    ///
    /// # Returns
    /// * `String` - Generated ID with "sub_" prefix
    pub fn generate_sub_agent_id() -> String {
        format!("sub_{}", Uuid::new_v4())
    }

    /// Gets the workflow ID.
    #[allow(dead_code)]
    pub fn workflow_id(&self) -> &str {
        &self.workflow_id
    }

    /// Gets the parent agent ID.
    #[allow(dead_code)]
    pub fn parent_agent_id(&self) -> &str {
        &self.parent_agent_id
    }

    /// Gets the database client.
    #[allow(dead_code)]
    pub fn db(&self) -> &Arc<DBClient> {
        &self.db
    }

    /// Gets the orchestrator.
    #[allow(dead_code)]
    pub fn orchestrator(&self) -> &Arc<AgentOrchestrator> {
        &self.orchestrator
    }

    /// Gets the MCP manager.
    #[allow(dead_code)]
    pub fn mcp_manager(&self) -> &Option<Arc<MCPManager>> {
        &self.mcp_manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_primary_permission_allowed() {
        let result = SubAgentExecutor::check_primary_permission(true, "spawn");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_primary_permission_denied() {
        let result = SubAgentExecutor::check_primary_permission(false, "spawn");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::PermissionDenied(msg) => {
                assert!(msg.contains("Only the primary"));
                assert!(msg.contains("spawn"));
            }
            _ => panic!("Expected PermissionDenied error"),
        }
    }

    #[test]
    fn test_check_limit_ok() {
        let result = SubAgentExecutor::check_limit(2, "spawn");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_limit_exceeded() {
        let result = SubAgentExecutor::check_limit(MAX_SUB_AGENTS, "spawn");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ValidationFailed(msg) => {
                assert!(msg.contains("Maximum"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_generate_sub_agent_id() {
        let id1 = SubAgentExecutor::generate_sub_agent_id();
        let id2 = SubAgentExecutor::generate_sub_agent_id();
        assert!(id1.starts_with("sub_"));
        assert!(id2.starts_with("sub_"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_execution_result_default() {
        let result = ExecutionResult::default();
        assert!(!result.success);
        assert!(result.report.is_empty());
        assert!(result.error_message.is_none());
        assert_eq!(result.metrics.duration_ms, 0);
        assert_eq!(result.metrics.tokens_input, 0);
        assert_eq!(result.metrics.tokens_output, 0);
    }
}
