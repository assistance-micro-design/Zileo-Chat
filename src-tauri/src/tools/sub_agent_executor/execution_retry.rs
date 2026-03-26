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

//! Retry logic with exponential backoff for sub-agent execution.
//!
//! Wraps `execute_with_heartbeat_timeout` with automatic retry on transient errors.

use std::time::Duration;

use tracing::{debug, info, warn};

use crate::agents::core::agent::Task;
use crate::tools::constants::sub_agent::{INITIAL_RETRY_DELAY_MS, MAX_RETRY_ATTEMPTS};

use super::activity_monitor::ActivityCallback;
use super::{ExecutionResult, SubAgentExecutor};

impl SubAgentExecutor {
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
    pub(crate) fn total_retry_delay_ms() -> u64 {
        let mut total = 0;
        for i in 0..MAX_RETRY_ATTEMPTS {
            total += INITIAL_RETRY_DELAY_MS * 2_u64.pow(i);
        }
        total
    }
}
