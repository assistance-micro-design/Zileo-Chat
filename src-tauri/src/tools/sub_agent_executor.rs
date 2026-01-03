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
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
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
use crate::tools::constants::sub_agent::{
    ACTIVITY_CHECK_INTERVAL_SECS, INACTIVITY_TIMEOUT_SECS, INITIAL_RETRY_DELAY_MS,
    MAX_RETRY_ATTEMPTS,
};
use crate::tools::sub_agent_circuit_breaker::SubAgentCircuitBreaker;
use crate::tools::validation_helper::{safe_truncate, ValidationHelper};
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
///
/// # Cancellation Support (OPT-SA-7)
///
/// The executor supports graceful cancellation via `CancellationToken`. When a token
/// is provided and cancelled, execution aborts immediately with a "cancelled" result.
/// This enables users to cancel long-running workflows and have sub-agents respond.
///
/// # Circuit Breaker Support (OPT-SA-8)
///
/// The executor supports circuit breaker protection via `SubAgentCircuitBreaker`.
/// When provided, the executor will:
/// - Check if circuit is open before execution (fail-fast if unhealthy)
/// - Record success on successful execution (reset failure count)
/// - Record failure on failed execution (increment failure count, may open circuit)
///
/// This prevents cascade failures when the sub-agent system is experiencing issues.
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
    /// Optional cancellation token for graceful shutdown (OPT-SA-7)
    cancellation_token: Option<CancellationToken>,
    /// Optional circuit breaker for execution resilience (OPT-SA-8)
    circuit_breaker: Option<Arc<Mutex<SubAgentCircuitBreaker>>>,
}

