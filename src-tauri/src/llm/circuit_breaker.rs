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

//! # Circuit Breaker for LLM Providers
//!
//! This module implements the circuit breaker pattern to protect against cascading failures
//! when LLM providers are unavailable or experiencing issues.
//!
//! ## States
//!
//! - **Closed**: Normal operation, requests pass through
//! - **Open**: Circuit is tripped, requests fail immediately
//! - **HalfOpen**: Testing if provider has recovered
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::llm::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
//!
//! let config = CircuitBreakerConfig::default();
//! let breaker = CircuitBreaker::new(config);
//!
//! // Check before making request
//! if breaker.is_available() {
//!     match provider.complete(prompt).await {
//!         Ok(response) => breaker.record_success(),
//!         Err(e) => breaker.record_failure(),
//!     }
//! }
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Configuration for circuit breaker behavior
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit
    pub failure_threshold: u32,
    /// Duration to wait before attempting recovery (half-open state)
    pub cooldown_duration: Duration,
    /// Number of consecutive successes in half-open state to close the circuit
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            cooldown_duration: Duration::from_secs(60),
            success_threshold: 2,
        }
    }
}

impl CircuitBreakerConfig {
    /// Creates a new configuration with custom values
    #[allow(dead_code)] // API completeness - custom configuration builder
    pub fn new(failure_threshold: u32, cooldown_secs: u64, success_threshold: u32) -> Self {
        Self {
            failure_threshold,
            cooldown_duration: Duration::from_secs(cooldown_secs),
            success_threshold,
        }
    }

    /// Creates a configuration optimized for LLM providers with longer cooldowns
    pub fn for_llm_provider() -> Self {
        Self {
            failure_threshold: 3,
            cooldown_duration: Duration::from_secs(30),
            success_threshold: 1,
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation, requests pass through
    Closed,
    /// Circuit is tripped, requests fail immediately
    Open,
    /// Testing if provider has recovered
    HalfOpen,
}

impl std::fmt::Display for CircuitState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CircuitState::Closed => write!(f, "Closed"),
            CircuitState::Open => write!(f, "Open"),
            CircuitState::HalfOpen => write!(f, "HalfOpen"),
        }
    }
}

/// Internal state of the circuit breaker
#[derive(Debug)]
struct CircuitBreakerState {
    /// Current state of the circuit
    state: CircuitState,
    /// Number of consecutive failures
    consecutive_failures: u32,
    /// Number of consecutive successes in half-open state
    consecutive_successes: u32,
    /// Time when the circuit was opened
    opened_at: Option<Instant>,
}

impl Default for CircuitBreakerState {
    fn default() -> Self {
        Self {
            state: CircuitState::Closed,
            consecutive_failures: 0,
            consecutive_successes: 0,
            opened_at: None,
        }
    }
}

/// Circuit breaker for protecting against LLM provider failures
///
/// Thread-safe implementation using Arc<RwLock>.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Configuration for the circuit breaker
    config: CircuitBreakerConfig,
    /// Internal state protected by RwLock
    state: Arc<RwLock<CircuitBreakerState>>,
    /// Provider name for logging
    provider_name: String,
}

