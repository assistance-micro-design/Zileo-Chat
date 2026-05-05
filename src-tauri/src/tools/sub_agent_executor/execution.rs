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

//! Execution engine for sub-agents with heartbeat monitoring and circuit breaker.
//!
//! Provides:
//! - Heartbeat-based inactivity timeout detection
//! - Cancellation support via `CancellationToken`
//! - Circuit breaker protection against cascade failures
//!
//! Retry logic is in `execution_retry.rs`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio_util::task::AbortOnDropHandle;
use tracing::{debug, error, info, warn};

use crate::agents::core::agent::{Report, Task};
use crate::models::sub_agent::SubAgentMetrics;
use crate::tools::constants::sub_agent::{ACTIVITY_CHECK_INTERVAL_SECS, INACTIVITY_TIMEOUT_SECS};
use crate::tools::ToolResult;

use super::activity_monitor::ActivityCallback;
use super::activity_monitor::ActivityMonitor;
use super::{ExecutionResult, SubAgentExecutor};

impl SubAgentExecutor {
    /// Checks if the circuit breaker allows execution.
    ///
    /// If a circuit breaker is configured and the circuit is open (system unhealthy),
    /// returns an error with remaining cooldown time. Otherwise returns Ok.
    ///
    /// # Returns
    /// * `Ok(())` - Execution is allowed (circuit closed/half-open or no circuit breaker)
    /// * `Err(ToolError)` - Execution blocked (circuit open)
    pub(crate) async fn check_circuit(&self) -> ToolResult<()> {
        if let Some(ref cb) = self.circuit_breaker {
            let mut guard = cb.lock().await;
            if !guard.allow_request() {
                let remaining = guard.remaining_cooldown_secs();
                return Err(crate::tools::ToolError::ExecutionFailed(format!(
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
    pub(crate) async fn record_success(&self) {
        if let Some(ref cb) = self.circuit_breaker {
            let mut guard = cb.lock().await;
            guard.record_success();
        }
    }

    /// Records failed execution with the circuit breaker.
    ///
    /// Increments failure count and may open circuit if threshold is reached.
    pub(crate) async fn record_failure(&self) {
        if let Some(ref cb) = self.circuit_breaker {
            let mut guard = cb.lock().await;
            guard.record_failure();
        }
    }

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
                    cached_tokens: None,
                    cache_write_tokens: None,
                    thinking_tokens: None,
                    cost_usd: None,
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
        // Forward the workflow's cancellation token into the sub-agent so its
        // own tool loop and LLM retry helpers see it explicitly. The
        // AbortOnDropHandle below is the cooperative-failsafe for the
        // tokio::select! drop case in `orchestrator_bridge`; the explicit
        // token is the fast path that lets reqwest abort mid-flight without
        // waiting for the runtime to tear down the task.
        let inner_cancellation = self.cancellation_token.clone();

        // Spawn the execution in a separate task so select! can poll the
        // heartbeat branch without being starved by a hot LLM loop.
        // `AbortOnDropHandle` ties the spawned task's lifetime to this
        // future: when the outer chain (e.g. the bridge `tokio::select!`
        // racing the workflow CancellationToken) drops us, the inner task
        // is aborted. Without this, `tokio::spawn` would detach the task
        // and the sub-agent would keep running after a user cancel.
        let execution_handle = AbortOnDropHandle::new(tokio::spawn(async move {
            monitor_for_exec.record_activity();
            let result = orchestrator
                .execute_with_mcp(&agent_id_owned, task, mcp_manager, inner_cancellation)
                .await;
            monitor_for_exec.record_activity();
            result
        }));

        // Call the activity callback once to signal start
        activity_callback();

        tokio::pin!(execution_handle);

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
                join_result = &mut execution_handle => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    let result = match join_result {
                        Ok(report_result) => report_result,
                        Err(e) if e.is_cancelled() => {
                            return Self::build_cancelled_result(agent_id, duration_ms);
                        }
                        Err(e) => Err(anyhow::anyhow!("Task join error: {}", e)),
                    };
                    return self.handle_execution_result(agent_id, result, duration_ms).await;
                }

                _ = &mut cancellation_future => {
                    // Drop of `execution_handle` on return aborts the inner
                    // task via `AbortOnDropHandle::drop`; no explicit
                    // `abort_handle.abort()` needed.
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    return Self::build_cancelled_result(agent_id, duration_ms);
                }

                _ = tokio::time::sleep(Duration::from_secs(ACTIVITY_CHECK_INTERVAL_SECS)) => {
                    monitor.record_activity();
                    let inactive_secs = monitor.seconds_since_last_activity();

                    if inactive_secs > INACTIVITY_TIMEOUT_SECS {
                        self.record_failure().await;
                        let duration_ms = start_time.elapsed().as_millis() as u64;
                        // Same drop-on-return semantics as cancellation.
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
                // Compute cost using THIS sub-agent's pricing. Falls back to
                // None when the sub-agent isn't registered or its model has
                // no pricing row.
                let cost = crate::llm::pricing::compute_sub_agent_cost(
                    &self.db,
                    self.orchestrator.registry(),
                    agent_id,
                    crate::llm::pricing::SubAgentCostInput {
                        tokens_input: report.metrics.tokens_input,
                        tokens_output: report.metrics.tokens_output,
                        cached_tokens: report.metrics.cached_tokens,
                        cache_write_tokens: report.metrics.cache_write_tokens,
                        provider_cost_usd: report.metrics.provider_cost_usd,
                    },
                )
                .await;
                ExecutionResult {
                    success: true,
                    report: report.content,
                    metrics: SubAgentMetrics {
                        duration_ms,
                        tokens_input: report.metrics.tokens_input as u64,
                        tokens_output: report.metrics.tokens_output as u64,
                        cached_tokens: report.metrics.cached_tokens.map(|n| n as u64),
                        cache_write_tokens: report.metrics.cache_write_tokens.map(|n| n as u64),
                        thinking_tokens: report.metrics.thinking_tokens.map(|n| n as u64),
                        cost_usd: cost.map(|c| c.cost_usd),
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
                        cached_tokens: None,
                        cache_write_tokens: None,
                        thinking_tokens: None,
                        cost_usd: None,
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
                cached_tokens: None,
                cache_write_tokens: None,
                thinking_tokens: None,
                cost_usd: None,
            },
            error_message: Some("Execution cancelled by user".to_string()),
            tool_executions: Vec::new(),
            reasoning_steps: Vec::new(),
        }
    }

    /// Builds an `ExecutionResult` for an inactivity timeout.
    pub(crate) fn build_timeout_result(
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
                cached_tokens: None,
                cache_write_tokens: None,
                thinking_tokens: None,
                cost_usd: None,
            },
            error_message: Some(format!(
                "Inactivity timeout: no activity for {} seconds (threshold: {}s)",
                inactive_secs, INACTIVITY_TIMEOUT_SECS
            )),
            tool_executions: Vec::new(),
            reasoning_steps: Vec::new(),
        }
    }
}

#[cfg(test)]
#[path = "execution_tests.rs"]
mod tests;
