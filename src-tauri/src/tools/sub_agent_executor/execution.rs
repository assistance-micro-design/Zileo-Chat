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

//! Execution engine for sub-agents with heartbeat monitoring, retry, and circuit breaker.
//!
//! Provides:
//! - Heartbeat-based inactivity timeout detection
//! - Cancellation support via `CancellationToken`
//! - Circuit breaker protection against cascade failures
//! - Retry with exponential backoff for transient errors

use std::sync::Arc;
use std::time::{Duration, Instant};

use tracing::{debug, error, info, warn};

use crate::agents::core::agent::{Report, Task};
use crate::models::sub_agent::SubAgentMetrics;
use crate::tools::constants::sub_agent::{
    ACTIVITY_CHECK_INTERVAL_SECS, INACTIVITY_TIMEOUT_SECS, INITIAL_RETRY_DELAY_MS,
    MAX_RETRY_ATTEMPTS,
};
use crate::tools::{ToolError, ToolResult};

use super::activity_monitor::ActivityCallback;
use super::activity_monitor::ActivityMonitor;
use super::{ExecutionResult, SubAgentExecutor};

impl SubAgentExecutor {
    // =========================================================================
    // Circuit Breaker Integration
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
    // Execution with Heartbeat-based Inactivity Timeout
    // =========================================================================

    /// Executes an agent with inactivity timeout monitoring, cancellation, and circuit breaker.
    ///
    /// Runs agent execution with a monitoring loop that
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
    /// # Cancellation Behavior
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
    pub async fn execute_with_heartbeat_timeout(
        &self,
        agent_id: &str,
        task: Task,
        on_activity: Option<ActivityCallback>,
    ) -> ExecutionResult {
        // Check circuit breaker before execution
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
                tool_executions: Vec::new(),
                reasoning_steps: Vec::new(),
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
        let execution_handle = tokio::spawn(async move {
            monitor_for_exec.record_activity();
            let result = orchestrator
                .execute_with_mcp(&agent_id_owned, task, mcp_manager, None)
                .await;
            monitor_for_exec.record_activity();
            result
        });

        let abort_handle = execution_handle.abort_handle();

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

        tokio::pin!(execution_future);

        // Create cancellation future based on whether token is present
        let cancellation_future = async {
            if let Some(ref token) = self.cancellation_token {
                token.cancelled().await;
            } else {
                std::future::pending::<()>().await;
            }
        };
        tokio::pin!(cancellation_future);

        // Monitoring loop with tokio::select!
        loop {
            tokio::select! {
                result = &mut execution_future => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    return self.handle_execution_result(agent_id, result, duration_ms).await;
                }

                _ = &mut cancellation_future => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    abort_handle.abort();
                    return Self::build_cancelled_result(agent_id, duration_ms);
                }

                _ = tokio::time::sleep(Duration::from_secs(ACTIVITY_CHECK_INTERVAL_SECS)) => {
                    monitor.record_activity();
                    let inactive_secs = monitor.seconds_since_last_activity();

                    if inactive_secs > INACTIVITY_TIMEOUT_SECS {
                        self.record_failure().await;
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        abort_handle.abort();
                        return Self::build_timeout_result(agent_id, inactive_secs, duration_ms);
                    }

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

    /// Handles the result from a completed execution future.
    async fn handle_execution_result(
        &self,
        agent_id: &str,
        result: Result<Report, anyhow::Error>,
        duration_ms: u64,
    ) -> ExecutionResult {
        match result {
            Ok(report) => {
                self.record_success().await;
                info!(
                    agent_id = %agent_id,
                    duration_ms = duration_ms,
                    tool_executions = report.metrics.tool_executions.len(),
                    reasoning_steps = report.metrics.reasoning_steps.len(),
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
                    tool_executions: report.metrics.tool_executions,
                    reasoning_steps: report.metrics.reasoning_steps,
                }
            }
            Err(e) => {
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
                    tool_executions: Vec::new(),
                    reasoning_steps: Vec::new(),
                }
            }
        }
    }

    /// Builds an `ExecutionResult` for a cancelled execution.
    fn build_cancelled_result(agent_id: &str, duration_ms: u64) -> ExecutionResult {
        warn!(
            agent_id = %agent_id,
            duration_ms = duration_ms,
            "Sub-agent execution cancelled by user"
        );
        ExecutionResult {
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
            tool_executions: Vec::new(),
            reasoning_steps: Vec::new(),
        }
    }

    /// Builds an `ExecutionResult` for an inactivity timeout.
    fn build_timeout_result(
        agent_id: &str,
        inactive_secs: u64,
        duration_ms: u64,
    ) -> ExecutionResult {
        warn!(
            agent_id = %agent_id,
            inactive_secs = inactive_secs,
            threshold_secs = INACTIVITY_TIMEOUT_SECS,
            duration_ms = duration_ms,
            "Sub-agent execution timed out due to inactivity"
        );
        ExecutionResult {
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
                inactive_secs, INACTIVITY_TIMEOUT_SECS, duration_ms
            ),
            metrics: SubAgentMetrics {
                duration_ms,
                tokens_input: 0,
                tokens_output: 0,
            },
            error_message: Some(format!(
                "Inactivity timeout: no activity for {} seconds (threshold: {}s)",
                inactive_secs, INACTIVITY_TIMEOUT_SECS
            )),
            tool_executions: Vec::new(),
            reasoning_steps: Vec::new(),
        }
    }

