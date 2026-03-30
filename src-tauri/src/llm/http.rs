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

//! Shared HTTP types and helpers for LLM providers.
//!
//! Contains response parsing types that are common across Mistral,
//! Ollama, and OpenAI-compatible providers:
//! - [`ParsedContent`] - Separated text and thinking content
//! - [`ContentBlock`] - Tagged content block (thinking or text)
//! - [`deserialize_content`] - Polymorphic content deserializer
//! - [`ApiErrorResponse`] / [`ApiErrorDetail`] - Error response types
//! - [`send_and_read_body`] - Common HTTP send + body read pattern
//! - [`parse_api_error`] - Shared error response parsing

use super::provider::LLMError;
use crate::tools::utils::safe_truncate;
use serde::Deserialize;
use tracing::debug;

/// Parsed content from an LLM API response, separating text from thinking blocks.
///
/// Used by all providers that return reasoning model responses.
#[derive(Debug, Clone)]
pub struct ParsedContent {
    /// The text content (final answer)
    pub text: String,
    /// Thinking content from reasoning models (if present)
    pub thinking: Option<String>,
}

/// Content block for reasoning models (thinking or text).
///
/// Supports two thinking formats:
/// - Array of TextBlock: `thinking: [{"type": "text", "text": "..."}]`
/// - Plain string: `thinking: "..."` (e.g. mistral-small with reasoning_effort)
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "thinking")]
    Thinking {
        #[serde(deserialize_with = "deserialize_thinking_field")]
        thinking: String,
    },
    #[serde(rename = "text")]
    Text { text: String },
}

/// Text block within thinking content (array format).
#[derive(Debug, Deserialize)]
struct TextBlock {
    text: String,
}

/// Deserializes the `thinking` field which can be either a plain string
/// or an array of TextBlock objects.
///
/// Handles both formats:
/// - `"thinking": "plain text"` (mistral-small with reasoning_effort)
/// - `"thinking": [{"type": "text", "text": "..."}]` (Magistral, OpenAI-compat)
fn deserialize_thinking_field<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};

    struct ThinkingFieldVisitor;

    impl<'de> Visitor<'de> for ThinkingFieldVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or an array of text blocks")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value)
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut parts = String::new();
            while let Some(block) = seq.next_element::<TextBlock>()? {
                if !parts.is_empty() {
                    parts.push('\n');
                }
                parts.push_str(&block.text);
            }
            Ok(parts)
        }
    }

    deserializer.deserialize_any(ThinkingFieldVisitor)
}

/// Custom deserializer for content field that handles both string and array formats.
///
/// - Standard models: content is a plain string -> `ParsedContent { text, thinking: None }`
/// - Reasoning models: content is an array of `ContentBlock` -> separated text and thinking
pub fn deserialize_content<'de, D>(deserializer: D) -> Result<ParsedContent, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct ContentVisitor;

    impl<'de> Visitor<'de> for ContentVisitor {
        type Value = ParsedContent;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or an array of content blocks")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let (thinking, clean) = crate::llm::utils::parse_think_tags(value);
            Ok(ParsedContent {
                text: clean,
                thinking,
            })
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            let (thinking, clean) = crate::llm::utils::parse_think_tags(&value);
            Ok(ParsedContent {
                text: clean,
                thinking,
            })
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut text_parts = String::new();
            let mut thinking_parts = String::new();

            while let Some(block) = seq.next_element::<ContentBlock>()? {
                match block {
                    ContentBlock::Thinking { thinking } => {
                        if !thinking.is_empty() {
                            if !thinking_parts.is_empty() {
                                thinking_parts.push('\n');
                            }
                            thinking_parts.push_str(&thinking);
                        }
                        debug!("Reasoning model thinking block extracted");
                    }
                    ContentBlock::Text { text } => {
                        if !text_parts.is_empty() {
                            text_parts.push('\n');
                        }
                        text_parts.push_str(&text);
                    }
                }
            }

            Ok(ParsedContent {
                text: text_parts,
                thinking: if thinking_parts.is_empty() {
                    None
                } else {
                    Some(thinking_parts)
                },
            })
        }
    }

    deserializer.deserialize_any(ContentVisitor)
}

/// API error response format (shared across OpenAI-compatible APIs).
///
/// Supports both `{"message": {...}}` and `{"error": {...}}` via serde alias.
#[derive(Debug, Deserialize)]
pub struct ApiErrorResponse {
    #[serde(alias = "error")]
    pub message: Option<ApiErrorDetail>,
}

/// Error detail in API response.
#[derive(Debug, Deserialize)]
pub struct ApiErrorDetail {
    pub message: String,
}

