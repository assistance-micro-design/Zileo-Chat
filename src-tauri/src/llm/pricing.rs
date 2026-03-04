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

//! # Token Pricing Module
//!
//! This module provides cost calculation functionality based on token counts
//! and model pricing configuration.
//!
//! ## Pricing Model
//!
//! LLM providers typically charge per million tokens (MTok) with different
//! rates for input (prompt), output (completion), cache-read, and cache-write tokens.
//!
//! ## Cache Pricing (OpenRouter)
//!
//! - **Cache read**: tokens served from cache, typically 0.25x-0.50x of input price
//! - **Cache write**: tokens written to cache on first request, typically 1.0x-1.25x of input price
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::llm::pricing::calculate_cost_with_cache;
//!
//! let cost = calculate_cost_with_cache(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0);
//! // Input: 10000 tokens * $2/MTok = $0.02
//! // Output: 2000 tokens * $6/MTok = $0.012
//! // Total: $0.032
//! ```

/// Calculates cost with separate pricing for cache-read and cache-write tokens.
///
/// When cached tokens are present, the input cost is split into up to 3 tiers:
/// - Regular input tokens use `input_price_per_mtok`
/// - Cache-read tokens use `cache_read_price_per_mtok`
/// - Cache-write tokens use `cache_write_price_per_mtok`
/// - Output tokens use `output_price_per_mtok`
///
/// # Arguments
///
/// * `tokens_input` - Total input tokens (includes cached read + write)
/// * `tokens_output` - Output tokens
/// * `cached_tokens` - Number of input tokens served from cache (cache reads)
/// * `cache_write_tokens` - Number of input tokens written to cache (cache writes)
/// * `input_price_per_mtok` - Price for regular (non-cached) input tokens
/// * `output_price_per_mtok` - Price for output tokens
/// * `cache_read_price_per_mtok` - Price for cache-read tokens
/// * `cache_write_price_per_mtok` - Price for cache-write tokens
///
/// # Returns
///
/// Total cost in USD, rounded to 6 decimal places
#[allow(clippy::too_many_arguments)]
pub fn calculate_cost_with_cache(
    tokens_input: usize,
    tokens_output: usize,
    cached_tokens: Option<usize>,
    cache_write_tokens: Option<usize>,
    input_price_per_mtok: f64,
    output_price_per_mtok: f64,
    cache_read_price_per_mtok: f64,
    cache_write_price_per_mtok: f64,
) -> f64 {
    let cache_read = cached_tokens.unwrap_or(0).min(tokens_input);
    let cache_write = cache_write_tokens
        .unwrap_or(0)
        .min(tokens_input.saturating_sub(cache_read));
    let regular_input = tokens_input.saturating_sub(cache_read + cache_write);

    let regular_cost = (regular_input as f64 / 1_000_000.0) * input_price_per_mtok;
    let read_cost = (cache_read as f64 / 1_000_000.0) * cache_read_price_per_mtok;
    let write_cost = (cache_write as f64 / 1_000_000.0) * cache_write_price_per_mtok;
    let output_cost = (tokens_output as f64 / 1_000_000.0) * output_price_per_mtok;

    let total = regular_cost + read_cost + write_cost + output_cost;
    (total * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost_no_cache() {
        // Mistral Large: $2/MTok input, $6/MTok output, no caching
        let cost = calculate_cost_with_cache(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0);
        // Input: (10000/1M)*2 = 0.02
        // Output: (2000/1M)*6 = 0.012
        // Total: 0.032
        assert!((cost - 0.032).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_small_model() {
        // Mistral Small: $0.2/MTok input, $0.6/MTok output
        let cost = calculate_cost_with_cache(50000, 10000, None, None, 0.2, 0.6, 0.0, 0.0);
        // Input: (50000/1M)*0.2 = 0.01
        // Output: (10000/1M)*0.6 = 0.006
        // Total: 0.016
        assert!((cost - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_zero_pricing() {
        // Ollama local models have zero pricing
        let cost = calculate_cost_with_cache(100000, 50000, None, None, 0.0, 0.0, 0.0, 0.0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        let cost = calculate_cost_with_cache(0, 0, None, None, 2.0, 6.0, 0.0, 0.0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_large_token_count() {
        // 1 million tokens each
        let cost = calculate_cost_with_cache(1_000_000, 1_000_000, None, None, 2.0, 6.0, 0.0, 0.0);
        // Input: 1*2 = 2.0, Output: 1*6 = 6.0, Total: 8.0
        assert!((cost - 8.0).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_precision() {
        // Very small token count
        let cost = calculate_cost_with_cache(100, 50, None, None, 2.0, 6.0, 0.0, 0.0);
        // Input: (100/1M)*2 = 0.0002
        // Output: (50/1M)*6 = 0.0003
        // Total: 0.0005
        assert!((cost - 0.0005).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_50_percent_price() {
        // OpenRouter: cache read at 50% of input price
        // 10000 total input, 8000 cache read
        let cost = calculate_cost_with_cache(10000, 2000, Some(8000), None, 2.0, 6.0, 1.0, 0.0);
        // Regular: (2000/1M)*2 = 0.004
        // Cache read: (8000/1M)*1 = 0.008
        // Output: (2000/1M)*6 = 0.012
        // Total: 0.024
        assert!((cost - 0.024).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_free() {
        // Cache read tokens are free (price = 0)
        let cost = calculate_cost_with_cache(10000, 2000, Some(8000), None, 2.0, 6.0, 0.0, 0.0);
        // Regular: (2000/1M)*2 = 0.004
        // Cache read: free
        // Output: (2000/1M)*6 = 0.012
        // Total: 0.016
        assert!((cost - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_cache_write_anthropic_125x() {
        // Anthropic: cache write at 1.25x of input price
        // 10000 total input, 0 cache read, 8000 cache write (first request)
        let cost = calculate_cost_with_cache(10000, 2000, None, Some(8000), 2.0, 6.0, 0.0, 2.5);
        // Regular: (2000/1M)*2 = 0.004
        // Cache write: (8000/1M)*2.5 = 0.02
        // Output: (2000/1M)*6 = 0.012
        // Total: 0.036
        assert!((cost - 0.036).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_and_write_combined() {
        // Mixed: some cache reads + some cache writes
        // 10000 total, 5000 cache read, 3000 cache write, 2000 regular
        let cost =
            calculate_cost_with_cache(10000, 1000, Some(5000), Some(3000), 2.0, 6.0, 0.5, 2.5);
        // Regular: (2000/1M)*2 = 0.004
        // Cache read: (5000/1M)*0.5 = 0.0025
        // Cache write: (3000/1M)*2.5 = 0.0075
        // Output: (1000/1M)*6 = 0.006
        // Total: 0.02
        assert!((cost - 0.02).abs() < 0.000001);
    }

    #[test]
    fn test_cache_exceeding_input_clamped() {
        // Safety: cached_tokens > tokens_input should be clamped
        let cost = calculate_cost_with_cache(1000, 500, Some(2000), None, 2.0, 6.0, 1.0, 0.0);
        // Clamped: cache_read = 1000, regular = 0
        // Cache read: (1000/1M)*1 = 0.001
        // Output: (500/1M)*6 = 0.003
        // Total: 0.004
        assert!((cost - 0.004).abs() < 0.000001);
    }

    #[test]
    fn test_no_cache_info_same_as_regular() {
        // No caching info -> same result regardless of cache prices
        let cost = calculate_cost_with_cache(10000, 2000, None, None, 2.0, 6.0, 1.0, 2.5);
        let cost_zero = calculate_cost_with_cache(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0);
        assert!((cost - cost_zero).abs() < 0.000001);
    }
}
