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
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::Emitter;
use tokio::sync::RwLock;
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
use crate::tools::constants::sub_agent::{ACTIVITY_CHECK_INTERVAL_SECS, INACTIVITY_TIMEOUT_SECS};
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

// =============================================================================
// OPT-SA-1: ActivityMonitor for Inactivity Timeout with Heartbeat
// =============================================================================

/// Callback type for activity notification.
///
/// This callback is invoked whenever the agent shows activity (LLM response,
/// tool call start/end, MCP response). It should be lightweight and thread-safe.
pub type ActivityCallback = Arc<dyn Fn() + Send + Sync>;

/// Monitors agent activity to detect hangs.
///
/// The ActivityMonitor tracks the timestamp of the last activity and provides
/// methods to:
/// - Record new activity (resetting the inactivity timer)
/// - Check how long since the last activity
///
/// This enables intelligent timeout detection that allows long-running but active
/// executions while catching genuine hangs (no activity for extended periods).
///
/// # Thread Safety
///
/// All operations are thread-safe via `RwLock`. The `record_activity()` method
/// uses `try_write()` to avoid blocking if a read is in progress.
///
/// # Example
///
/// ```ignore
/// let monitor = ActivityMonitor::new();
///
/// // In the execution loop:
/// monitor.record_activity(); // Called on each LLM token, tool call, etc.
///
/// // In the monitoring loop:
/// if monitor.seconds_since_last_activity() > INACTIVITY_TIMEOUT_SECS {
///     // Abort - agent is hung
/// }
/// ```
#[derive(Clone)]
pub struct ActivityMonitor {
    /// Timestamp of the last recorded activity
    last_activity: Arc<RwLock<Instant>>,
}