impl CircuitBreaker {
    /// Creates a new circuit breaker with the given configuration
    pub fn new(config: CircuitBreakerConfig, provider_name: String) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(CircuitBreakerState::default())),
            provider_name,
        }
    }

    /// Creates a new circuit breaker with default configuration
    #[allow(dead_code)] // API completeness - alternative constructor
    pub fn with_defaults(provider_name: String) -> Self {
        Self::new(CircuitBreakerConfig::for_llm_provider(), provider_name)
    }

    /// Gets the current state of the circuit breaker
    #[allow(dead_code)] // API completeness - state inspection
    pub async fn state(&self) -> CircuitState {
        let state = self.state.read().await;
        state.state
    }

    /// Checks if the circuit breaker allows requests to pass through
    ///
    /// This method also handles the transition from Open to HalfOpen
    /// when the cooldown period has elapsed.
    pub async fn is_available(&self) -> bool {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if cooldown has elapsed
                if let Some(opened_at) = state.opened_at {
                    if opened_at.elapsed() >= self.config.cooldown_duration {
                        info!(
                            provider = %self.provider_name,
                            "Circuit breaker transitioning to half-open after cooldown"
                        );
                        state.state = CircuitState::HalfOpen;
                        state.consecutive_successes = 0;
                        true
                    } else {
                        debug!(
                            provider = %self.provider_name,
                            remaining_secs = (self.config.cooldown_duration - opened_at.elapsed()).as_secs(),
                            "Circuit breaker still open, waiting for cooldown"
                        );
                        false
                    }
                } else {
                    // Shouldn't happen, but handle gracefully
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Records a successful request
    ///
    /// In HalfOpen state, consecutive successes may close the circuit.
    /// In Closed state, resets the failure counter.
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                // Reset failure counter on success
                if state.consecutive_failures > 0 {
                    debug!(
                        provider = %self.provider_name,
                        "Reset failure counter after success"
                    );
                    state.consecutive_failures = 0;
                }
            }
            CircuitState::HalfOpen => {
                state.consecutive_successes += 1;
                debug!(
                    provider = %self.provider_name,
                    successes = state.consecutive_successes,
                    threshold = self.config.success_threshold,
                    "Recording success in half-open state"
                );

                if state.consecutive_successes >= self.config.success_threshold {
                    info!(
                        provider = %self.provider_name,
                        "Circuit breaker closing after recovery"
                    );
                    state.state = CircuitState::Closed;
                    state.consecutive_failures = 0;
                    state.consecutive_successes = 0;
                    state.opened_at = None;
                }
            }
            CircuitState::Open => {
                // Shouldn't happen (is_available would have transitioned to HalfOpen)
                // but handle gracefully
                warn!(
                    provider = %self.provider_name,
                    "Unexpected success recorded while circuit is open"
                );
            }
        }
    }

    /// Records a failed request
    ///
    /// In Closed state, consecutive failures may open the circuit.
    /// In HalfOpen state, any failure reopens the circuit.
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;

        match state.state {
            CircuitState::Closed => {
                state.consecutive_failures += 1;
                debug!(
                    provider = %self.provider_name,
                    failures = state.consecutive_failures,
                    threshold = self.config.failure_threshold,
                    "Recording failure in closed state"
                );

                if state.consecutive_failures >= self.config.failure_threshold {
                    warn!(
                        provider = %self.provider_name,
                        failures = state.consecutive_failures,
                        "Circuit breaker opening after threshold exceeded"
                    );
                    state.state = CircuitState::Open;
                    state.opened_at = Some(Instant::now());
                }
            }
            CircuitState::HalfOpen => {
                // Any failure in half-open state reopens the circuit
                warn!(
                    provider = %self.provider_name,
                    "Circuit breaker reopening after failure in half-open state"
                );
                state.state = CircuitState::Open;
                state.opened_at = Some(Instant::now());
                state.consecutive_successes = 0;
            }
            CircuitState::Open => {
                // Already open, just update the timestamp to extend cooldown
                state.opened_at = Some(Instant::now());
            }
        }
    }

    /// Resets the circuit breaker to closed state
    ///
    /// This should only be used for manual intervention or testing.
    #[allow(dead_code)] // Manual intervention API
    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        info!(
            provider = %self.provider_name,
            previous_state = %state.state,
            "Circuit breaker manually reset"
        );
        *state = CircuitBreakerState::default();
    }

    /// Gets statistics about the circuit breaker
    #[allow(dead_code)] // API completeness - status monitoring
    pub async fn stats(&self) -> CircuitBreakerStats {
        let state = self.state.read().await;
        CircuitBreakerStats {
            state: state.state,
            consecutive_failures: state.consecutive_failures,
            consecutive_successes: state.consecutive_successes,
            opened_at: state.opened_at,
            config: self.config.clone(),
        }
    }
}

/// Statistics about the circuit breaker state
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CircuitBreakerStats {
    /// Current state
    pub state: CircuitState,
    /// Number of consecutive failures
    pub consecutive_failures: u32,
    /// Number of consecutive successes (in half-open state)
    pub consecutive_successes: u32,
    /// When the circuit was opened (if applicable)
    pub opened_at: Option<Instant>,
    /// Configuration
    pub config: CircuitBreakerConfig,
}

