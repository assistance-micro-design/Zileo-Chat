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

//! # Retry Mechanism for LLM API Calls
//!
//! This module provides retry logic with exponential backoff for LLM API calls.
//! It handles transient failures (network issues, rate limits, server errors)
//! while failing fast on non-recoverable errors (auth failures, bad requests).
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::llm::retry::{RetryConfig, with_retry};
//!
//! let config = RetryConfig::default();
//! let result = with_retry(
//!     || async { provider.complete(prompt).await },
//!     &config,
//!     |err| err.is_retryable(),
//! ).await;
//! ```

use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{debug, warn};

use super::provider::LLMError;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not including the initial attempt)
    pub max_retries: u32,
    /// Initial delay before first retry (milliseconds)
    pub initial_delay_ms: u64,
    /// Maximum delay between retries (milliseconds)
    pub max_delay_ms: u64,
    /// Multiplier for exponential backoff (default: 2.0)
    pub backoff_multiplier: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
            backoff_multiplier: 2.0,
        }
    }
}

#[allow(dead_code)]
impl RetryConfig {
    /// Creates a new RetryConfig with custom values
    pub fn new(max_retries: u32, initial_delay_ms: u64, max_delay_ms: u64) -> Self {
        Self {
            max_retries,
            initial_delay_ms,
            max_delay_ms,
            backoff_multiplier: 2.0,
        }
    }

    /// Calculates the delay for a given attempt number (0-indexed)
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms =
            (self.initial_delay_ms as f64) * self.backoff_multiplier.powi(attempt as i32);
        let clamped_ms = delay_ms.min(self.max_delay_ms as f64) as u64;
        Duration::from_millis(clamped_ms)
    }
}

/// Determines if an LLM error is retryable
///
/// Retryable errors:
/// - ConnectionError: Network issues, transient failures
/// - RequestFailed: May be rate limit (429) or server error (5xx)
/// - StreamingError: May be transient network issue
///
/// Non-retryable errors:
/// - NotConfigured: Configuration issue, won't fix itself
/// - InvalidProvider: Invalid input, won't fix itself
/// - MissingApiKey: Auth issue, won't fix itself
/// - ModelNotFound: Invalid model, won't fix itself
/// - Internal: Programming error, won't fix itself
pub fn is_retryable(error: &LLMError) -> bool {
    matches!(
        error,
        LLMError::ConnectionError(_) | LLMError::RequestFailed(_) | LLMError::StreamingError(_)
    )
}

