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
//! rates for input (prompt) and output (completion) tokens.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::llm::pricing::calculate_cost;
//!
//! let cost = calculate_cost(10000, 2000, 2.0, 6.0);
//! // Input: 10000 tokens * $2/MTok = $0.02
//! // Output: 2000 tokens * $6/MTok = $0.012
//! // Total: $0.032
//! ```

/// Calculates the cost based on token counts and pricing per million tokens.
///
/// # Arguments
///
/// * `tokens_input` - Number of input (prompt) tokens
/// * `tokens_output` - Number of output (completion) tokens
/// * `input_price_per_mtok` - Price in USD per million input tokens
/// * `output_price_per_mtok` - Price in USD per million output tokens
///
/// # Returns
///
/// Total cost in USD, rounded to 6 decimal places for precision
///
/// # Examples
///
/// ```
/// use zileo_chat::llm::pricing::calculate_cost;
///
/// // Mistral Large pricing: $2/MTok input, $6/MTok output
/// let cost = calculate_cost(10000, 2000, 2.0, 6.0);
/// assert!((cost - 0.032).abs() < 0.000001);
///
/// // Free model (Ollama local)
/// let cost_free = calculate_cost(100000, 50000, 0.0, 0.0);
/// assert_eq!(cost_free, 0.0);
/// ```
pub fn calculate_cost(
    tokens_input: usize,
    tokens_output: usize,
    input_price_per_mtok: f64,
    output_price_per_mtok: f64,
) -> f64 {
    let input_cost = (tokens_input as f64 / 1_000_000.0) * input_price_per_mtok;
    let output_cost = (tokens_output as f64 / 1_000_000.0) * output_price_per_mtok;

    // Round to 6 decimal places to avoid floating point precision issues
    let total = input_cost + output_cost;
    (total * 1_000_000.0).round() / 1_000_000.0
}

/// Reference pricing for Mistral models (November 2025)
///
/// Note: Prices are subject to change. Users should verify current pricing
/// at https://mistral.ai/technology/#pricing
#[allow(dead_code)] // Reference constants for cost calculations
pub mod mistral_pricing {
    /// Mistral Large pricing
    pub const LARGE_INPUT_PER_MTOK: f64 = 2.0;
    pub const LARGE_OUTPUT_PER_MTOK: f64 = 6.0;

    /// Mistral Small pricing
    pub const SMALL_INPUT_PER_MTOK: f64 = 0.2;
    pub const SMALL_OUTPUT_PER_MTOK: f64 = 0.6;

    /// Codestral pricing
    pub const CODESTRAL_INPUT_PER_MTOK: f64 = 0.2;
    pub const CODESTRAL_OUTPUT_PER_MTOK: f64 = 0.6;

    /// Mistral Embed pricing (input only)
    pub const EMBED_INPUT_PER_MTOK: f64 = 0.1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_cost_mistral_large() {
        // Mistral Large: $2/MTok input, $6/MTok output
        let cost = calculate_cost(10000, 2000, 2.0, 6.0);
        // Input: (10000/1M)*2 = 0.02
        // Output: (2000/1M)*6 = 0.012
        // Total: 0.032
        assert!((cost - 0.032).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_mistral_small() {
        // Mistral Small: $0.2/MTok input, $0.6/MTok output
        let cost = calculate_cost(50000, 10000, 0.2, 0.6);
        // Input: (50000/1M)*0.2 = 0.01
        // Output: (10000/1M)*0.6 = 0.006
        // Total: 0.016
        assert!((cost - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_zero_pricing() {
        // Ollama local models have zero pricing
        let cost = calculate_cost(100000, 50000, 0.0, 0.0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        let cost = calculate_cost(0, 0, 2.0, 6.0);
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_cost_large_token_count() {
        // 1 million tokens each
        let cost = calculate_cost(1_000_000, 1_000_000, 2.0, 6.0);
        // Input: 1*2 = 2.0
        // Output: 1*6 = 6.0
        // Total: 8.0
        assert!((cost - 8.0).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_precision() {
        // Very small token count
        let cost = calculate_cost(100, 50, 2.0, 6.0);
        // Input: (100/1M)*2 = 0.0002
        // Output: (50/1M)*6 = 0.0003
        // Total: 0.0005
        assert!((cost - 0.0005).abs() < 0.000001);
    }

    #[test]
    fn test_reference_pricing_values() {
        // Verify reference pricing constants are reasonable
        let large_input = mistral_pricing::LARGE_INPUT_PER_MTOK;
        let large_output = mistral_pricing::LARGE_OUTPUT_PER_MTOK;
        let small_input = mistral_pricing::SMALL_INPUT_PER_MTOK;

        assert!(large_input > 0.0, "Large input pricing should be positive");
        assert!(
            large_output > large_input,
            "Output should cost more than input"
        );
        assert!(
            small_input < large_input,
            "Small model should be cheaper than large"
        );
    }
}
