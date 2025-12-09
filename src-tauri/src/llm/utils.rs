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

//! # LLM Utility Functions
//!
//! Shared utilities for LLM operations across providers.
//!
//! ## Functions
//!
//! - [`estimate_tokens`] - Estimates token count using word-based approximation
//! - [`simulate_streaming`] - Simulates streaming by chunking a complete response

use super::LLMError;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::warn;

// ============================================================================
// Token Estimation
// ============================================================================

/// Estimates token count using word-based approximation.
///
/// French/English text averages ~1.3-1.5 tokens per word.
/// Using word count * 1.5 gives better accuracy than char/4.
///
/// # Arguments
/// * `text` - The text to estimate tokens for
///
/// # Returns
/// Estimated token count (minimum 1)
///
/// # Example
/// ```
/// use zileo_chat::llm::utils::estimate_tokens;
///
/// assert_eq!(estimate_tokens("hello"), 2);  // 1 word * 1.5 = 1.5 -> 2
/// assert_eq!(estimate_tokens("This is a test"), 6);  // 4 words * 1.5 = 6
/// ```
pub fn estimate_tokens(text: &str) -> usize {
    let word_count = text.split_whitespace().count();
    let estimate = ((word_count as f64) * 1.5).ceil() as usize;
    estimate.max(1)
}

// ============================================================================
// Streaming Simulation
// ============================================================================

/// Default chunk size for simulated streaming (characters per chunk)
pub const DEFAULT_CHUNK_SIZE: usize = 20;

/// Default delay between chunks (milliseconds)
pub const DEFAULT_CHUNK_DELAY_MS: u64 = 10;

/// Simulates streaming by chunking a complete response.
///
/// Used when a provider doesn't support native streaming or as a fallback.
/// The response is split into chunks and sent through a channel with a small delay
/// between each chunk to simulate the streaming experience.
///
/// # Arguments
///
/// * `content` - The complete response content to simulate streaming for
/// * `chunk_size` - Number of characters per chunk (default: 20)
/// * `delay_ms` - Delay between chunks in milliseconds (default: 10)
///
/// # Returns
///
/// A channel receiver that yields chunks of the response as `Result<String, LLMError>`
///
/// # Example
///
/// ```rust,ignore
/// let rx = simulate_streaming("Hello, world!".to_string(), None, None);
/// while let Some(result) = rx.recv().await {
///     match result {
///         Ok(chunk) => print!("{}", chunk),
///         Err(e) => eprintln!("Error: {}", e),
///     }
/// }
/// ```
pub fn simulate_streaming(
    content: String,
    chunk_size: Option<usize>,
    delay_ms: Option<u64>,
) -> mpsc::Receiver<Result<String, LLMError>> {
    let (tx, rx) = mpsc::channel(100);
    let chunk_size = chunk_size.unwrap_or(DEFAULT_CHUNK_SIZE);
    let delay = Duration::from_millis(delay_ms.unwrap_or(DEFAULT_CHUNK_DELAY_MS));

    tokio::spawn(async move {
        for chunk in content.as_bytes().chunks(chunk_size) {
            let chunk_str = String::from_utf8_lossy(chunk).to_string();
            if tx.send(Ok(chunk_str)).await.is_err() {
                warn!("Streaming receiver dropped");
                break;
            }
            tokio::time::sleep(delay).await;
        }
    });

    rx
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Token estimation tests
    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 1);
    }

    #[test]
    fn test_estimate_tokens_single_word() {
        assert_eq!(estimate_tokens("hello"), 2); // 1 * 1.5 = 1.5 -> 2
    }

    #[test]
    fn test_estimate_tokens_sentence() {
        // "This is a test" = 4 words * 1.5 = 6
        assert_eq!(estimate_tokens("This is a test"), 6);
    }

    #[test]
    fn test_estimate_tokens_french_sentence() {
        // "Bonjour comment allez-vous" = 3 words * 1.5 = 4.5 -> 5
        assert_eq!(estimate_tokens("Bonjour comment allez-vous"), 5);
    }

    #[test]
    fn test_estimate_tokens_whitespace_only() {
        assert_eq!(estimate_tokens("   "), 1);
    }

    #[test]
    fn test_estimate_tokens_long_text() {
        let text = "The quick brown fox jumps over the lazy dog";
        // 9 words * 1.5 = 13.5 -> 14
        assert_eq!(estimate_tokens(text), 14);
    }

    // Streaming simulation tests
    #[tokio::test]
    async fn test_simulate_streaming_empty() {
        let mut rx = simulate_streaming(String::new(), None, None);
        // Empty string should result in no chunks
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_simulate_streaming_single_chunk() {
        let content = "Hello".to_string();
        let mut rx = simulate_streaming(content.clone(), Some(100), Some(1));

        let chunk = rx.recv().await.expect("Should receive chunk");
        assert_eq!(chunk.unwrap(), "Hello");

        // No more chunks
        assert!(rx.recv().await.is_none());
    }

    #[tokio::test]
    async fn test_simulate_streaming_multiple_chunks() {
        let content = "Hello, world!".to_string(); // 13 chars
        let mut rx = simulate_streaming(content, Some(5), Some(1));

        let mut received = String::new();
        while let Some(result) = rx.recv().await {
            received.push_str(&result.unwrap());
        }

        assert_eq!(received, "Hello, world!");
    }

    #[tokio::test]
    async fn test_simulate_streaming_default_params() {
        let content = "Test content for streaming simulation".to_string();
        let mut rx = simulate_streaming(content.clone(), None, None);

        let mut received = String::new();
        while let Some(result) = rx.recv().await {
            received.push_str(&result.unwrap());
        }

        assert_eq!(received, content);
    }

    #[tokio::test]
    async fn test_simulate_streaming_chunk_count() {
        let content = "12345678901234567890".to_string(); // 20 chars
        let chunk_size = 5;
        let mut rx = simulate_streaming(content, Some(chunk_size), Some(1));

        let mut chunk_count = 0;
        while let Some(result) = rx.recv().await {
            assert!(result.is_ok());
            chunk_count += 1;
        }

        assert_eq!(chunk_count, 4); // 20 / 5 = 4 chunks
    }
}
