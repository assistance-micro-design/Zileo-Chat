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
// Thinking Extraction (OpenAI/OpenRouter format)
// ============================================================================

/// Extracts thinking/reasoning content from an OpenAI-format message object.
///
/// Checks two alternative formats used by various providers:
/// - `message.reasoning` (OpenRouter format, string)
/// - `message.reasoning_details[]` (array of objects with `text` field)
///
/// # Arguments
/// * `message` - A JSON value representing the `choices[0].message` object
///
/// # Returns
/// The extracted thinking text, or `None` if no reasoning content is found.
pub fn extract_thinking_from_message(message: &serde_json::Value) -> Option<String> {
    // OpenRouter format: message.reasoning (string)
    if let Some(reasoning) = message.get("reasoning").and_then(|v| v.as_str()) {
        if !reasoning.trim().is_empty() {
            return Some(reasoning.to_string());
        }
    }

    // Alternative format: message.reasoning_details (array of objects with text)
    if let Some(details) = message.get("reasoning_details").and_then(|v| v.as_array()) {
        let texts: Vec<&str> = details
            .iter()
            .filter_map(|item| {
                item.get("text")
                    .and_then(|t| t.as_str())
                    .filter(|s| !s.trim().is_empty())
            })
            .collect();
        if !texts.is_empty() {
            return Some(texts.join("\n"));
        }
    }

    None
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

    // Thinking extraction tests
    #[test]
    fn test_extract_thinking_from_message_reasoning_field() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer",
            "reasoning": "Step 1..."
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("Step 1...".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_reasoning_details() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer",
            "reasoning_details": [{"text": "Step 1"}, {"text": "Step 2"}]
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("Step 1\nStep 2".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_none() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer"
        });
        assert!(extract_thinking_from_message(&message).is_none());
    }

    #[test]
    fn test_extract_thinking_from_message_empty_reasoning() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer",
            "reasoning": "   "
        });
        assert!(extract_thinking_from_message(&message).is_none());
    }

    #[test]
    fn test_extract_thinking_from_message_empty_details() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer",
            "reasoning_details": [{"text": "  "}]
        });
        assert!(extract_thinking_from_message(&message).is_none());
    }
}
