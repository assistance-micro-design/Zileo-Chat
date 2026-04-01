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
//! - [`parse_think_tags`] - Extracts `<think>` blocks from content strings
//! - [`extract_thinking_from_message`] - Extracts thinking content from provider-specific response formats

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

/// Parses `<think>...</think>` tags from content strings.
///
/// Many reasoning models (Kimi, DeepSeek, QwQ) served via OpenAI-compatible APIs
/// embed thinking content in the response string using `<think>` tags rather than
/// structured fields.
///
/// # Returns
/// A tuple of (thinking content or None, cleaned content without tags).
pub fn parse_think_tags(content: &str) -> (Option<String>, String) {
    if let Some(start) = content.find("<think>") {
        if let Some(rel_end) = content[start + 7..].find("</think>") {
            let end = start + 7 + rel_end;
            let thinking_raw = &content[start + 7..end];
            let thinking = thinking_raw.trim().to_string();
            let clean = format!("{}{}", &content[..start], &content[end + 8..])
                .trim()
                .to_string();
            if thinking.is_empty() {
                return (None, clean);
            }
            return (Some(thinking), clean);
        }
    }
    (None, content.to_string())
}

/// Extracts thinking/reasoning content from an OpenAI-format message object.
///
/// Checks multiple formats used by various providers:
/// - `message.reasoning` (OpenRouter format, string)
/// - `message.reasoning_content` (vLLM/LM Studio format, string)
/// - `message.reasoning_details[]` (array of objects with `text` field)
/// - `message.thinking` (Ollama/proxy format, string)
/// - `<think>...</think>` tags in content string (Kimi, DeepSeek, QwQ)
/// - Content blocks array with `{type: "thinking"}` (Mistral-like)
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

    // vLLM/LM Studio format: message.reasoning_content (string)
    if let Some(reasoning) = message.get("reasoning_content").and_then(|v| v.as_str()) {
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

    // Ollama/proxy format: message.thinking (string)
    if let Some(thinking) = message.get("thinking").and_then(|v| v.as_str()) {
        if !thinking.trim().is_empty() {
            return Some(thinking.to_string());
        }
    }

    // Think tags in content string (Kimi, DeepSeek, QwQ via OpenAI-compatible APIs)
    if let Some(content_str) = message.get("content").and_then(|v| v.as_str()) {
        let (thinking, _) = parse_think_tags(content_str);
        if thinking.is_some() {
            return thinking;
        }
    }

    // Content blocks format (Mistral/Kimi): message.content is array with thinking blocks
    if let Some(blocks) = message.get("content").and_then(|v| v.as_array()) {
        let mut thinking_parts = String::new();
        for block in blocks {
            if block.get("type").and_then(|t| t.as_str()) == Some("thinking") {
                // Array format: thinking is [{type: "text", text: "..."}]
                if let Some(items) = block.get("thinking").and_then(|t| t.as_array()) {
                    for item in items {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            if !thinking_parts.is_empty() {
                                thinking_parts.push('\n');
                            }
                            thinking_parts.push_str(text);
                        }
                    }
                }
                // String format: thinking is a plain string
                else if let Some(text) = block.get("thinking").and_then(|t| t.as_str()) {
                    if !thinking_parts.is_empty() {
                        thinking_parts.push('\n');
                    }
                    thinking_parts.push_str(text);
                }
            }
        }
        if !thinking_parts.is_empty() {
            return Some(thinking_parts);
        }
    }

    None
}

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

    #[test]
    fn test_extract_thinking_from_message_content_blocks_with_thinking() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": "Let me analyze this..."},
                {"type": "text", "text": "Here is my answer"}
            ]
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("Let me analyze this...".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_content_blocks_array_thinking() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [{"type": "text", "text": "Step 1"}, {"type": "text", "text": "Step 2"}]},
                {"type": "text", "text": "Answer"}
            ]
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("Step 1\nStep 2".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_thinking_field() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer",
            "thinking": "I need to think about this..."
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("I need to think about this...".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_thinking_field_empty() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "Answer",
            "thinking": "  "
        });
        assert!(extract_thinking_from_message(&message).is_none());
    }

    #[test]
    fn test_extract_thinking_from_message_think_tags_in_content() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "<think>Let me reason about this...</think>Here is my answer"
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("Let me reason about this...".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_think_tags_multiline() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "<think>\nStep 1: Analyze\nStep 2: Plan\n</think>\n\nHere is my answer"
        });
        assert_eq!(
            extract_thinking_from_message(&message),
            Some("Step 1: Analyze\nStep 2: Plan".to_string())
        );
    }

    #[test]
    fn test_extract_thinking_from_message_think_tags_empty() {
        let message = serde_json::json!({
            "role": "assistant",
            "content": "<think>  </think>Answer"
        });
        assert!(extract_thinking_from_message(&message).is_none());
    }

    #[test]
    fn test_parse_think_tags_basic() {
        let (thinking, clean) = parse_think_tags("<think>reasoning</think>answer");
        assert_eq!(thinking, Some("reasoning".to_string()));
        assert_eq!(clean, "answer");
    }

    #[test]
    fn test_parse_think_tags_multiline() {
        let (thinking, clean) = parse_think_tags("<think>\nline1\nline2\n</think>\n\nfinal");
        assert_eq!(thinking, Some("line1\nline2".to_string()));
        assert_eq!(clean, "final");
    }

    #[test]
    fn test_parse_think_tags_no_tags() {
        let (thinking, clean) = parse_think_tags("just normal content");
        assert!(thinking.is_none());
        assert_eq!(clean, "just normal content");
    }

    #[test]
    fn test_parse_think_tags_empty_thinking() {
        let (thinking, clean) = parse_think_tags("<think>  </think>answer");
        assert!(thinking.is_none());
        assert_eq!(clean, "answer");
    }
}
