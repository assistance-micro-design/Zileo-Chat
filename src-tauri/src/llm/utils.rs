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

#[cfg(test)]
mod tests {
    use super::*;

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
}