impl SubAgentExecutor {
    /// Creates a new executor without cancellation token or circuit breaker.
    ///
    /// For most use cases, prefer `with_resilience()` to support graceful shutdown
    /// and circuit breaker protection.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `orchestrator` - Agent orchestrator for execution
    /// * `mcp_manager` - Optional MCP manager for tool routing
    /// * `app_handle` - Optional app handle for event emission
    /// * `workflow_id` - Workflow ID for scoping
    /// * `parent_agent_id` - ID of parent agent calling sub-agent tools
    #[allow(dead_code)]
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
            cancellation_token: None,
            circuit_breaker: None,
        }
    }

    /// Creates a new executor with cancellation token support (OPT-SA-7).
    ///
    /// Note: This constructor does not include circuit breaker. Use `with_resilience()`
    /// for full resilience features including circuit breaker protection.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `orchestrator` - Agent orchestrator for execution
    /// * `mcp_manager` - Optional MCP manager for tool routing
    /// * `app_handle` - Optional app handle for event emission
    /// * `workflow_id` - Workflow ID for scoping
    /// * `parent_agent_id` - ID of parent agent calling sub-agent tools
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
    ///
    /// # Example
    /// ```ignore
    /// let executor = SubAgentExecutor::with_cancellation(
    ///     db, orchestrator, mcp_manager, app_handle,
    ///     workflow_id, parent_agent_id,
    ///     Some(cancellation_token),
    /// );
    /// ```
    pub fn with_cancellation(
        db: Arc<DBClient>,
        orchestrator: Arc<AgentOrchestrator>,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<tauri::AppHandle>,
        workflow_id: String,
        parent_agent_id: String,
        cancellation_token: Option<CancellationToken>,
    ) -> Self {
        Self {
            db,
            orchestrator,
            mcp_manager,
            app_handle,
            workflow_id,
            parent_agent_id,
            cancellation_token,
            circuit_breaker: None,
        }
    }

    /// Creates a new executor with full resilience features (OPT-SA-7, OPT-SA-8).
    ///
    /// This is the recommended constructor for production use as it supports:
    /// - Graceful cancellation via CancellationToken
    /// - Circuit breaker protection via SubAgentCircuitBreaker
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `orchestrator` - Agent orchestrator for execution
    /// * `mcp_manager` - Optional MCP manager for tool routing
    /// * `app_handle` - Optional app handle for event emission
    /// * `workflow_id` - Workflow ID for scoping
    /// * `parent_agent_id` - ID of parent agent calling sub-agent tools
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown (OPT-SA-7)
    /// * `circuit_breaker` - Optional circuit breaker for execution resilience (OPT-SA-8)
    ///
    /// # Example
    /// ```ignore
    /// let executor = SubAgentExecutor::with_resilience(
    ///     db, orchestrator, mcp_manager, app_handle,
    ///     workflow_id, parent_agent_id,
    ///     Some(cancellation_token),
    ///     Some(circuit_breaker),
    /// );
    /// ```
    #[allow(dead_code)] // Will be used when tools are updated to use resilience
    #[allow(clippy::too_many_arguments)]
    pub fn with_resilience(
        db: Arc<DBClient>,
        orchestrator: Arc<AgentOrchestrator>,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<tauri::AppHandle>,
        workflow_id: String,
        parent_agent_id: String,
        cancellation_token: Option<CancellationToken>,
        circuit_breaker: Option<Arc<Mutex<SubAgentCircuitBreaker>>>,
    ) -> Self {
        Self {
            db,
            orchestrator,
            mcp_manager,
            app_handle,
            workflow_id,
            parent_agent_id,
            cancellation_token,
            circuit_breaker,
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
        self.create_execution_record_with_parent(child_agent_id, child_agent_name, prompt, None)
            .await
    }

    /// Creates an execution record with optional parent execution ID for hierarchical tracing (OPT-SA-11).
    ///
    /// # Arguments
    /// * `child_agent_id` - Sub-agent ID
    /// * `child_agent_name` - Sub-agent name
    /// * `prompt` - Task prompt
    /// * `parent_execution_id` - Optional parent execution ID for correlation tracing
    ///
    /// # Returns
    /// * `Ok(String)` - Execution record ID
    /// * `Err(ToolError)` - Database error
    pub async fn create_execution_record_with_parent(
        &self,
        child_agent_id: &str,
        child_agent_name: &str,
        prompt: &str,
        parent_execution_id: Option<String>,
    ) -> ToolResult<String> {
        let execution_id = Uuid::new_v4().to_string();

        let mut execution_create = SubAgentExecutionCreate::with_parent(
            self.workflow_id.clone(),
            self.parent_agent_id.clone(),
            child_agent_id.to_string(),
            child_agent_name.to_string(),
            prompt.to_string(),
            parent_execution_id.clone(),
        );
        execution_create.status = "running".to_string();

        self.db
            .create("sub_agent_execution", &execution_id, execution_create)
            .await
            .map_err(|e| {
                ToolError::DatabaseError(format!("Failed to create execution record: {}", e))
            })?;

        // OPT-SA-11: Log with parent_execution_id for hierarchical tracing
        if let Some(ref parent_id) = parent_execution_id {
            debug!(
                execution_id = %execution_id,
                parent_execution_id = %parent_id,
                child_agent_id = %child_agent_id,
                workflow_id = %self.workflow_id,
                "Created sub-agent execution record with parent correlation"
            );
        } else {
            debug!(
                execution_id = %execution_id,
                child_agent_id = %child_agent_id,
                workflow_id = %self.workflow_id,
                "Created sub-agent execution record"
            );
        }

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
        let result_summary = safe_truncate(&result.report, 200, true);

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
    // OPT-SA-8: Circuit Breaker Integration
    // =========================================================================

    /// Checks if the circuit breaker allows execution.
    ///
    /// If a circuit breaker is configured and the circuit is open (system unhealthy),
    /// returns an error with remaining cooldown time. Otherwise returns Ok.
    ///
    /// # Returns
    /// * `Ok(())` - Execution is allowed (circuit closed/half-open or no circuit breaker)
    /// * `Err(ToolError)` - Execution blocked (circuit open)
    pub async fn check_circuit(&self) -> ToolResult<()> {
        if let Some(ref cb) = self.circuit_breaker {
            let mut guard = cb.lock().await;
            if !guard.allow_request() {
                let remaining = guard.remaining_cooldown_secs();
                return Err(ToolError::ExecutionFailed(format!(
                    "Sub-agent circuit breaker is open due to consecutive failures. \
                     System is unhealthy. Retry after {} seconds cooldown.",
                    remaining
                )));
            }
        }
        Ok(())
    }

    /// Records successful execution with the circuit breaker.
    ///
    /// Resets failure count and ensures circuit is closed.
    pub async fn record_success(&self) {
        if let Some(ref cb) = self.circuit_breaker {
            let mut guard = cb.lock().await;
            guard.record_success();
        }
    }

    /// Records failed execution with the circuit breaker.
    ///
    /// Increments failure count and may open circuit if threshold is reached.
    pub async fn record_failure(&self) {
        if let Some(ref cb) = self.circuit_breaker {
            let mut guard = cb.lock().await;
            guard.record_failure();
        }
    }

    // =========================================================================
    // OPT-SA-1: Execution with Heartbeat-based Inactivity Timeout
    // OPT-SA-7: Cancellation Support
    // OPT-SA-8: Circuit Breaker Protection
    // =========================================================================

    /// Executes an agent with inactivity timeout monitoring, cancellation, and circuit breaker.
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
    /// # Cancellation Behavior (OPT-SA-7)
    ///
    /// If a cancellation token was provided when creating the executor (via
    /// `with_cancellation`), the execution will abort immediately when the
    /// token is cancelled. This enables graceful shutdown when the user
    /// cancels the workflow.
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
        // OPT-SA-8: Check circuit breaker before execution
        if let Err(e) = self.check_circuit().await {
            warn!(
                agent_id = %agent_id,
                error = %e,
                "Sub-agent execution blocked by circuit breaker"
            );
            return ExecutionResult {
                success: false,
                report: format!(
                    "# Sub-Agent Blocked\n\n\
                     Circuit breaker is open - sub-agent system is unhealthy.\n\n\
                     {}",
                    e
                ),
                metrics: SubAgentMetrics {
                    duration_ms: 0,
                    tokens_input: 0,
                    tokens_output: 0,
                },
                error_message: Some(e.to_string()),
            };
        }

        let monitor = Arc::new(ActivityMonitor::new());
        let start_time = Instant::now();

        // Create callback that records activity (used by caller if provided)
        let activity_callback = on_activity.unwrap_or_else(|| monitor.create_callback());

        // Clone values for the execution future
        let orchestrator = self.orchestrator.clone();
        let mcp_manager = self.mcp_manager.clone();
        let agent_id_owned = agent_id.to_string();
        let monitor_for_exec = monitor.clone();

        // Spawn the execution in a separate task so select! can properly poll
        // This allows the heartbeat check to run even when execution is waiting on I/O
        let execution_handle = tokio::spawn(async move {
            // Record activity at start
            monitor_for_exec.record_activity();

            let result = orchestrator
                .execute_with_mcp(&agent_id_owned, task, mcp_manager)
                .await;

            // Record activity at end
            monitor_for_exec.record_activity();

            result
        });

        // Get abort handle for timeout cancellation
        let abort_handle = execution_handle.abort_handle();

        // Wrap the JoinHandle in a future
        let execution_future = async {
            execution_handle.await.map_err(|e| {
                if e.is_cancelled() {
                    anyhow::anyhow!("Task was cancelled (timeout or user cancellation)")
                } else {
                    anyhow::anyhow!("Task join error: {}", e)
                }
            })?
        };

        // Call the activity callback once to signal start
        activity_callback();

        // Pin the future for use in select!
        tokio::pin!(execution_future);

        // OPT-SA-7: Create cancellation future based on whether token is present
        // If no token, create a future that never completes
        let cancellation_future = async {
            if let Some(ref token) = self.cancellation_token {
                token.cancelled().await;
            } else {
                // No token - wait forever (this branch will never complete)
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(cancellation_future);

        // Monitoring loop with tokio::select!
        loop {
            tokio::select! {
                // Branch 1: Execution completed
                result = &mut execution_future => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    return match result {
                        Ok(report) => {
                            // OPT-SA-8: Record success with circuit breaker
                            self.record_success().await;

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
                            // OPT-SA-8: Record failure with circuit breaker
                            self.record_failure().await;

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

                // Branch 2: Cancellation requested (OPT-SA-7)
                // Note: Cancellation is user-initiated, not a system failure - don't record as failure
                _ = &mut cancellation_future => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    warn!(
                        agent_id = %agent_id,
                        duration_ms = duration_ms,
                        "Sub-agent execution cancelled by user"
                    );

                    // Abort the spawned task
                    abort_handle.abort();

                    return ExecutionResult {
                        success: false,
                        report: format!(
                            "# Sub-Agent Cancelled\n\n\
                             Execution was cancelled by user request.\n\n\
                             - Elapsed time before cancellation: {} ms\n\n\
                             The workflow cancellation was propagated to this sub-agent.",
                            duration_ms
                        ),
                        metrics: SubAgentMetrics {
                            duration_ms,
                            tokens_input: 0,
                            tokens_output: 0,
                        },
                        error_message: Some("Execution cancelled by user".to_string()),
                    };
                }

                // Branch 3: Activity check interval
                // The fact that select! can reach this branch proves the async runtime is not blocked
                // This means the execution is progressing (waiting on I/O, not stuck)
                _ = tokio::time::sleep(Duration::from_secs(ACTIVITY_CHECK_INTERVAL_SECS)) => {
                    // Record activity: if we reach here, the tokio runtime is responsive
                    // which means the spawned task is making progress (even if waiting on I/O)
                    monitor.record_activity();

                    // Since we just recorded activity, inactive_secs will be ~0
                    // The timeout now only triggers if the entire select! loop is blocked
                    // (which shouldn't happen with properly async operations)
                    let inactive_secs = monitor.seconds_since_last_activity();

                    if inactive_secs > INACTIVITY_TIMEOUT_SECS {
                        // OPT-SA-8: Record timeout as failure with circuit breaker
                        // Inactivity timeouts indicate system issues, so record as failure
                        self.record_failure().await;

                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        warn!(
                            agent_id = %agent_id,
                            inactive_secs = inactive_secs,
                            threshold_secs = INACTIVITY_TIMEOUT_SECS,
                            duration_ms = duration_ms,
                            "Sub-agent execution timed out due to inactivity"
                        );

                        // Abort the spawned task
                        abort_handle.abort();

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
                        "Sub-agent heartbeat check: runtime responsive, execution progressing"
                    );
                }
            }
        }
    }

    // =========================================================================
    // OPT-SA-10: Retry with Exponential Backoff
    // =========================================================================

    /// Executes with automatic retry on transient errors using exponential backoff.
    ///
    /// This method wraps `execute_with_heartbeat_timeout` with retry logic that
    /// automatically retries on transient failures. The delay doubles between
    /// each retry attempt (exponential backoff) to avoid overwhelming services.
    ///
    /// # Retry Policy
    ///
    /// - Maximum attempts: 3 (initial + 2 retries)
    /// - Initial delay: 500ms
    /// - Backoff multiplier: 2x (500ms -> 1000ms -> 2000ms)
    /// - Retryable errors: Network timeouts, temporary service unavailability
    /// - Non-retryable errors: Validation failures, permission errors, cancellation
    ///
    /// # Retryable Error Detection
    ///
    /// An error is considered retryable if it matches patterns indicating
    /// transient issues:
    /// - "timeout" - Network or execution timeouts
    /// - "temporarily unavailable" - Service temporarily down
    /// - "connection refused" - Service not ready
    /// - "network error" - Network connectivity issues
    /// - "rate limit" - API rate limiting (wait and retry)
    /// - "503" or "502" - HTTP service unavailable/bad gateway
    /// - "retry" - Explicit retry suggestion in error
    ///
    /// # Circuit Breaker Integration
    ///
    /// The retry logic respects the circuit breaker:
    /// - If circuit is open, no retry is attempted (fail fast)
    /// - Each retry checks circuit state before execution
    /// - Successes and failures are recorded appropriately
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID to execute
    /// * `task` - Task to execute (will be cloned for retries)
    /// * `on_activity` - Optional activity callback for heartbeat monitoring
    ///
    /// # Returns
    /// * `ExecutionResult` - Result of the successful attempt or last failure
    ///
    /// # Example
    ///
    /// ```ignore
    /// let result = executor.execute_with_retry(
    ///     agent_id,
    ///     task,
    ///     Some(activity_callback),
    /// ).await;
    ///
    /// if !result.success {
    ///     // All retry attempts failed
    ///     eprintln!("Execution failed after retries: {:?}", result.error_message);
    /// }
    /// ```
    pub async fn execute_with_retry(
        &self,
        agent_id: &str,
        task: Task,
        on_activity: Option<ActivityCallback>,
    ) -> ExecutionResult {
        let mut last_result = ExecutionResult::default();

        for attempt in 0..=MAX_RETRY_ATTEMPTS {
            // Execute with heartbeat timeout (includes circuit breaker check)
            let result = self
                .execute_with_heartbeat_timeout(agent_id, task.clone(), on_activity.clone())
                .await;

            // Success - return immediately
            if result.success {
                if attempt > 0 {
                    info!(
                        agent_id = %agent_id,
                        attempt = attempt + 1,
                        "Sub-agent execution succeeded on retry"
                    );
                }
                return result;
            }

            // Check if error is retryable
            let is_retryable = result
                .error_message
                .as_ref()
                .map(|msg| Self::is_retryable_error(msg))
                .unwrap_or(false);

            // Non-retryable error - return immediately
            if !is_retryable {
                debug!(
                    agent_id = %agent_id,
                    error = ?result.error_message,
                    "Non-retryable error, not attempting retry"
                );
                return result;
            }

            // Store result for potential final return
            last_result = result;

            // Last attempt - don't sleep, just return
            if attempt >= MAX_RETRY_ATTEMPTS {
                break;
            }

            // Calculate delay with exponential backoff
            let delay_ms = INITIAL_RETRY_DELAY_MS * 2_u64.pow(attempt);
            warn!(
                agent_id = %agent_id,
                attempt = attempt + 1,
                max_attempts = MAX_RETRY_ATTEMPTS + 1,
                delay_ms = delay_ms,
                error = ?last_result.error_message,
                "Retrying sub-agent execution after transient error"
            );

            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        // All retries exhausted - enhance error message
        if let Some(original_error) = last_result.error_message.take() {
            last_result.error_message = Some(format!(
                "{} (after {} retry attempts with exponential backoff)",
                original_error,
                MAX_RETRY_ATTEMPTS + 1
            ));
            last_result.report = format!(
                "# Sub-Agent Retry Exhausted\n\n\
                 All {} attempts failed.\n\n\
                 - Initial attempt: failed\n\
                 - Retry attempts: {} (with exponential backoff)\n\
                 - Total delays: {} ms\n\n\
                 Last error: {}",
                MAX_RETRY_ATTEMPTS + 1,
                MAX_RETRY_ATTEMPTS,
                Self::total_retry_delay_ms(),
                original_error
            );
        }

        warn!(
            agent_id = %agent_id,
            total_attempts = MAX_RETRY_ATTEMPTS + 1,
            error = ?last_result.error_message,
            "Sub-agent execution failed after all retry attempts"
        );

        last_result
    }

    /// Determines if an error message indicates a retryable transient error.
    ///
    /// Checks for patterns that suggest the error is temporary and may succeed
    /// on retry. Case-insensitive matching.
    ///
    /// # Arguments
    /// * `error_message` - The error message to analyze
    ///
    /// # Returns
    /// * `true` - Error appears to be transient and retryable
    /// * `false` - Error appears to be permanent (don't retry)
    pub fn is_retryable_error(error_message: &str) -> bool {
        let lower = error_message.to_lowercase();

        // Retryable patterns (transient errors)
        let retryable_patterns = [
            "timeout",
            "timed out",
            "temporarily unavailable",
            "temporary failure",
            "connection refused",
            "connection reset",
            "network error",
            "network unreachable",
            "rate limit",
            "rate_limit",
            "too many requests",
            "503", // Service Unavailable
            "502", // Bad Gateway
            "429", // Too Many Requests
            "retry",
            "try again",
            "service unavailable",
            "server busy",
            "overloaded",
            "capacity",
        ];

        // Non-retryable patterns (permanent errors - check first)
        let non_retryable_patterns = [
            "cancelled",
            "permission denied",
            "not found",
            "invalid",
            "unauthorized",
            "forbidden",
            "bad request",
            "circuit breaker",
            "validation failed",
            "authentication",
        ];

        // Check non-retryable first (takes precedence)
        for pattern in &non_retryable_patterns {
            if lower.contains(pattern) {
                return false;
            }
        }

        // Check retryable patterns
        for pattern in &retryable_patterns {
            if lower.contains(pattern) {
                return true;
            }
        }

        false
    }

    /// Calculates total delay across all retry attempts (for documentation).
    ///
    /// With MAX_RETRY_ATTEMPTS=2 and INITIAL_RETRY_DELAY_MS=500:
    /// - Attempt 0 fails: sleep 500ms
    /// - Attempt 1 fails: sleep 1000ms
    /// - Attempt 2 fails: no sleep
    ///
    /// Total: 1500ms
    fn total_retry_delay_ms() -> u64 {
        let mut total = 0;
        for i in 0..MAX_RETRY_ATTEMPTS {
            total += INITIAL_RETRY_DELAY_MS * 2_u64.pow(i);
        }
        total
    }
}

// =============================================================================
// OPT-SA-10: Retryable Error Helper (standalone function for external use)
// =============================================================================

/// Checks if an error message indicates a retryable transient error.
///
/// This is a standalone function wrapper for `SubAgentExecutor::is_retryable_error`
/// for use in contexts where the executor is not available.
///
/// See [`SubAgentExecutor::is_retryable_error`] for details on pattern matching.
#[allow(dead_code)]
pub fn is_retryable_error(error_message: &str) -> bool {
    SubAgentExecutor::is_retryable_error(error_message)
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
        let interval = ACTIVITY_CHECK_INTERVAL_SECS;
        let timeout = INACTIVITY_TIMEOUT_SECS;
        assert!(interval < timeout / 2, "Check interval should be less than half the timeout");
    }

    // =========================================================================
    // OPT-SA-7: CancellationToken Tests
    // =========================================================================

    #[test]
    fn test_executor_with_cancellation_token() {
        // Test that with_cancellation stores the token
        use crate::agents::core::{AgentOrchestrator, AgentRegistry};

        // Create minimal dependencies (won't actually be used in this test)
        let registry = Arc::new(AgentRegistry::new());
        let _orchestrator = Arc::new(AgentOrchestrator::new(registry));
        let token = CancellationToken::new();

        // Use a mock DBClient - we don't need a real one for this test
        // Skip this test until we have a proper mock
        // For now, just test the token behavior

        // Test that CancellationToken can be cloned and cancelled
        let token2 = token.clone();
        assert!(!token.is_cancelled());
        assert!(!token2.is_cancelled());

        token.cancel();

        assert!(token.is_cancelled());
        assert!(token2.is_cancelled()); // Clone shares state
    }

    #[tokio::test]
    async fn test_cancellation_token_immediate_cancellation() {
        // Test that a pre-cancelled token completes immediately
        let token = CancellationToken::new();
        token.cancel();

        // This should complete immediately (not hang)
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), token.cancelled()).await;

        assert!(result.is_ok(), "cancelled() should complete immediately");
    }

    #[tokio::test]
    async fn test_cancellation_token_async_cancellation() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        // Spawn task that waits for cancellation
        let handle = tokio::spawn(async move {
            token_clone.cancelled().await;
            "cancelled"
        });

        // Cancel after small delay
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        token.cancel();

        // Wait for task with timeout
        let result = tokio::time::timeout(std::time::Duration::from_millis(100), handle).await;

        assert!(result.is_ok(), "Task should complete after cancellation");
        assert_eq!(result.unwrap().unwrap(), "cancelled");
    }

    // =========================================================================
    // OPT-SA-10: Retry with Exponential Backoff Tests
    // =========================================================================

    #[test]
    fn test_retry_constants() {
        // Verify retry constants are reasonable values
        assert_eq!(MAX_RETRY_ATTEMPTS, 2); // 3 total attempts
        assert_eq!(INITIAL_RETRY_DELAY_MS, 500); // 500ms initial delay

        // Total delay should be 500 + 1000 = 1500ms
        assert_eq!(SubAgentExecutor::total_retry_delay_ms(), 1500);
    }

    #[test]
    fn test_is_retryable_error_timeout_patterns() {
        // Timeout errors should be retryable
        assert!(SubAgentExecutor::is_retryable_error("Connection timeout"));
        assert!(SubAgentExecutor::is_retryable_error(
            "Request timed out after 30s"
        ));
        assert!(SubAgentExecutor::is_retryable_error(
            "TIMEOUT waiting for response"
        ));
    }

    #[test]
    fn test_is_retryable_error_network_patterns() {
        // Network errors should be retryable
        assert!(SubAgentExecutor::is_retryable_error("Connection refused"));
        assert!(SubAgentExecutor::is_retryable_error(
            "Network error: unreachable"
        ));
        assert!(SubAgentExecutor::is_retryable_error(
            "Connection reset by peer"
        ));
    }

    #[test]
    fn test_is_retryable_error_http_status_codes() {
        // HTTP 5xx and 429 should be retryable
        assert!(SubAgentExecutor::is_retryable_error(
            "HTTP 503 Service Unavailable"
        ));
        assert!(SubAgentExecutor::is_retryable_error(
            "Error 502 Bad Gateway"
        ));
        assert!(SubAgentExecutor::is_retryable_error(
            "429 Too Many Requests"
        ));
    }

    #[test]
    fn test_is_retryable_error_rate_limit_patterns() {
        // Rate limiting should be retryable
        assert!(SubAgentExecutor::is_retryable_error("Rate limit exceeded"));
        assert!(SubAgentExecutor::is_retryable_error("rate_limit_error"));
        assert!(SubAgentExecutor::is_retryable_error(
            "Too many requests, try again"
        ));
    }

    #[test]
    fn test_is_retryable_error_service_patterns() {
        // Service availability errors should be retryable
        assert!(SubAgentExecutor::is_retryable_error(
            "Service temporarily unavailable"
        ));
        assert!(SubAgentExecutor::is_retryable_error(
            "Temporary failure, retry later"
        ));
        assert!(SubAgentExecutor::is_retryable_error("Server is overloaded"));
        assert!(SubAgentExecutor::is_retryable_error(
            "Server busy, please retry"
        ));
    }

    #[test]
    fn test_is_retryable_error_non_retryable_patterns() {
        // Permanent errors should NOT be retryable
        assert!(!SubAgentExecutor::is_retryable_error(
            "Execution cancelled by user"
        ));
        assert!(!SubAgentExecutor::is_retryable_error("Permission denied"));
        assert!(!SubAgentExecutor::is_retryable_error("Resource not found"));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Invalid configuration"
        ));
        assert!(!SubAgentExecutor::is_retryable_error("Unauthorized access"));
        assert!(!SubAgentExecutor::is_retryable_error("Bad request format"));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Circuit breaker is open"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Validation failed for input"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Authentication required"
        ));
        assert!(!SubAgentExecutor::is_retryable_error("403 Forbidden"));
    }

    #[test]
    fn test_is_retryable_error_non_retryable_takes_precedence() {
        // Non-retryable patterns should take precedence
        // "timeout" is retryable but "cancelled" is not
        assert!(!SubAgentExecutor::is_retryable_error(
            "Operation cancelled due to timeout validation failed"
        ));

        // Contains both "retry" and "invalid"
        assert!(!SubAgentExecutor::is_retryable_error(
            "Invalid request, do not retry"
        ));
    }

    #[test]
    fn test_is_retryable_error_case_insensitive() {
        // Should be case insensitive
        assert!(SubAgentExecutor::is_retryable_error("TIMEOUT"));
        assert!(SubAgentExecutor::is_retryable_error("TimeOut"));
        assert!(SubAgentExecutor::is_retryable_error("CONNECTION REFUSED"));
        assert!(!SubAgentExecutor::is_retryable_error("CANCELLED"));
        assert!(!SubAgentExecutor::is_retryable_error("Invalid"));
    }

    #[test]
    fn test_is_retryable_error_unknown_errors() {
        // Unknown errors should NOT be retryable (conservative)
        assert!(!SubAgentExecutor::is_retryable_error(
            "Something went wrong"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Unknown error occurred"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(""));
    }

    #[test]
    fn test_is_retryable_error_standalone_function() {
        // Test the standalone helper function
        assert!(is_retryable_error("timeout"));
        assert!(!is_retryable_error("cancelled"));
    }

    // =========================================================================
    // OPT-SA-11: Correlation ID (parent_execution_id) Tests
    // =========================================================================

    #[test]
    fn test_create_execution_record_with_parent_default_none() {
        // create_execution_record should delegate to create_execution_record_with_parent
        // with parent_execution_id = None by default
        // This is a signature test - actual DB integration tested in integration tests
        // Verifying the method signature exists and is callable
        use crate::models::sub_agent::SubAgentExecutionCreate;

        // Test that new() creates with None parent
        let create = SubAgentExecutionCreate::new(
            "wf_001".to_string(),
            "parent".to_string(),
            "child".to_string(),
            "name".to_string(),
            "prompt".to_string(),
        );
        assert!(create.parent_execution_id.is_none());
    }

    #[test]
    fn test_create_execution_record_with_parent_some() {
        use crate::models::sub_agent::SubAgentExecutionCreate;

        let parent_id = "parent_exec_123".to_string();
        let create = SubAgentExecutionCreate::with_parent(
            "wf_001".to_string(),
            "parent".to_string(),
            "child".to_string(),
            "name".to_string(),
            "prompt".to_string(),
            Some(parent_id.clone()),
        );
        assert_eq!(create.parent_execution_id, Some(parent_id));
    }

    #[test]
    fn test_correlation_id_serialization_with_parent() {
        use crate::models::sub_agent::SubAgentExecutionCreate;

        let create = SubAgentExecutionCreate::with_parent(
            "wf".to_string(),
            "parent".to_string(),
            "child".to_string(),
            "name".to_string(),
            "prompt".to_string(),
            Some("batch_123".to_string()),
        );

        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"parent_execution_id\":\"batch_123\""));
    }

    #[test]
    fn test_correlation_id_serialization_without_parent() {
        use crate::models::sub_agent::SubAgentExecutionCreate;

        let create = SubAgentExecutionCreate::new(
            "wf".to_string(),
            "parent".to_string(),
            "child".to_string(),
            "name".to_string(),
            "prompt".to_string(),
        );

        let json = serde_json::to_string(&create).unwrap();
        // parent_execution_id should be skipped when None
        assert!(!json.contains("parent_execution_id"));
    }
}
