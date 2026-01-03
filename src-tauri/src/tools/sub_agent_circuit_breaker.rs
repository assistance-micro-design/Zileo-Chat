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

//! Circuit Breaker for Sub-Agent Execution Resilience (OPT-SA-8)
//!
//! Implements the circuit breaker pattern to prevent cascade failures when
//! sub-agent executions repeatedly fail. This protects against scenarios where
//! the LLM provider or orchestration system is experiencing issues.
//!
//! ## States
//!
//! - **Closed**: Normal operation, sub-agent executions proceed
//! - **Open**: System unhealthy (3+ consecutive failures), executions rejected immediately
//! - **HalfOpen**: Testing recovery, allows one execution to test system health
//!
//! ## Configuration
//!
//! - `failure_threshold`: Number of consecutive failures before opening (default: 3)
//! - `cooldown`: Time to wait before attempting recovery (default: 60s)
//!
//! ## Usage
//!
//! The circuit breaker is shared across sub-agent tools via `AgentToolContext`.
//! Each execution checks the circuit before proceeding and records success/failure.
//!
//! ```rust,ignore
//! // Check if execution is allowed
//! let allowed = circuit_breaker.lock().await.allow_request();
//! if !allowed {
//!     return Err(ToolError::ExecutionFailed("Circuit breaker open"));
//! }
//!
//! // Execute and record result
//! match execute_sub_agent().await {
//!     Ok(result) => {
//!         circuit_breaker.lock().await.record_success();
//!         Ok(result)
//!     }
//!     Err(e) => {
//!         circuit_breaker.lock().await.record_failure();
//!         Err(e)
//!     }
//! }
//! ```
//!
//! ## Difference from MCP Circuit Breaker
//!
//! While similar in structure to the MCP circuit breaker, this version:
//! - Protects the sub-agent execution system specifically
//! - Is shared across SpawnAgentTool, DelegateTaskTool, ParallelTasksTool
//! - Uses the same default thresholds but can be configured independently

use std::time::{Duration, Instant};
use tracing::debug;

use super::constants::sub_agent::{CIRCUIT_COOLDOWN_SECS, CIRCUIT_FAILURE_THRESHOLD};

/// Circuit breaker state for sub-agent executions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitState {
    /// Normal operation, executions proceed
    #[default]
    Closed,
    /// System unhealthy, executions rejected immediately
    Open,
    /// Testing recovery, allows one execution through
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "Closed"),
            Self::Open => write!(f, "Open"),
            Self::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// Circuit breaker for sub-agent execution resilience.
///
/// Tracks consecutive failures and implements state transitions to prevent
/// cascade failures when the sub-agent execution system is unhealthy.
///
/// # Thread Safety
///
/// This struct uses interior mutability for thread-safe state management.
/// Wrap in `Arc<Mutex<>>` or `Arc<RwLock<>>` for concurrent access.
#[derive(Debug)]
pub struct SubAgentCircuitBreaker {
    /// Current state of the circuit
    state: CircuitState,
    /// Number of consecutive failures
    failure_count: u32,
    /// Threshold for opening the circuit
    failure_threshold: u32,
    /// Cooldown duration before attempting recovery
    cooldown: Duration,
    /// Timestamp of last failure
    last_failure: Option<Instant>,
}

impl SubAgentCircuitBreaker {
    /// Creates a new circuit breaker with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `failure_threshold` - Number of failures before opening circuit
    /// * `cooldown` - Duration to wait before attempting recovery
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            cooldown,
            last_failure: None,
        }
    }

    /// Creates a circuit breaker with default settings.
    ///
    /// - Failure threshold: 3 (from constants::sub_agent::CIRCUIT_FAILURE_THRESHOLD)
    /// - Cooldown: 60 seconds (from constants::sub_agent::CIRCUIT_COOLDOWN_SECS)
    pub fn with_defaults() -> Self {
        Self::new(
            CIRCUIT_FAILURE_THRESHOLD,
            Duration::from_secs(CIRCUIT_COOLDOWN_SECS),
        )
    }

    /// Checks if a sub-agent execution is allowed to proceed.
    ///
    /// Returns `true` if the circuit is closed or transitioning to half-open.
    /// Returns `false` if the circuit is open (system is unhealthy).
    ///
    /// When returning false, the caller should return an error immediately
    /// to fail fast and avoid overwhelming an already troubled system.
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if cooldown has elapsed
                if self
                    .last_failure
                    .map(|t| t.elapsed() > self.cooldown)
                    .unwrap_or(true)
                {
                    debug!("Sub-agent circuit breaker transitioning to half-open");
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true, // Allow one request to test recovery
        }
    }

    /// Records a successful sub-agent execution.
    ///
    /// Resets failure count and closes the circuit.
    /// Call this after a sub-agent execution completes successfully.
    pub fn record_success(&mut self) {
        if self.state == CircuitState::HalfOpen {
            debug!("Sub-agent circuit breaker closing after successful recovery");
        }
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.last_failure = None;
    }

    /// Records a failed sub-agent execution.
    ///
    /// Increments failure count and opens circuit if threshold is reached.
    /// Call this after a sub-agent execution fails.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());

        if self.failure_count >= self.failure_threshold {
            if self.state != CircuitState::Open {
                debug!(
                    failure_count = self.failure_count,
                    threshold = self.failure_threshold,
                    "Sub-agent circuit breaker opening after consecutive failures"
                );
            }
            self.state = CircuitState::Open;
        } else if self.state == CircuitState::HalfOpen {
            // Recovery failed, go back to open
            debug!("Sub-agent circuit breaker reopening after failed recovery attempt");
            self.state = CircuitState::Open;
        }
    }

    /// Returns the current state of the circuit.
    #[allow(dead_code)] // Used in tests and by external code
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Returns the current failure count.
    #[allow(dead_code)] // Used in tests and by external code
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Returns the configured failure threshold.
    #[allow(dead_code)] // Used in tests and by external code
    pub fn failure_threshold(&self) -> u32 {
        self.failure_threshold
    }

    /// Returns the configured cooldown duration.
    #[allow(dead_code)] // Used in tests and by external code
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    /// Returns the time since last failure, if any.
    #[allow(dead_code)] // Used in tests and by external code
    pub fn time_since_last_failure(&self) -> Option<Duration> {
        self.last_failure.map(|t| t.elapsed())
    }

    /// Returns remaining cooldown time before circuit can transition to half-open.
    ///
    /// Returns `None` if circuit is not open or cooldown has elapsed.
    pub fn remaining_cooldown(&self) -> Option<Duration> {
        if self.state != CircuitState::Open {
            return None;
        }

        self.last_failure.and_then(|t| {
            let elapsed = t.elapsed();
            if elapsed < self.cooldown {
                Some(self.cooldown - elapsed)
            } else {
                None
            }
        })
    }

    /// Returns remaining cooldown in seconds, or 0 if not applicable.
    ///
    /// Convenience method for error messages and logging.
    pub fn remaining_cooldown_secs(&self) -> u64 {
        self.remaining_cooldown().map(|d| d.as_secs()).unwrap_or(0)
    }

    /// Resets the circuit breaker to closed state.
    ///
    /// Use with caution - typically only for testing or manual intervention.
    #[allow(dead_code)] // Used in tests and for manual intervention
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.last_failure = None;
        debug!("Sub-agent circuit breaker manually reset");
    }
}

