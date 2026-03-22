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
//! use zileo_chat::llm::pricing::{calculate_cost_with_cache, CostParams};
//!
//! let cost = calculate_cost_with_cache(&CostParams {
//!     tokens_input: 10000, tokens_output: 2000,
//!     cached_tokens: None, cache_write_tokens: None,
//!     input_price_per_mtok: 2.0, output_price_per_mtok: 6.0,
//!     cache_read_price_per_mtok: 0.0, cache_write_price_per_mtok: 0.0,
//! });
//! // Input: 10000 tokens * $2/MTok = $0.02
//! // Output: 2000 tokens * $6/MTok = $0.012
//! // Total: $0.032
//! ```

/// Parameters for cost calculation with cache-aware pricing.
pub struct CostParams {
    /// Total input tokens (includes cached read + write)
    pub tokens_input: usize,
    /// Output tokens
    pub tokens_output: usize,
    /// Number of input tokens served from cache (cache reads)
    pub cached_tokens: Option<usize>,
    /// Number of input tokens written to cache (cache writes)
    pub cache_write_tokens: Option<usize>,
    /// Price per million regular (non-cached) input tokens
    pub input_price_per_mtok: f64,
    /// Price per million output tokens
    pub output_price_per_mtok: f64,
    /// Price per million cache-read tokens
    pub cache_read_price_per_mtok: f64,
    /// Price per million cache-write tokens
    pub cache_write_price_per_mtok: f64,
}

/// Calculates cost with separate pricing for cache-read and cache-write tokens.
///
/// Returns total cost in USD, rounded to 6 decimal places.
pub fn calculate_cost_with_cache(p: &CostParams) -> f64 {
    let cache_read = p.cached_tokens.unwrap_or(0).min(p.tokens_input);
    let cache_write = p
        .cache_write_tokens
        .unwrap_or(0)
        .min(p.tokens_input.saturating_sub(cache_read));
    let regular_input = p.tokens_input.saturating_sub(cache_read + cache_write);

    let regular_cost = (regular_input as f64 / 1_000_000.0) * p.input_price_per_mtok;
    let read_cost = (cache_read as f64 / 1_000_000.0) * p.cache_read_price_per_mtok;
    let write_cost = (cache_write as f64 / 1_000_000.0) * p.cache_write_price_per_mtok;
    let output_cost = (p.tokens_output as f64 / 1_000_000.0) * p.output_price_per_mtok;

    let total = regular_cost + read_cost + write_cost + output_cost;
    (total * 1_000_000.0).round() / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn cost(
        ti: usize,
        to: usize,
        cr: Option<usize>,
        cw: Option<usize>,
        ip: f64,
        op: f64,
        crp: f64,
        cwp: f64,
    ) -> f64 {
        calculate_cost_with_cache(&CostParams {
            tokens_input: ti,
            tokens_output: to,
            cached_tokens: cr,
            cache_write_tokens: cw,
            input_price_per_mtok: ip,
            output_price_per_mtok: op,
            cache_read_price_per_mtok: crp,
            cache_write_price_per_mtok: cwp,
        })
    }

    #[test]
    fn test_calculate_cost_no_cache() {
        assert!((cost(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0) - 0.032).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_small_model() {
        assert!((cost(50000, 10000, None, None, 0.2, 0.6, 0.0, 0.0) - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_zero_pricing() {
        assert_eq!(cost(100000, 50000, None, None, 0.0, 0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        assert_eq!(cost(0, 0, None, None, 2.0, 6.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn test_calculate_cost_large_token_count() {
        assert!(
            (cost(1_000_000, 1_000_000, None, None, 2.0, 6.0, 0.0, 0.0) - 8.0).abs() < 0.000001
        );
    }

    #[test]
    fn test_calculate_cost_precision() {
        assert!((cost(100, 50, None, None, 2.0, 6.0, 0.0, 0.0) - 0.0005).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_50_percent_price() {
        assert!((cost(10000, 2000, Some(8000), None, 2.0, 6.0, 1.0, 0.0) - 0.024).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_free() {
        assert!((cost(10000, 2000, Some(8000), None, 2.0, 6.0, 0.0, 0.0) - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_cache_write_anthropic_125x() {
        assert!((cost(10000, 2000, None, Some(8000), 2.0, 6.0, 0.0, 2.5) - 0.036).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_and_write_combined() {
        assert!(
            (cost(10000, 1000, Some(5000), Some(3000), 2.0, 6.0, 0.5, 2.5) - 0.02).abs() < 0.000001
        );
    }

    #[test]
    fn test_cache_exceeding_input_clamped() {
        assert!((cost(1000, 500, Some(2000), None, 2.0, 6.0, 1.0, 0.0) - 0.004).abs() < 0.000001);
    }

    #[test]
    fn test_no_cache_info_same_as_regular() {
        let c1 = cost(10000, 2000, None, None, 2.0, 6.0, 1.0, 2.5);
        let c2 = cost(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0);
        assert!((c1 - c2).abs() < 0.000001);
    }
}