/// Executes an async operation with retry logic and exponential backoff
///
/// # Arguments
///
/// * `operation` - An async function that returns `Result<T, LLMError>`
/// * `config` - Retry configuration (max retries, delays, etc.)
///
/// # Returns
///
/// The result of the operation, or the last error if all retries failed
///
/// # Example
///
/// ```rust,ignore
/// let result = with_retry(
///     || async { provider.complete(prompt).await },
///     &RetryConfig::default(),
/// ).await;
/// ```
pub async fn with_retry<F, T, Fut>(operation: F, config: &RetryConfig) -> Result<T, LLMError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, LLMError>>,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(result) => {
                if attempt > 0 {
                    debug!(
                        attempt = attempt,
                        "Operation succeeded after {} retries", attempt
                    );
                }
                return Ok(result);
            }
            Err(error) => {
                // Check if we should retry
                if !is_retryable(&error) {
                    debug!(
                        error = %error,
                        "Non-retryable error, failing immediately"
                    );
                    return Err(error);
                }

                // Check if we've exceeded max retries
                if attempt >= config.max_retries {
                    warn!(
                        attempt = attempt,
                        max_retries = config.max_retries,
                        error = %error,
                        "Max retries exceeded"
                    );
                    return Err(error);
                }

                // Calculate delay and wait
                let delay = config.delay_for_attempt(attempt);
                warn!(
                    attempt = attempt + 1,
                    max_retries = config.max_retries,
                    delay_ms = delay.as_millis() as u64,
                    error = %error,
                    "Retrying after transient error"
                );

                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_default_config() {
        let config = RetryConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.initial_delay_ms, 1000);
        assert_eq!(config.max_delay_ms, 30000);
        assert!((config.backoff_multiplier - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_delay_for_attempt() {
        let config = RetryConfig::default();

        // Attempt 0: 1000ms
        assert_eq!(config.delay_for_attempt(0), Duration::from_millis(1000));

        // Attempt 1: 1000 * 2 = 2000ms
        assert_eq!(config.delay_for_attempt(1), Duration::from_millis(2000));

        // Attempt 2: 1000 * 4 = 4000ms
        assert_eq!(config.delay_for_attempt(2), Duration::from_millis(4000));

        // Attempt 3: 1000 * 8 = 8000ms
        assert_eq!(config.delay_for_attempt(3), Duration::from_millis(8000));
    }

    #[test]
    fn test_delay_capped_at_max() {
        let config = RetryConfig::new(10, 1000, 5000);

        // After several attempts, should be capped at 5000ms
        assert_eq!(config.delay_for_attempt(10), Duration::from_millis(5000));
    }

    #[test]
    fn test_is_retryable() {
        // Retryable errors
        assert!(is_retryable(&LLMError::ConnectionError(
            "timeout".to_string()
        )));
        assert!(is_retryable(&LLMError::RequestFailed(
            "rate limit".to_string()
        )));
        assert!(is_retryable(&LLMError::StreamingError(
            "connection reset".to_string()
        )));

        // Non-retryable errors
        assert!(!is_retryable(&LLMError::NotConfigured(
            "mistral".to_string()
        )));
        assert!(!is_retryable(&LLMError::InvalidProvider(
            "unknown".to_string()
        )));
        assert!(!is_retryable(&LLMError::MissingApiKey(
            "mistral".to_string()
        )));
        assert!(!is_retryable(&LLMError::ModelNotFound("gpt-4".to_string())));
        assert!(!is_retryable(&LLMError::Internal("bug".to_string())));
    }

    #[tokio::test]
    async fn test_retry_success_first_attempt() {
        let config = RetryConfig::default();
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = with_retry(
            || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Ok::<_, LLMError>("success".to_string())
                }
            },
            &config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_success_after_failures() {
        let config = RetryConfig::new(3, 10, 100); // Short delays for test
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = with_retry(
            || {
                let count = call_count_clone.clone();
                async move {
                    let n = count.fetch_add(1, Ordering::SeqCst);
                    if n < 2 {
                        // Fail first 2 attempts
                        Err(LLMError::ConnectionError("timeout".to_string()))
                    } else {
                        Ok::<_, LLMError>("success".to_string())
                    }
                }
            },
            &config,
        )
        .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(call_count.load(Ordering::SeqCst), 3); // 2 failures + 1 success
    }

    #[tokio::test]
    async fn test_retry_max_exceeded() {
        let config = RetryConfig::new(2, 10, 100); // Short delays, max 2 retries
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = with_retry(
            || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<String, _>(LLMError::ConnectionError("timeout".to_string()))
                }
            },
            &config,
        )
        .await;

        assert!(result.is_err());
        // 1 initial + 2 retries = 3 total attempts
        assert_eq!(call_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_non_retryable_error_fails_immediately() {
        let config = RetryConfig::new(3, 10, 100);
        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = call_count.clone();

        let result = with_retry(
            || {
                let count = call_count_clone.clone();
                async move {
                    count.fetch_add(1, Ordering::SeqCst);
                    Err::<String, _>(LLMError::MissingApiKey("mistral".to_string()))
                }
            },
            &config,
        )
        .await;

        assert!(result.is_err());
        // Should fail immediately without retrying
        assert_eq!(call_count.load(Ordering::SeqCst), 1);
    }
}