impl Default for SubAgentCircuitBreaker {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = SubAgentCircuitBreaker::with_defaults();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_default_impl() {
        let cb = SubAgentCircuitBreaker::default();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_threshold(), CIRCUIT_FAILURE_THRESHOLD);
        assert_eq!(cb.cooldown(), Duration::from_secs(CIRCUIT_COOLDOWN_SECS));
    }

    #[test]
    fn test_allow_request_when_closed() {
        let mut cb = SubAgentCircuitBreaker::with_defaults();
        assert!(cb.allow_request());
    }

    #[test]
    fn test_opens_after_threshold_failures() {
        let mut cb = SubAgentCircuitBreaker::new(3, Duration::from_secs(60));

        // First two failures - still closed
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 1);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 2);

        // Third failure - opens
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.failure_count(), 3);
    }

    #[test]
    fn test_rejects_when_open() {
        let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_secs(60));

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Should reject requests
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_transitions_to_half_open_after_cooldown() {
        let mut cb = SubAgentCircuitBreaker::new(
            1,
            Duration::from_millis(10), // Very short cooldown for testing
        );

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(20));

        // Should transition to half-open
        assert!(cb.allow_request());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_closes_on_success_after_half_open() {
        let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_millis(10));

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        cb.allow_request(); // Transitions to half-open

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_reopens_on_failure_in_half_open() {
        let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_millis(10));

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        cb.allow_request(); // Transitions to half-open

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut cb = SubAgentCircuitBreaker::with_defaults();

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_remaining_cooldown() {
        let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_secs(60));

        // No cooldown when closed
        assert!(cb.remaining_cooldown().is_none());
        assert_eq!(cb.remaining_cooldown_secs(), 0);

        cb.record_failure();

        // Should have remaining cooldown when open
        let remaining = cb.remaining_cooldown();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() > Duration::from_secs(50));
        assert!(cb.remaining_cooldown_secs() > 50);
    }

    #[test]
    fn test_reset() {
        let mut cb = SubAgentCircuitBreaker::new(1, Duration::from_secs(60));

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
        assert!(cb.remaining_cooldown().is_none());
    }

    #[test]
    fn test_time_since_last_failure() {
        let mut cb = SubAgentCircuitBreaker::with_defaults();

        // No failure yet
        assert!(cb.time_since_last_failure().is_none());

        cb.record_failure();

        // Should have a recent failure
        let elapsed = cb.time_since_last_failure();
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap() < Duration::from_secs(1));
    }

    #[test]
    fn test_circuit_state_display() {
        assert_eq!(format!("{}", CircuitState::Closed), "Closed");
        assert_eq!(format!("{}", CircuitState::Open), "Open");
        assert_eq!(format!("{}", CircuitState::HalfOpen), "HalfOpen");
    }

    #[test]
    fn test_circuit_state_default() {
        let state = CircuitState::default();
        assert_eq!(state, CircuitState::Closed);
    }

    #[test]
    fn test_multiple_success_after_failures() {
        let mut cb = SubAgentCircuitBreaker::with_defaults();

        // Two failures
        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);
        assert_eq!(cb.state(), CircuitState::Closed);

        // Success resets everything
        cb.record_success();
        assert_eq!(cb.failure_count(), 0);

        // Can fail again
        cb.record_failure();
        assert_eq!(cb.failure_count(), 1);
    }

    #[test]
    fn test_custom_threshold() {
        let mut cb = SubAgentCircuitBreaker::new(5, Duration::from_secs(30));

        for _ in 0..4 {
            cb.record_failure();
            assert_eq!(cb.state(), CircuitState::Closed);
        }

        // Fifth failure opens
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.failure_count(), 5);
    }
}
