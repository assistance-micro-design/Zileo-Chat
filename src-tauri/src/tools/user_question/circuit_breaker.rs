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

//! Circuit Breaker for UserQuestionTool Timeout Resilience (OPT-UQ-12)
//!
//! Implements the circuit breaker pattern to prevent repeated questions when
//! users are unresponsive. Opens after 3 consecutive timeouts, preventing
//! new questions for a cooldown period.
//!
//! ## States
//!
//! - **Closed**: Normal operation, questions can be asked
//! - **Open**: Too many timeouts, questions are rejected immediately
//! - **HalfOpen**: Testing recovery, allows one question through
//!
//! ## Configuration
//!
//! Uses constants from `tools::constants::user_question`:
//! - `CIRCUIT_FAILURE_THRESHOLD`: 3 consecutive timeouts to open (default)
//! - `CIRCUIT_COOLDOWN_SECS`: 60 seconds before recovery attempt (default)
//!
//! ## Example
//!
//! ```rust,ignore
//! use crate::tools::user_question::circuit_breaker::UserQuestionCircuitBreaker;
//!
//! let mut cb = UserQuestionCircuitBreaker::new("workflow_123".to_string());
//!
//! if cb.allow_question() {
//!     match ask_user_question().await {
//!         Ok(response) => cb.record_success(),
//!         Err(e) if e.contains("Timeout") => cb.record_timeout(),
//!         Err(_) => {} // Other errors don't affect circuit
//!     }
//! } else {
//!     // Circuit is open, fail fast
//!     return Err("User appears unresponsive, skipping question");
//! }
//! ```

use std::time::{Duration, Instant};
use tracing::debug;

/// Default failure threshold before opening circuit (3 consecutive timeouts)
#[allow(dead_code)]
pub const DEFAULT_TIMEOUT_THRESHOLD: u32 = 3;

/// Default cooldown period before half-open state (60 seconds)
#[allow(dead_code)]
pub const DEFAULT_COOLDOWN_SECS: u64 = 60;

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CircuitState {
    /// Normal operation, questions can be asked
    Closed,
    /// Too many timeouts, questions rejected immediately
    Open,
    /// Testing recovery, allows one question through
    HalfOpen,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self::Closed
    }
}

/// Circuit breaker for UserQuestionTool timeout resilience.
///
/// Tracks consecutive timeouts and implements state transitions to prevent
/// spamming questions when users are unresponsive.
#[derive(Debug)]
#[allow(dead_code)]
pub struct UserQuestionCircuitBreaker {
    /// Current state of the circuit
    state: CircuitState,
    /// Number of consecutive timeouts
    timeout_count: u32,
    /// Threshold for opening the circuit
    timeout_threshold: u32,
    /// Cooldown duration before attempting recovery
    cooldown: Duration,
    /// Timestamp of last timeout
    last_timeout: Option<Instant>,
    /// Workflow ID for logging
    workflow_id: String,
}

