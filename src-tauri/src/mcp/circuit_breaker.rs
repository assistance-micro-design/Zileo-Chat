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

//! Circuit Breaker for MCP Server Resilience
//!
//! Implements the circuit breaker pattern to prevent cascade failures when
//! MCP servers become unhealthy. Three states:
//!
//! - **Closed**: Normal operation, requests pass through
//! - **Open**: Server unhealthy, requests are rejected immediately
//! - **HalfOpen**: Testing recovery, allows one request through
//!
//! ## Configuration
//!
//! - `failure_threshold`: Number of consecutive failures before opening (default: 3)
//! - `cooldown`: Time to wait before attempting recovery (default: 60s)
//!
//! ## Example
//!
//! ```rust,ignore
//! let mut cb = CircuitBreaker::default();
//!
//! // Check if request is allowed
//! if cb.allow_request() {
//!     match call_server().await {
//!         Ok(_) => cb.record_success(),
//!         Err(_) => cb.record_failure(),
//!     }
//! } else {
//!     // Circuit is open, fail fast
//!     return Err("Server unavailable");
//! }
//! ```

use std::time::{Duration, Instant};
use tracing::debug;

/// Default failure threshold before opening circuit
pub const DEFAULT_FAILURE_THRESHOLD: u32 = 3;

/// Default cooldown period before half-open state (60 seconds)
pub const DEFAULT_COOLDOWN_SECS: u64 = 60;

/// Circuit breaker state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CircuitState {
    /// Normal operation, requests pass through
    #[default]
    Closed,
    /// Server unhealthy, requests rejected immediately
    Open,
    /// Testing recovery, allows one request through
    HalfOpen,
}

/// Circuit breaker for MCP server resilience
///
/// Tracks failures and implements state transitions to prevent
/// cascade failures when servers become unhealthy.
#[derive(Debug)]
pub struct CircuitBreaker {
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
    /// Server name for logging
    server_name: String,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with custom configuration
    ///
    /// # Arguments
    ///
    /// * `server_name` - Name of the server (for logging)
    /// * `failure_threshold` - Number of failures before opening circuit
    /// * `cooldown` - Duration to wait before attempting recovery
    pub fn new(server_name: String, failure_threshold: u32, cooldown: Duration) -> Self {
        Self {
            state: CircuitState::Closed,
            failure_count: 0,
            failure_threshold,
            cooldown,
            last_failure: None,
            server_name,
        }
    }

    /// Creates a circuit breaker with default settings
    ///
    /// - Failure threshold: 3
    /// - Cooldown: 60 seconds
    pub fn with_defaults(server_name: String) -> Self {
        Self::new(
            server_name,
            DEFAULT_FAILURE_THRESHOLD,
            Duration::from_secs(DEFAULT_COOLDOWN_SECS),
        )
    }

    /// Checks if a request is allowed to proceed
    ///
    /// Returns `true` if the circuit is closed or transitioning to half-open.
    /// Returns `false` if the circuit is open (server is unhealthy).
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
                    debug!(
                        server = %self.server_name,
                        "Circuit breaker transitioning to half-open"
                    );
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true, // Allow one request to test recovery
        }
    }

    /// Records a successful request
    ///
    /// Resets failure count and closes the circuit.
    pub fn record_success(&mut self) {
        if self.state == CircuitState::HalfOpen {
            debug!(
                server = %self.server_name,
                "Circuit breaker closing after successful recovery"
            );
        }
        self.failure_count = 0;
        self.state = CircuitState::Closed;
        self.last_failure = None;
    }

    /// Records a failed request
    ///
    /// Increments failure count and opens circuit if threshold is reached.
    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());

        if self.failure_count >= self.failure_threshold {
            if self.state != CircuitState::Open {
                debug!(
                    server = %self.server_name,
                    failure_count = self.failure_count,
                    threshold = self.failure_threshold,
                    "Circuit breaker opening after failures"
                );
            }
            self.state = CircuitState::Open;
        } else if self.state == CircuitState::HalfOpen {
            // Recovery failed, go back to open
            debug!(
                server = %self.server_name,
                "Circuit breaker reopening after failed recovery attempt"
            );
            self.state = CircuitState::Open;
        }
    }

    /// Returns the current state of the circuit
    pub fn state(&self) -> CircuitState {
        self.state
    }

    /// Returns the current failure count
    pub fn failure_count(&self) -> u32 {
        self.failure_count
    }

    /// Returns the configured failure threshold
    pub fn failure_threshold(&self) -> u32 {
        self.failure_threshold
    }

    /// Returns the configured cooldown duration
    pub fn cooldown(&self) -> Duration {
        self.cooldown
    }

    /// Returns the time since last failure, if any
    pub fn time_since_last_failure(&self) -> Option<Duration> {
        self.last_failure.map(|t| t.elapsed())
    }

    /// Returns remaining cooldown time before circuit can transition to half-open
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

    /// Resets the circuit breaker to closed state
    ///
    /// Use with caution - typically only for testing or manual intervention.
    pub fn reset(&mut self) {
        self.state = CircuitState::Closed;
        self.failure_count = 0;
        self.last_failure = None;
        debug!(
            server = %self.server_name,
            "Circuit breaker manually reset"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::with_defaults("test".to_string());
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_allow_request_when_closed() {
        let mut cb = CircuitBreaker::with_defaults("test".to_string());
        assert!(cb.allow_request());
    }

    #[test]
    fn test_opens_after_threshold_failures() {
        let mut cb = CircuitBreaker::new("test".to_string(), 3, Duration::from_secs(60));

        // First two failures - still closed
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Third failure - opens
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
        assert_eq!(cb.failure_count(), 3);
    }

    #[test]
    fn test_rejects_when_open() {
        let mut cb = CircuitBreaker::new("test".to_string(), 1, Duration::from_secs(60));

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        // Should reject requests
        assert!(!cb.allow_request());
    }

    #[test]
    fn test_transitions_to_half_open_after_cooldown() {
        let mut cb = CircuitBreaker::new(
            "test".to_string(),
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
        let mut cb = CircuitBreaker::new("test".to_string(), 1, Duration::from_millis(10));

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        cb.allow_request(); // Transitions to half-open

        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
    }

    #[test]
    fn test_reopens_on_failure_in_half_open() {
        let mut cb = CircuitBreaker::new("test".to_string(), 1, Duration::from_millis(10));

        cb.record_failure();
        std::thread::sleep(Duration::from_millis(20));
        cb.allow_request(); // Transitions to half-open

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }

    #[test]
    fn test_success_resets_failure_count() {
        let mut cb = CircuitBreaker::with_defaults("test".to_string());

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.failure_count(), 2);

        cb.record_success();
        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_remaining_cooldown() {
        let mut cb = CircuitBreaker::new("test".to_string(), 1, Duration::from_secs(60));

        // No cooldown when closed
        assert!(cb.remaining_cooldown().is_none());

        cb.record_failure();

        // Should have remaining cooldown when open
        let remaining = cb.remaining_cooldown();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() > Duration::from_secs(50));
    }

    #[test]
    fn test_reset() {
        let mut cb = CircuitBreaker::new("test".to_string(), 1, Duration::from_secs(60));

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);

        cb.reset();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert_eq!(cb.failure_count(), 0);
        assert!(cb.remaining_cooldown().is_none());
    }
}