    // =========================================================================
    // Retry with Exponential Backoff
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
    /// # Arguments
    /// * `agent_id` - Agent ID to execute
    /// * `task` - Task to execute (will be cloned for retries)
    /// * `on_activity` - Optional activity callback for heartbeat monitoring
    ///
    /// # Returns
    /// * `ExecutionResult` - Result of the successful attempt or last failure
    pub async fn execute_with_retry(
        &self,
        agent_id: &str,
        task: Task,
        on_activity: Option<ActivityCallback>,
    ) -> ExecutionResult {
        let mut last_result = ExecutionResult::default();

        for attempt in 0..=MAX_RETRY_ATTEMPTS {
            let result = self
                .execute_with_heartbeat_timeout(agent_id, task.clone(), on_activity.clone())
                .await;

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

            let is_retryable = result
                .error_message
                .as_ref()
                .map(|msg| Self::is_retryable_error(msg))
                .unwrap_or(false);

            if !is_retryable {
                debug!(
                    agent_id = %agent_id,
                    error = ?result.error_message,
                    "Non-retryable error, not attempting retry"
                );
                return result;
            }

            last_result = result;

            if attempt >= MAX_RETRY_ATTEMPTS {
                break;
            }

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
            "503",
            "502",
            "429",
            "retry",
            "try again",
            "service unavailable",
            "server busy",
            "overloaded",
            "capacity",
        ];

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::agent::{ReasoningSource, ReasoningStepData, ToolExecutionData};
    use crate::tools::constants::sub_agent::{
        ACTIVITY_CHECK_INTERVAL_SECS, INACTIVITY_TIMEOUT_SECS, INITIAL_RETRY_DELAY_MS,
        MAX_RETRY_ATTEMPTS,
    };
    use tokio_util::sync::CancellationToken;

    #[test]
    fn test_execution_result_default() {
        let result = ExecutionResult::default();
        assert!(!result.success);
        assert!(result.report.is_empty());
        assert!(result.error_message.is_none());
        assert_eq!(result.metrics.duration_ms, 0);
        assert_eq!(result.metrics.tokens_input, 0);
        assert_eq!(result.metrics.tokens_output, 0);
        assert!(result.tool_executions.is_empty());
        assert!(result.reasoning_steps.is_empty());
    }

    #[test]
    fn test_execution_result_preserves_tool_executions() {
        let tool_exec = ToolExecutionData {
            tool_type: "mcp".to_string(),
            tool_name: "find_symbol".to_string(),
            server_name: Some("serena".to_string()),
            input_params: serde_json::json!({"name": "MyClass"}),
            output_result: serde_json::json!({"found": true}),
            success: true,
            error_message: None,
            duration_ms: 150,
            iteration: 0,
            sequence: 0,
        };
        let result = ExecutionResult {
            success: true,
            report: "Done".to_string(),
            metrics: SubAgentMetrics {
                duration_ms: 1000,
                tokens_input: 500,
                tokens_output: 200,
            },
            error_message: None,
            tool_executions: vec![tool_exec],
            reasoning_steps: Vec::new(),
        };
        assert_eq!(result.tool_executions.len(), 1);
        assert_eq!(result.tool_executions[0].tool_name, "find_symbol");
        assert_eq!(
            result.tool_executions[0].server_name,
            Some("serena".to_string())
        );
    }

    #[test]
    fn test_execution_result_preserves_reasoning_steps() {
        let step = ReasoningStepData {
            content: "Analyzing the codebase structure".to_string(),
            duration_ms: 300,
            sequence: 0,
            source: ReasoningSource::AgentFlow,
        };
        let result = ExecutionResult {
            success: true,
            report: "Done".to_string(),
            metrics: SubAgentMetrics {
                duration_ms: 1000,
                tokens_input: 500,
                tokens_output: 200,
            },
            error_message: None,
            tool_executions: Vec::new(),
            reasoning_steps: vec![step],
        };
        assert_eq!(result.reasoning_steps.len(), 1);
        assert_eq!(
            result.reasoning_steps[0].content,
            "Analyzing the codebase structure"
        );
        assert_eq!(result.reasoning_steps[0].duration_ms, 300);
    }

    #[test]
    fn test_inactivity_timeout_constants() {
        assert_eq!(INACTIVITY_TIMEOUT_SECS, 300);
        assert_eq!(ACTIVITY_CHECK_INTERVAL_SECS, 30);
        const _: () = assert!(
            ACTIVITY_CHECK_INTERVAL_SECS < INACTIVITY_TIMEOUT_SECS / 2,
            "Check interval should be less than half the timeout"
        );
    }

    // =========================================================================
    // CancellationToken Tests
    // =========================================================================

    #[test]
    fn test_cancellation_token_clone_shares_state() {
        let token = CancellationToken::new();
        let token2 = token.clone();
        assert!(!token.is_cancelled());
        assert!(!token2.is_cancelled());

        token.cancel();

        assert!(token.is_cancelled());
        assert!(token2.is_cancelled());
    }

    #[tokio::test]
    async fn test_cancellation_token_immediate_cancellation() {
        let token = CancellationToken::new();
        token.cancel();

        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), token.cancelled()).await;