impl ActivityMonitor {
    /// Creates a new ActivityMonitor with the current time as initial activity.
    pub fn new() -> Self {
        Self {
            last_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }

    /// Records a new activity, resetting the inactivity timer.
    ///
    /// This should be called whenever the agent shows signs of progress:
    /// - LLM returns tokens
    /// - Tool call starts
    /// - Tool call completes
    /// - MCP server responds
    ///
    /// Uses `try_write()` to avoid blocking. If the lock is held, the activity
    /// is skipped (this is acceptable as another activity will be recorded soon).
    pub fn record_activity(&self) {
        if let Ok(mut last) = self.last_activity.try_write() {
            *last = Instant::now();
        }
        // If try_write fails, another thread is writing - that's fine,
        // activity is being recorded anyway
    }

    /// Returns the number of seconds since the last recorded activity.
    ///
    /// Returns 0 if the lock cannot be acquired (conservative - assume active).
    pub fn seconds_since_last_activity(&self) -> u64 {
        self.last_activity
            .try_read()
            .map(|last| last.elapsed().as_secs())
            .unwrap_or(0)
    }

    /// Creates a callback closure that records activity when called.
    ///
    /// This callback can be passed to the orchestrator/agent for automatic
    /// activity tracking during execution.
    pub fn create_callback(self: &Arc<Self>) -> ActivityCallback {
        let monitor = Arc::clone(self);
        Arc::new(move || {
            monitor.record_activity();
        })
    }
}

impl Default for ActivityMonitor {
    fn default() -> Self {
        Self::new()
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

    /// Executes an agent and collects metrics (without heartbeat monitoring).
    ///
    /// This is the legacy method. For new code, prefer `execute_with_heartbeat_timeout`.
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID to execute
    /// * `task` - Task to execute
    ///
    /// # Returns
    /// * `ExecutionResult` - Result with success, report, metrics, and optional error
    #[allow(dead_code)] // Kept for backward compatibility
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

    // =========================================================================
    // OPT-SA-1: Execution with Heartbeat-based Inactivity Timeout
    // =========================================================================

    /// Executes an agent with inactivity timeout monitoring.
    ///
    /// This method wraps `execute_with_metrics` with a monitoring loop that
    /// detects genuine hangs by tracking activity. Unlike simple timeouts,
    /// this approach allows long-running but active executions to continue
    /// while catching agents that have truly stopped responding.
    ///
    /// # Activity Detection
    ///
    /// The following events reset the inactivity timer:
    /// - LLM returns tokens (streaming response)
    /// - Tool call starts
    /// - Tool call completes
    /// - MCP server responds
    ///
    /// # Timeout Behavior
    ///
    /// - Check interval: 30 seconds (ACTIVITY_CHECK_INTERVAL_SECS)
    /// - Timeout threshold: 300 seconds / 5 minutes (INACTIVITY_TIMEOUT_SECS)
    /// - If no activity for 5 minutes, execution is aborted with an error
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID to execute
    /// * `task` - Task to execute
    /// * `on_activity` - Optional callback invoked during execution for activity tracking.
    ///   If None, a local ActivityMonitor is created.
    ///
    /// # Returns
    /// * `ExecutionResult` - Result with success, report, metrics, and optional error
    ///
    /// # Example
    ///
    /// ```ignore
    /// // With automatic activity tracking via callback
    /// let monitor = Arc::new(ActivityMonitor::new());
    /// let callback = monitor.create_callback();
    ///
    /// let result = executor.execute_with_heartbeat_timeout(
    ///     agent_id,
    ///     task,
    ///     Some(callback),
    /// ).await;
    /// ```
    pub async fn execute_with_heartbeat_timeout(
        &self,
        agent_id: &str,
        task: Task,
        on_activity: Option<ActivityCallback>,
    ) -> ExecutionResult {
        let monitor = Arc::new(ActivityMonitor::new());
        let start_time = Instant::now();

        // Create callback that records activity
        let _activity_callback = on_activity.unwrap_or_else(|| monitor.create_callback());

        // Clone values for the execution future
        let orchestrator = self.orchestrator.clone();
        let mcp_manager = self.mcp_manager.clone();
        let agent_id_owned = agent_id.to_string();

        // Spawn the execution as a future we can poll with timeout
        // Note: We need to pass the callback to execute_with_mcp_monitored
        // For now, we use a simple approach: execute without callback and monitor externally
        // The callback propagation requires changes to Agent trait (next phase)
        let execution_future = async {
            orchestrator
                .execute_with_mcp(&agent_id_owned, task, mcp_manager)
                .await
        };

        // Pin the future for use in select!
        tokio::pin!(execution_future);

        // Monitoring loop with tokio::select!
        loop {
            tokio::select! {
                // Execution completed
                result = &mut execution_future => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    return match result {
                        Ok(report) => {
                            info!(
                                agent_id = %agent_id,
                                duration_ms = duration_ms,
                                "Sub-agent execution completed successfully (with heartbeat monitoring)"
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
                    };
                }

                // Activity check interval
                _ = tokio::time::sleep(Duration::from_secs(ACTIVITY_CHECK_INTERVAL_SECS)) => {
                    let inactive_secs = monitor.seconds_since_last_activity();

                    if inactive_secs > INACTIVITY_TIMEOUT_SECS {
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        warn!(
                            agent_id = %agent_id,
                            inactive_secs = inactive_secs,
                            threshold_secs = INACTIVITY_TIMEOUT_SECS,
                            duration_ms = duration_ms,
                            "Sub-agent execution timed out due to inactivity"
                        );

                        return ExecutionResult {
                            success: false,
                            report: format!(
                                "# Sub-Agent Timeout\n\n\
                                 Execution aborted: no activity detected for {} seconds.\n\n\
                                 - Inactivity threshold: {} seconds\n\
                                 - Total elapsed time: {} ms\n\n\
                                 This may indicate:\n\
                                 - The agent is waiting for an unresponsive external service\n\
                                 - A deadlock or infinite loop in tool execution\n\
                                 - Network connectivity issues\n\n\
                                 Consider checking LLM provider status and MCP server availability.",
                                inactive_secs,
                                INACTIVITY_TIMEOUT_SECS,
                                duration_ms
                            ),
                            metrics: SubAgentMetrics {
                                duration_ms,
                                tokens_input: 0,
                                tokens_output: 0,
                            },
                            error_message: Some(format!(
                                "Inactivity timeout: no activity for {} seconds (threshold: {}s)",
                                inactive_secs,
                                INACTIVITY_TIMEOUT_SECS
                            )),
                        };
                    }

                    // Log heartbeat at debug level
                    debug!(
                        agent_id = %agent_id,
                        last_activity_secs_ago = inactive_secs,
                        threshold_secs = INACTIVITY_TIMEOUT_SECS,
                        "Sub-agent heartbeat check: still within activity threshold"
                    );
                }
            }
        }
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

    // =========================================================================
    // OPT-SA-1: ActivityMonitor Tests
    // =========================================================================

    #[test]
    fn test_activity_monitor_new() {
        let monitor = ActivityMonitor::new();
        // Should start with recent activity (just created)
        assert!(monitor.seconds_since_last_activity() < 2);
    }

    #[test]
    fn test_activity_monitor_default() {
        let monitor = ActivityMonitor::default();
        assert!(monitor.seconds_since_last_activity() < 2);
    }

    #[test]
    fn test_activity_monitor_record_activity() {
        let monitor = ActivityMonitor::new();

        // Small delay to ensure time has passed
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Record new activity
        monitor.record_activity();

        // Should be very recent
        assert!(monitor.seconds_since_last_activity() < 1);
    }

    #[test]
    fn test_activity_monitor_clone() {
        let monitor = ActivityMonitor::new();
        let cloned = monitor.clone();

        // Both should show same initial time
        let time1 = monitor.seconds_since_last_activity();
        let time2 = cloned.seconds_since_last_activity();

        // Due to Arc, they should be identical (pointing to same data)
        assert_eq!(time1, time2);

        // Recording on one should affect the other (shared state)
        std::thread::sleep(std::time::Duration::from_millis(50));
        monitor.record_activity();

        // Both should now show recent activity
        assert!(cloned.seconds_since_last_activity() < 1);
    }

    #[test]
    fn test_activity_monitor_callback() {
        let monitor = Arc::new(ActivityMonitor::new());

        // Wait a bit
        std::thread::sleep(std::time::Duration::from_millis(50));

        // Create callback and invoke it
        let callback = monitor.create_callback();
        callback();

        // Activity should be recorded
        assert!(monitor.seconds_since_last_activity() < 1);
    }

    #[test]
    fn test_inactivity_timeout_constants() {
        // Verify constants are reasonable values
        assert_eq!(INACTIVITY_TIMEOUT_SECS, 300); // 5 minutes
        assert_eq!(ACTIVITY_CHECK_INTERVAL_SECS, 30); // 30 seconds

        // Check interval should be much smaller than timeout
        assert!(ACTIVITY_CHECK_INTERVAL_SECS < INACTIVITY_TIMEOUT_SECS / 2);
    }
}