#[allow(dead_code)]
impl UserQuestionCircuitBreaker {
    /// Creates a new circuit breaker with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `workflow_id` - Workflow ID (for logging)
    /// * `timeout_threshold` - Number of timeouts before opening circuit
    /// * `cooldown` - Duration to wait before attempting recovery
    pub fn new(workflow_id: String, timeout_threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            timeout_count: 0,
            timeout_threshold,
            cooldown,
            last_timeout: None,
            workflow_id,
        }
    }

    /// Creates a circuit breaker with default settings.
    ///
    /// - Timeout threshold: 3
    /// - Cooldown: 60 seconds
    pub fn with_defaults(workflow_id: String) -> Self {
        Self::new(
            workflow_id,
            DEFAULT_TIMEOUT_THRESHOLD,
            Duration::from_secs(DEFAULT_COOLDOWN_SECS),
        )
    }

    /// Checks if a question is allowed to be asked.
    ///
    /// Returns `true` if the circuit is closed or transitioning to half-open.
    /// Returns `false` if the circuit is open (user is unresponsive).
    pub fn allow_question(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if cooldown has elapsed
                if self
                    .last_timeout
                    .map(|t| t.elapsed() > self.cooldown)
                    .unwrap_or(true)
                {
                    debug!(
                        workflow_id = %self.workflow_id,
                        "UserQuestion circuit breaker transitioning to half-open"
                    );
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true, // Allow one question to test recovery
        }
    }

    /// Records a successful question response.
    ///
    /// Resets timeout count and closes the circuit.
    pub fn record_success(&mut self) {
        if self.state == CircuitState::HalfOpen {
            debug!(
                workflow_id = %self.workflow_id,
                "UserQuestion circuit breaker closing after successful response"
            );
        }
        self.timeout_count = 0;
        self.state = CircuitState::Closed;
        self.last_timeout = None;
    }

    /// Records a question timeout.
    ///
    /// Increments timeout count and opens circuit if threshold is reached.
    pub fn record_timeout(&mut self) {
        self.timeout_count += 1;
        self.last_timeout = Some(Instant::now());

        if self.timeout_count >= self.timeout_threshold {
            if self.state != CircuitState::Open {
                debug!(
                    workflow_id = %self.workflow_id,
                    timeout_count = self.timeout_count,
                    threshold = self.timeout_threshold,
                    "UserQuestion circuit breaker opening after consecutive timeouts"
                );
            }
            self.state = CircuitState::Open;
        } else if self.state == CircuitState::HalfOpen {
            // Recovery failed, go back to open
            debug!(
                workflow_id = %self.workflow_id,
                "UserQuestion circuit breaker reopening after failed recovery attempt"
            );
            self.state = CircuitState::Open;
        }
    }

    /// Records a question skip (user chose to skip).
    ///
    /// Skips don't count as timeouts since user actively responded.
    pub fn record_skip(&mut self) {
        // Skips are intentional user actions, not timeouts
        // Reset state like a success
        self.record_success();
    }

    /// Returns the current state of the circuit.
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Returns the current timeout count.
    pub fn timeout_count(&self) -> u32 {
        self.timeout_count
    }

    /// Returns the configured timeout threshold.
    #[allow(dead_code)]
    pub fn timeout_threshold(&self) -> u32 {
        self.timeout_threshold
    }

    /// Returns the configured cooldown duration.
    #[allow(dead_code)]
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    /// Returns remaining cooldown time before circuit can transition to half-open.
    ///
    /// Returns `None` if circuit is not open or cooldown has elapsed.
    pub fn remaining_cooldown(&self) -> Option<Duration> {
        if self.state != CircuitState::Open {
            return None;
        }

        self.last_timeout.and_then(|t| {
            let elapsed = t.elapsed();
            if elapsed < self.cooldown {
                Some(self.cooldown - elapsed)
            } else {
                None
            }
        })
    }

    /// Resets the circuit breaker to closed state.
    ///
    /// Use with caution - typically only for testing or manual intervention.
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.timeout_count = 0;
        self.last_timeout = None;
        debug!(
            workflow_id = %self.workflow_id,
            "UserQuestion circuit breaker manually reset"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = UserQuestionCircuitBreaker::with_defaults("test_wf".to_string());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.timeout_count(), 0);
    }

    #[test]
    fn test_allow_question_when_closed() {
        let mut cb = UserQuestionCircuitBreaker::with_defaults("test_wf".to_string());
        assert!(cb.allow_question());
    }

    #[test]
    fn test_opens_after_threshold_timeouts() {
        let mut cb =
            UserQuestionCircuitBreaker::new("test_wf".to_string(), 3, Duration::from_secs(60));

        // First two timeouts - still closed
        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Third timeout - opens
        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.timeout_count(), 3);
    }

    #[test]
    fn test_rejects_when_open() {
        let mut cb =
            UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_secs(60));

        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Open);

        // Should reject questions
        assert!(!cb.allow_question());
    }

    #[test]
    fn test_transitions_to_half_open_after_cooldown() {
        let mut cb = UserQuestionCircuitBreaker::new(
            "test_wf".to_string(),
            1,
            Duration::from_millis(10), // Very short cooldown for testing
        );

        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Open);

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(20));

        // Should transition to half-open
        assert!(cb.allow_question());
        assert_eq!(cb.state(), CircuitState::HalfOpen);
    }

    #[test]
    fn test_closes_on_success_after_half_open() {
        let mut cb =
            UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_millis(10));

        cb.record_timeout();
        std::thread::sleep(Duration::from_millis(20));
        cb.allow_question(); // Transitions to half-open

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.timeout_count(), 0);
    }

    #[test]
    fn test_reopens_on_timeout_in_half_open() {
        let mut cb =
            UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_millis(10));

        cb.record_timeout();
        std::thread::sleep(Duration::from_millis(20));
        cb.allow_question(); // Transitions to half-open

        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_success_resets_timeout_count() {
        let mut cb = UserQuestionCircuitBreaker::with_defaults("test_wf".to_string());

        cb.record_timeout();
        cb.record_timeout();
        assert_eq!(cb.timeout_count(), 2);

        cb.record_success();
        assert_eq!(cb.timeout_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_skip_resets_like_success() {
        let mut cb = UserQuestionCircuitBreaker::with_defaults("test_wf".to_string());

        cb.record_timeout();
        cb.record_timeout();
        assert_eq!(cb.timeout_count(), 2);

        cb.record_skip();
        assert_eq!(cb.timeout_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_remaining_cooldown() {
        let mut cb =
            UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_secs(60));

        // No cooldown when closed
        assert!(cb.remaining_cooldown().is_none());

        cb.record_timeout();

        // Should have remaining cooldown when open
        let remaining = cb.remaining_cooldown();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() > Duration::from_secs(50));
    }

    #[test]
    fn test_reset() {
        let mut cb =
            UserQuestionCircuitBreaker::new("test_wf".to_string(), 1, Duration::from_secs(60));

        cb.record_timeout();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.timeout_count(), 0);
        assert!(cb.remaining_cooldown().is_none());
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_TIMEOUT_THRESHOLD, 3);
        assert_eq!(DEFAULT_COOLDOWN_SECS, 60);
    }
}