        assert!(result.is_ok(), "cancelled() should complete immediately");
    }

    #[tokio::test]
    async fn test_cancellation_token_async_cancellation() {
        let token = CancellationToken::new();
        let token_clone = token.clone();

        let handle = tokio::spawn(async move {
            token_clone.cancelled().await;
            "cancelled"
        });

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        token.cancel();

        let result = tokio::time::timeout(std::time::Duration::from_millis(100), handle).await;

        assert!(result.is_ok(), "Task should complete after cancellation");
        assert_eq!(result.unwrap().unwrap(), "cancelled");
    }

    // =========================================================================
    // Retry with Exponential Backoff Tests
    // =========================================================================

    #[test]
    fn test_retry_constants() {
        assert_eq!(MAX_RETRY_ATTEMPTS, 2);
        assert_eq!(INITIAL_RETRY_DELAY_MS, 500);
        assert_eq!(SubAgentExecutor::total_retry_delay_ms(), 1500);
    }

    #[test]
    fn test_is_retryable_error_timeout_patterns() {
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
        assert!(SubAgentExecutor::is_retryable_error("Rate limit exceeded"));
        assert!(SubAgentExecutor::is_retryable_error("rate_limit_error"));
        assert!(SubAgentExecutor::is_retryable_error(
            "Too many requests, try again"
        ));
    }

    #[test]
    fn test_is_retryable_error_service_patterns() {
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
        assert!(!SubAgentExecutor::is_retryable_error(
            "Operation cancelled due to timeout validation failed"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Invalid request, do not retry"
        ));
    }

    #[test]
    fn test_is_retryable_error_case_insensitive() {
        assert!(SubAgentExecutor::is_retryable_error("TIMEOUT"));
        assert!(SubAgentExecutor::is_retryable_error("TimeOut"));
        assert!(SubAgentExecutor::is_retryable_error("CONNECTION REFUSED"));
        assert!(!SubAgentExecutor::is_retryable_error("CANCELLED"));
        assert!(!SubAgentExecutor::is_retryable_error("Invalid"));
    }

    #[test]
    fn test_is_retryable_error_unknown_errors() {
        assert!(!SubAgentExecutor::is_retryable_error(
            "Something went wrong"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(
            "Unknown error occurred"
        ));
        assert!(!SubAgentExecutor::is_retryable_error(""));
    }

    // =========================================================================
    // Correlation ID (parent_execution_id) Tests
    // =========================================================================

    #[test]
    fn test_create_execution_record_with_parent_default_none() {
        use crate::models::sub_agent::SubAgentExecutionCreate;

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
        assert!(!json.contains("parent_execution_id"));
    }
}