/// Sends an HTTP request and reads the response body as text.
///
/// Returns `(status_code, body_text)`.
///
/// # Errors
/// Returns `LLMError::RequestFailed` if the request fails or the body cannot be read.
pub async fn send_and_read_body(
    response: Result<reqwest::Response, reqwest::Error>,
) -> Result<(reqwest::StatusCode, String), LLMError> {
    let response =
        response.map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| LLMError::RequestFailed(format!("Failed to read response body: {}", e)))?;

    Ok((status, body))
}

/// Parses an error response body using the shared [`ApiErrorResponse`] format.
///
/// Tries to extract a structured error message; falls back to raw body.
pub fn parse_api_error(provider_name: &str, status: reqwest::StatusCode, body: &str) -> LLMError {
    let error_msg = if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(body) {
        error_response
            .message
            .map(|e| e.message)
            .unwrap_or_else(|| body.to_string())
    } else {
        body.to_string()
    };
    LLMError::RequestFailed(format!(
        "{} API error ({}): {}",
        provider_name, status, error_msg
    ))
}

/// Parses a JSON response body, providing a truncated body in the error message.
///
/// # Errors
/// Returns `LLMError::RequestFailed` with a truncated body excerpt on parse failure.
pub fn parse_json_response<T: serde::de::DeserializeOwned>(
    provider_name: &str,
    body: &str,
) -> Result<T, LLMError> {
    serde_json::from_str(body).map_err(|e| {
        LLMError::RequestFailed(format!(
            "Failed to parse {} response: {}. Body: {}",
            provider_name,
            e,
            safe_truncate(body, 500, true)
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper struct to test deserialize_content via serde.
    #[derive(Debug, Deserialize)]
    struct TestMessage {
        #[allow(dead_code)]
        role: String,
        #[serde(deserialize_with = "deserialize_content")]
        content: ParsedContent,
    }

    #[test]
    fn test_deserialize_standard_content() {
        let json = r#"{"role": "assistant", "content": "Hello world"}"#;
        let msg: TestMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "Hello world");
        assert!(msg.content.thinking.is_none());
    }

    #[test]
    fn test_deserialize_reasoning_content_array_thinking() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [{"type": "text", "text": "Let me think..."}]},
                {"type": "text", "text": "The answer is 42"}
            ]
        }"#;
        let msg: TestMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "The answer is 42");
        assert_eq!(msg.content.thinking, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_deserialize_reasoning_content_string_thinking() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": "Je dois compter les r dans strawberry..."},
                {"type": "text", "text": "Il y a 3 lettres r."}
            ]
        }"#;
        let msg: TestMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "Il y a 3 lettres r.");
        assert_eq!(
            msg.content.thinking,
            Some("Je dois compter les r dans strawberry...".to_string())
        );
    }

    #[test]
    fn test_deserialize_multiple_thinking_blocks() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [{"type": "text", "text": "Step 1: analyze"}]},
                {"type": "thinking", "thinking": [{"type": "text", "text": "Step 2: compute"}]},
                {"type": "text", "text": "The result is 7"}
            ]
        }"#;
        let msg: TestMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "The result is 7");
        assert_eq!(
            msg.content.thinking,
            Some("Step 1: analyze\nStep 2: compute".to_string())
        );
    }

    #[test]
    fn test_deserialize_multiple_text_blocks() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "text", "text": "First part"},
                {"type": "text", "text": "Second part"}
            ]
        }"#;
        let msg: TestMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "First part\nSecond part");
        assert!(msg.content.thinking.is_none());
    }

    #[test]
    fn test_deserialize_thinking_with_multiple_sub_blocks() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [
                    {"type": "text", "text": "First thought"},
                    {"type": "text", "text": "Second thought"}
                ]},
                {"type": "text", "text": "Final answer"}
            ]
        }"#;
        let msg: TestMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "Final answer");
        assert_eq!(
            msg.content.thinking,
            Some("First thought\nSecond thought".to_string())
        );
    }

    #[test]
    fn test_parse_api_error_structured() {
        let body = r#"{"error": {"message": "Rate limit exceeded"}}"#;
        let err = parse_api_error("TestProvider", reqwest::StatusCode::TOO_MANY_REQUESTS, body);
        let msg = err.to_string();
        assert!(msg.contains("Rate limit exceeded"));
        assert!(msg.contains("TestProvider"));
    }

    #[test]
    fn test_parse_api_error_raw_body() {
        let body = "Internal Server Error";
        let err = parse_api_error(
            "TestProvider",
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            body,
        );
        let msg = err.to_string();
        assert!(msg.contains("Internal Server Error"));
    }
}