#[allow(dead_code)]
impl CircuitBreakerStats {
    /// Returns the time remaining until the circuit can transition to half-open
    pub fn cooldown_remaining(&self) -> Option<Duration> {
        match (self.state, self.opened_at) {
            (CircuitState::Open, Some(opened_at)) => {
                let elapsed = opened_at.elapsed();
                if elapsed < self.config.cooldown_duration {
                    Some(self.config.cooldown_duration - elapsed)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CircuitBreakerConfig {
        CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_millis(100), // Short for tests
            success_threshold: 2,
        }
    }

    #[tokio::test]
    async fn test_circuit_breaker_starts_closed() {
        let breaker = CircuitBreaker::new(test_config(), "test".to_string());
        assert_eq!(breaker.state().await, CircuitState::Closed);
        assert!(breaker.is_available().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let breaker = CircuitBreaker::new(test_config(), "test".to_string());

        // Record failures up to threshold
        for i in 0..3 {
            assert!(
                breaker.is_available().await,
                "Should be available before threshold (attempt {})",
                i
            );
            breaker.record_failure().await;
        }

        // Circuit should now be open
        assert_eq!(breaker.state().await, CircuitState::Open);
        assert!(!breaker.is_available().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_success_resets_failure_count() {
        let breaker = CircuitBreaker::new(test_config(), "test".to_string());

        // Record some failures (but not enough to open)
        breaker.record_failure().await;
        breaker.record_failure().await;

        // Success should reset the counter
        breaker.record_success().await;

        // Now we should need 3 more failures to open
        breaker.record_failure().await;
        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitState::Closed);

        breaker.record_failure().await;
        assert_eq!(breaker.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_after_cooldown() {
        let breaker = CircuitBreaker::new(test_config(), "test".to_string());

        // Open the circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }
        assert_eq!(breaker.state().await, CircuitState::Open);

        // Wait for cooldown
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open on is_available check
        assert!(breaker.is_available().await);
        assert_eq!(breaker.state().await, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn test_circuit_breaker_closes_after_success_in_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_millis(50),
            success_threshold: 2,
        };
        let breaker = CircuitBreaker::new(config, "test".to_string());

        // Open the circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }

        // Wait for cooldown and transition to half-open
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(breaker.is_available().await);
        assert_eq!(breaker.state().await, CircuitState::HalfOpen);

        // Record successes
        breaker.record_success().await;
        assert_eq!(breaker.state().await, CircuitState::HalfOpen);

        breaker.record_success().await;
        assert_eq!(breaker.state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reopens_on_failure_in_half_open() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_millis(50),
            success_threshold: 2,
        };
        let breaker = CircuitBreaker::new(config, "test".to_string());

        // Open the circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }

        // Wait for cooldown and transition to half-open
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(breaker.is_available().await);
        assert_eq!(breaker.state().await, CircuitState::HalfOpen);

        // One success, then failure
        breaker.record_success().await;
        breaker.record_failure().await;

        // Should reopen
        assert_eq!(breaker.state().await, CircuitState::Open);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        let breaker = CircuitBreaker::new(test_config(), "test".to_string());

        // Open the circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }
        assert_eq!(breaker.state().await, CircuitState::Open);

        // Reset
        breaker.reset().await;
        assert_eq!(breaker.state().await, CircuitState::Closed);
        assert!(breaker.is_available().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_stats() {
        let breaker = CircuitBreaker::new(test_config(), "test".to_string());

        // Record some failures
        breaker.record_failure().await;
        breaker.record_failure().await;

        let stats = breaker.stats().await;
        assert_eq!(stats.state, CircuitState::Closed);
        assert_eq!(stats.consecutive_failures, 2);
        assert_eq!(stats.consecutive_successes, 0);
    }

    #[tokio::test]
    async fn test_cooldown_remaining() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_millis(200),
            success_threshold: 2,
        };
        let breaker = CircuitBreaker::new(config, "test".to_string());

        // Open the circuit
        for _ in 0..3 {
            breaker.record_failure().await;
        }

        let stats = breaker.stats().await;
        let remaining = stats.cooldown_remaining();
        assert!(remaining.is_some());
        assert!(remaining.unwrap() <= Duration::from_millis(200));
    }

    #[test]
    fn test_default_config() {
        let config = CircuitBreakerConfig::default();
        assert_eq!(config.failure_threshold, 5);
        assert_eq!(config.cooldown_duration, Duration::from_secs(60));
        assert_eq!(config.success_threshold, 2);
    }

    #[test]
    fn test_llm_provider_config() {
        let config = CircuitBreakerConfig::for_llm_provider();
        assert_eq!(config.failure_threshold, 3);
        assert_eq!(config.cooldown_duration, Duration::from_secs(30));
        assert_eq!(config.success_threshold, 1);
    }
}
