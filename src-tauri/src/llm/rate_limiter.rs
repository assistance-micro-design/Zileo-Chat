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

//! # Rate Limiter for LLM API Calls (OPT-LLM-8)
//!
//! This module provides rate limiting functionality to respect API rate limits
//! imposed by LLM providers.
//!
//! ## Rate Limits by Provider
//!
//! | Provider | Free Tier | Paid Tier | Default Delay |
//! |----------|-----------|-----------|---------------|
//! | Mistral  | 1 req/s   | 5 req/s   | 1000ms        |
//! | Ollama   | No limit  | -         | 1000ms        |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::llm::rate_limiter::RateLimiter;
//!
//! let limiter = RateLimiter::new();
//!
//! // Before each API call
//! limiter.wait_if_needed().await;
//! // ... make API call ...
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;
use tracing::debug;

/// Minimum delay between API calls in milliseconds.
///
/// This value (1000ms = 1 second) is chosen to be compatible with:
/// - Mistral Free Tier: 1 request per second
/// - Mistral Paid Tier: 5 requests per second (conservative)
/// - Ollama: No limit, but adds safety margin
///
/// Sources:
/// - https://docs.mistral.ai/deployment/ai-studio/tier
/// - https://help.mistral.ai/en/articles/424390-how-do-api-rate-limits-work-and-how-do-i-increase-them
pub const MIN_DELAY_BETWEEN_CALLS_MS: u64 = 1000;

/// Rate limiter for respecting LLM provider API rate limits.
///
/// Ensures a minimum delay between consecutive API calls to avoid
/// hitting rate limits (HTTP 429 errors).
///
/// The rate limiter is thread-safe and can be shared across async tasks.
#[derive(Clone)]
pub struct RateLimiter {
    /// Timestamp of the last API call
    last_call: Arc<Mutex<Option<Instant>>>,
    /// Minimum delay between calls
    min_delay: Duration,
}

#[allow(dead_code)]
impl RateLimiter {
    /// Creates a new rate limiter with the default minimum delay.
    pub fn new() -> Self {
        Self {
            last_call: Arc::new(Mutex::new(None)),
            min_delay: Duration::from_millis(MIN_DELAY_BETWEEN_CALLS_MS),
        }
    }

    /// Creates a new rate limiter with a custom minimum delay.
    ///
    /// # Arguments
    /// * `min_delay_ms` - Minimum delay between calls in milliseconds
    pub fn with_delay(min_delay_ms: u64) -> Self {
        Self {
            last_call: Arc::new(Mutex::new(None)),
            min_delay: Duration::from_millis(min_delay_ms),
        }
    }

    /// Waits if necessary before allowing a new API call.
    ///
    /// This method should be called before each API request. It will:
    /// 1. Check the time since the last call
    /// 2. Sleep if the minimum delay hasn't elapsed
    /// 3. Update the last call timestamp
    ///
    /// The first call never waits.
    pub async fn wait_if_needed(&self) {
        let mut last = self.last_call.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_delay {
                let wait_time = self.min_delay - elapsed;
                debug!(
                    wait_ms = wait_time.as_millis() as u64,
                    elapsed_ms = elapsed.as_millis() as u64,
                    min_delay_ms = self.min_delay.as_millis() as u64,
                    "Rate limiting: waiting before next API call"
                );
                sleep(wait_time).await;
            }
        }

        *last = Some(Instant::now());
    }

    /// Returns the configured minimum delay between calls.
    pub fn min_delay(&self) -> Duration {
        self.min_delay
    }

    /// Returns the time elapsed since the last call, if any.
    pub async fn time_since_last_call(&self) -> Option<Duration> {
        let last = self.last_call.lock().await;
        last.map(|t| t.elapsed())
    }

    /// Resets the rate limiter, clearing the last call timestamp.
    ///
    /// After reset, the next call will not need to wait.
    pub async fn reset(&self) {
        let mut last = self.last_call.lock().await;
        *last = None;
        debug!("Rate limiter reset");
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_first_call_no_wait() {
        let limiter = RateLimiter::new();
        let start = Instant::now();
        limiter.wait_if_needed().await;
        // First call should not wait
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_rate_limiter_second_call_waits() {
        let limiter = RateLimiter::new();

        // First call
        limiter.wait_if_needed().await;

        // Second call should wait approximately min_delay
        let start = Instant::now();
        limiter.wait_if_needed().await;

        // Should have waited at least 900ms (allowing some tolerance)
        assert!(start.elapsed() >= Duration::from_millis(900));
    }

    #[tokio::test]
    async fn test_rate_limiter_no_wait_after_delay() {
        let limiter = RateLimiter::with_delay(100); // Use short delay for test

        // First call
        limiter.wait_if_needed().await;

        // Wait longer than the minimum delay
        sleep(Duration::from_millis(150)).await;

        // Second call should not need to wait (delay already elapsed)
        let start = Instant::now();
        limiter.wait_if_needed().await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_rate_limiter_custom_delay() {
        let custom_delay_ms = 500;
        let limiter = RateLimiter::with_delay(custom_delay_ms);

        assert_eq!(limiter.min_delay(), Duration::from_millis(custom_delay_ms));

        // First call
        limiter.wait_if_needed().await;

        // Second call should wait approximately 500ms
        let start = Instant::now();
        limiter.wait_if_needed().await;

        // Should have waited at least 400ms (allowing some tolerance)
        assert!(start.elapsed() >= Duration::from_millis(400));
        // But not more than 600ms
        assert!(start.elapsed() < Duration::from_millis(600));
    }

    #[tokio::test]
    async fn test_rate_limiter_default() {
        let limiter = RateLimiter::default();
        assert_eq!(
            limiter.min_delay(),
            Duration::from_millis(MIN_DELAY_BETWEEN_CALLS_MS)
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_time_since_last_call() {
        let limiter = RateLimiter::new();

        // Before any call, should be None
        assert!(limiter.time_since_last_call().await.is_none());

        // After a call, should have a value
        limiter.wait_if_needed().await;
        let elapsed = limiter.time_since_last_call().await;
        assert!(elapsed.is_some());
        assert!(elapsed.unwrap() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_rate_limiter_reset() {
        let limiter = RateLimiter::with_delay(100);

        // Make a call
        limiter.wait_if_needed().await;
        assert!(limiter.time_since_last_call().await.is_some());

        // Reset
        limiter.reset().await;
        assert!(limiter.time_since_last_call().await.is_none());

        // Next call should not wait
        let start = Instant::now();
        limiter.wait_if_needed().await;
        assert!(start.elapsed() < Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_rate_limiter_clone() {
        let limiter1 = RateLimiter::with_delay(100);

        // Make a call with limiter1
        limiter1.wait_if_needed().await;

        // Clone and verify they share state
        let limiter2 = limiter1.clone();

        // limiter2 should also see the last call time
        assert!(limiter2.time_since_last_call().await.is_some());

        // Calling wait on limiter2 should respect the shared state
        let start = Instant::now();
        limiter2.wait_if_needed().await;
        // Should wait since the first call was recent
        assert!(start.elapsed() >= Duration::from_millis(50));
    }
}
