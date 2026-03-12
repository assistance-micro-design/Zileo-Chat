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

//! Generic OpenAI-compatible provider implementation.
//!
//! Supports any API that follows the OpenAI chat completions format:
//! - POST `{base_url}/chat/completions` for completions
//! - GET `{base_url}/models` for connection testing
//!
//! Handles both standard and reasoning model response formats via
//! a polymorphic content deserializer (string or array of content blocks).

use super::provider::{LLMError, LLMResponse, ProviderType};
use crate::models::agent::ReasoningEffort;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

// ============================================================================
// OpenAI-compatible API Types
// ============================================================================

/// API request body for chat completions
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    /// OpenAI-compatible reasoning effort parameter.
    /// Sent as `"reasoning_effort": "low"/"medium"/"high"` for providers that support it.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

/// Message in OpenAI API format
#[derive(Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

/// API response (handles both standard and reasoning models)
#[derive(Debug, Deserialize)]
struct ChatResponse {
    #[allow(dead_code)]
    id: Option<String>,
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
}

/// Choice in API response
#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
    finish_reason: Option<String>,
}

/// Parsed content separating text from thinking blocks
#[derive(Debug, Clone)]
struct ParsedContent {
    /// The text content (final answer)
    text: String,
    /// Thinking content from reasoning models (if present)
    thinking: Option<String>,
}

/// Response message - content can be string or array of content blocks
#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    #[allow(dead_code)]
    role: String,
    #[serde(deserialize_with = "deserialize_content")]
    content: ParsedContent,
}

/// Content block for reasoning models (thinking or text)
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "thinking")]
    Thinking { thinking: Vec<TextBlock> },
    #[serde(rename = "text")]
    Text { text: String },
}

/// Text block within thinking content
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TextBlock {
    text: String,
}

/// Usage statistics from API response
#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

/// API error response
#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    #[serde(alias = "error")]
    message: Option<ApiErrorDetail>,
}

/// Error detail in API response
#[derive(Debug, Deserialize)]
struct ApiErrorDetail {
    message: String,
}

// ============================================================================
// Function Calling Types
// ============================================================================

/// API request body for chat completions with tools
#[derive(Debug, Serialize)]
struct ToolChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
}

// ============================================================================
// Content Deserializer (supports both string and array formats)
// ============================================================================

/// Custom deserializer for content field that handles both string and array formats.
/// Extracts thinking content from reasoning models into ParsedContent.
fn deserialize_content<'de, D>(deserializer: D) -> Result<ParsedContent, D::Error>
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
            Ok(ParsedContent {
                text: value.to_string(),
                thinking: None,
            })
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(ParsedContent {
                text: value,
                thinking: None,
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
                        for tb in &thinking {
                            if !thinking_parts.is_empty() {
                                thinking_parts.push('\n');
                            }
                            thinking_parts.push_str(&tb.text);
                        }
                        debug!("Reasoning model thinking blocks: {} items", thinking.len());
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

// ============================================================================
// Prompt Cache Control
// ============================================================================

/// Applies prompt cache control markers at strategic positions to maximize cache hits.
///
/// Places up to 3 `cache_control: { "type": "ephemeral" }` breakpoints:
/// - **BP1**: System message (always, stable across iterations)
/// - **BP2**: Last assistant message before current iteration (near-end of stable prefix)
/// - **BP3**: Last tool message (exact boundary of stable prefix)
///
/// Only BP1 is applied for short conversations (< 3 messages).
/// Required for Anthropic Claude models via OpenRouter. Harmlessly ignored
/// by providers that cache automatically (OpenAI, DeepSeek, Gemini).
fn apply_prompt_cache_control(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    // Find indices for BP2 and BP3 within the stable prefix.
    // The last message is always new content (current iteration) and must NOT be marked.
    let mut last_assistant_idx: Option<usize> = None;
    let mut last_tool_idx: Option<usize> = None;

    if messages.len() > 2 {
        let stable_prefix = &messages[..messages.len() - 1];
        for (i, msg) in stable_prefix.iter().enumerate().rev() {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if last_tool_idx.is_none() && role == "tool" {
                last_tool_idx = Some(i);
            }
            if last_assistant_idx.is_none() && role == "assistant" {
                // Mark assistant if it's before the last tool, or if there's no tool
                if last_tool_idx.is_none() || i < last_tool_idx.unwrap_or(usize::MAX) {
                    last_assistant_idx = Some(i);
                    break;
                }
            }
        }
    }

    messages
        .iter()
        .enumerate()
        .map(|(i, msg)| {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

            let should_mark = match role {
                "system" => true,
                "assistant" => Some(i) == last_assistant_idx,
                "tool" => Some(i) == last_tool_idx,
                _ => false,
            };

            if should_mark {
                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    let mut marked = serde_json::json!({
                        "role": role,
                        "content": [{
                            "type": "text",
                            "text": content,
                            "cache_control": { "type": "ephemeral" }
                        }]
                    });
                    // Preserve tool-specific fields
                    if role == "tool" {
                        if let Some(tool_call_id) = msg.get("tool_call_id") {
                            marked["tool_call_id"] = tool_call_id.clone();
                        }
                        if let Some(name) = msg.get("name") {
                            marked["name"] = name.clone();
                        }
                    }
                    marked
                } else {
                    // Already multipart or non-string content: skip
                    msg.clone()
                }
            } else {
                msg.clone()
            }
        })
        .collect()
}

// ============================================================================
// OpenAI-Compatible Provider
// ============================================================================

/// Generic provider for any OpenAI-compatible API.
///
/// Supports configurable base URL and API key. Used for custom providers
/// like RouterLab, OpenRouter, Together AI, etc.
pub struct OpenAiCompatibleProvider {
    /// API key
    api_key: Arc<RwLock<Option<String>>>,
    /// Base URL (e.g., "https://api.routerlab.ch/v1")
    base_url: Arc<RwLock<Option<String>>>,
    /// Provider name for logging and identification
    provider_name: String,
    /// Shared HTTP client (connection pooling)
    http_client: Arc<reqwest::Client>,
}

impl OpenAiCompatibleProvider {
    /// Creates a new unconfigured provider with a shared HTTP client.
    pub fn new(name: &str, http_client: Arc<reqwest::Client>) -> Self {
        Self {
            api_key: Arc::new(RwLock::new(None)),
            base_url: Arc::new(RwLock::new(None)),
            provider_name: name.to_string(),
            http_client,
        }
    }

    /// Configures the provider with API key and base URL.
    pub async fn configure(&self, api_key: &str, base_url: &str) -> Result<(), LLMError> {
        if api_key.is_empty() {
            return Err(LLMError::MissingApiKey(self.provider_name.clone()));
        }
        if base_url.is_empty() {
            return Err(LLMError::NotConfigured(format!(
                "Base URL is required for {}",
                self.provider_name
            )));
        }

        // Normalize: remove trailing slash
        let normalized_url = base_url.trim_end_matches('/').to_string();

        *self.api_key.write().await = Some(api_key.to_string());
        *self.base_url.write().await = Some(normalized_url);

        info!(provider = %self.provider_name, "Custom provider configured");
        Ok(())
    }

    /// Clears the provider configuration.
    #[allow(dead_code)] // API completeness - provider lifecycle
    pub async fn clear(&self) {
        *self.api_key.write().await = None;
        *self.base_url.write().await = None;
        info!(provider = %self.provider_name, "Custom provider cleared");
    }

    /// Checks if the provider is properly configured.
    pub fn is_configured(&self) -> bool {
        self.api_key
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
            && self
                .base_url
                .try_read()
                .map(|guard| guard.is_some())
                .unwrap_or(false)
    }

    /// Gets the API key if configured.
    #[allow(dead_code)] // API completeness - provider inspection
    pub async fn get_api_key(&self) -> Option<String> {
        self.api_key.read().await.clone()
    }

    /// Gets the base URL if configured.
    pub async fn get_base_url(&self) -> Option<String> {
        self.base_url.read().await.clone()
    }

    /// Gets the provider name.
    #[allow(dead_code)] // API completeness - provider inspection
    pub fn provider_name(&self) -> &str {
        &self.provider_name
    }

    /// Makes a completion request to the API.
    #[instrument(
        name = "openai_compat_complete",
        skip(self, prompt, system_prompt),
        fields(
            provider = %self.provider_name,
            model = %model,
            prompt_len = prompt.len()
        )
    )]
    pub async fn complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: &str,
        temperature: f32,
        max_tokens: usize,
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Result<LLMResponse, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured(self.provider_name.clone()))?;

        let base_url = self.base_url.read().await.clone().ok_or_else(|| {
            LLMError::NotConfigured(format!("Base URL not set for {}", self.provider_name))
        })?;

        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_text.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ];

        let request_body = ChatRequest {
            model: model.to_string(),
            messages,
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            reasoning_effort: reasoning_effort.map(|e| e.as_str().to_string()),
        };

        let url = format!("{}/chat/completions", base_url);

        debug!(
            model = model,
            temperature = temperature,
            max_tokens = max_tokens,
            url = %url,
            "Making request to OpenAI-compatible API"
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() {
            let error_msg =
                if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(&body) {
                    error_response
                        .message
                        .map(|e| e.message)
                        .unwrap_or_else(|| body.clone())
                } else {
                    body.clone()
                };
            return Err(LLMError::RequestFailed(format!(
                "{} API error ({}): {}",
                self.provider_name, status, error_msg
            )));
        }

        let chat_response: ChatResponse = serde_json::from_str(&body).map_err(|e| {
            LLMError::RequestFailed(format!(
                "Failed to parse {} response: {}. Body: {}",
                self.provider_name,
                e,
                &body[..body.len().min(500)]
            ))
        })?;

        let choice = chat_response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| LLMError::RequestFailed("No choices in response".to_string()))?;

        let parsed = choice.message.content;
        let content = parsed.text;
        let mut thinking_content = parsed.thinking;
        let finish_reason = choice.finish_reason;

        // Best-effort: extract thinking from alternative response formats
        // if not already found via content blocks
        if thinking_content.is_none() {
            thinking_content = Self::extract_thinking_from_raw(&body);
        }

        let (tokens_input, tokens_output) = if let Some(usage) = chat_response.usage {
            (usage.prompt_tokens, usage.completion_tokens)
        } else {
            let estimate = |text: &str| -> usize {
                let word_count = text.split_whitespace().count();
                ((word_count as f64) * 1.5).ceil() as usize
            };
            (estimate(prompt) + estimate(system_text), estimate(&content))
        };

        info!(
            provider = %self.provider_name,
            tokens_input = tokens_input,
            tokens_output = tokens_output,
            response_len = content.len(),
            has_thinking = thinking_content.is_some(),
            "Custom provider completion successful"
        );

        Ok(LLMResponse {
            content,
            tokens_input,
            tokens_output,
            model: model.to_string(),
            provider: ProviderType::Custom(self.provider_name.clone()),
            finish_reason,
            thinking_tokens: thinking_content
                .as_ref()
                .map(|t| crate::llm::utils::estimate_tokens(t)),
            thinking_content,
        })
    }

    /// Makes a completion request with function calling support.
    #[instrument(
        name = "openai_compat_complete_with_tools",
        skip(self, messages, tools, tool_choice),
        fields(provider = %self.provider_name, model = %model, tools_count = tools.len())
    )]
    pub async fn complete_with_tools(
        &self,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
        tool_choice: Option<serde_json::Value>,
        model: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<serde_json::Value, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured(self.provider_name.clone()))?;

        let base_url = self.base_url.read().await.clone().ok_or_else(|| {
            LLMError::NotConfigured(format!("Base URL not set for {}", self.provider_name))
        })?;

        // Apply prompt cache control to system message for providers that support it
        // (required for Anthropic Claude, harmlessly ignored by others)
        let cached_messages = apply_prompt_cache_control(messages);

        let request_body = ToolChatRequest {
            model: model.to_string(),
            messages: cached_messages,
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            tools: if tools.is_empty() {
                None
            } else {
                Some(tools.to_vec())
            },
            tool_choice,
        };

        let url = format!("{}/chat/completions", base_url);

        debug!(
            model = model,
            temperature = temperature,
            max_tokens = max_tokens,
            tools_count = request_body.tools.as_ref().map(|t| t.len()).unwrap_or(0),
            "Making request with tools to OpenAI-compatible API"
        );

        let response = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("Failed to read response body: {}", e)))?;

        if !status.is_success() {
            let error_msg =
                if let Ok(error_response) = serde_json::from_str::<ApiErrorResponse>(&body) {
                    error_response
                        .message
                        .map(|e| e.message)
                        .unwrap_or_else(|| body.clone())
                } else {
                    body.clone()
                };
            return Err(LLMError::RequestFailed(format!(
                "{} API error ({}): {}",
                self.provider_name, status, error_msg
            )));
        }

        let json_response: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            LLMError::RequestFailed(format!(
                "Failed to parse {} response: {}. Body: {}",
                self.provider_name,
                e,
                &body[..body.len().min(500)]
            ))
        })?;

        if let Some(usage) = json_response.get("usage") {
            let prompt_tokens = usage
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let completion_tokens = usage
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            info!(
                provider = %self.provider_name,
                tokens_input = prompt_tokens,
                tokens_output = completion_tokens,
                "Custom provider tool completion successful"
            );
        }

        Ok(json_response)
    }

    /// Best-effort extraction of thinking content from raw JSON response body.
    ///
    /// Delegates to [`crate::llm::utils::extract_thinking_from_message`] after
    /// parsing the JSON and navigating to `choices[0].message`.
    fn extract_thinking_from_raw(body: &str) -> Option<String> {
        let json: serde_json::Value = serde_json::from_str(body).ok()?;
        let message = json.pointer("/choices/0/message")?;
        crate::llm::utils::extract_thinking_from_message(message)
    }

    /// Tests connection by making a GET request to `{base_url}/models`.
    pub async fn test_connection(&self) -> Result<bool, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured(self.provider_name.clone()))?;

        let base_url = self.base_url.read().await.clone().ok_or_else(|| {
            LLMError::NotConfigured(format!("Base URL not set for {}", self.provider_name))
        })?;

        let url = format!("{}/models", base_url);

        let response = self
            .http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| LLMError::ConnectionError(format!("Connection failed: {}", e)))?;

        Ok(response.status().is_success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_new() {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        );
        let provider = OpenAiCompatibleProvider::new("routerlab", http_client);
        assert_eq!(provider.provider_name(), "routerlab");
        assert!(!provider.is_configured());
    }

    #[tokio::test]
    async fn test_provider_configure() {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        );
        let provider = OpenAiCompatibleProvider::new("test", http_client);

        let result = provider
            .configure("test-key", "https://api.example.com/v1")
            .await;
        assert!(result.is_ok());
        assert!(provider.is_configured());

        assert_eq!(provider.get_api_key().await, Some("test-key".to_string()));
        assert_eq!(
            provider.get_base_url().await,
            Some("https://api.example.com/v1".to_string())
        );
    }

    #[tokio::test]
    async fn test_provider_configure_trailing_slash() {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        );
        let provider = OpenAiCompatibleProvider::new("test", http_client);

        provider
            .configure("key", "https://api.example.com/v1/")
            .await
            .expect("configure should succeed");

        assert_eq!(
            provider.get_base_url().await,
            Some("https://api.example.com/v1".to_string())
        );
    }

    #[tokio::test]
    async fn test_provider_clear() {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        );
        let provider = OpenAiCompatibleProvider::new("test", http_client);

        provider
            .configure("key", "https://api.example.com/v1")
            .await
            .expect("configure should succeed");
        assert!(provider.is_configured());

        provider.clear().await;
        assert!(!provider.is_configured());
    }

    #[tokio::test]
    async fn test_provider_empty_api_key() {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
        );
        let provider = OpenAiCompatibleProvider::new("test", http_client);

        let result = provider.configure("", "https://api.example.com/v1").await;
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_standard_content() {
        let json = r#"{"role": "assistant", "content": "Hello world"}"#;
        let msg: ChatResponseMessage = serde_json::from_str(json).expect("parse should succeed");
        assert_eq!(msg.content.text, "Hello world");
        assert!(msg.content.thinking.is_none());
    }

    #[test]
    fn test_deserialize_reasoning_content() {
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [{"type": "text", "text": "Let me think..."}]},
                {"type": "text", "text": "The answer is 42"}
            ]
        }"#;
        let msg: ChatResponseMessage = serde_json::from_str(json).expect("parse should succeed");
        assert_eq!(msg.content.text, "The answer is 42");
        assert_eq!(msg.content.thinking, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_extract_thinking_from_raw_reasoning_field() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": "Answer", "reasoning": "Step 1..."}}]}"#;
        let result = OpenAiCompatibleProvider::extract_thinking_from_raw(body);
        assert_eq!(result, Some("Step 1...".to_string()));
    }

    #[test]
    fn test_extract_thinking_from_raw_reasoning_details() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": "Answer", "reasoning_details": [{"text": "Step 1"}, {"text": "Step 2"}]}}]}"#;
        let result = OpenAiCompatibleProvider::extract_thinking_from_raw(body);
        assert_eq!(result, Some("Step 1\nStep 2".to_string()));
    }

    #[test]
    fn test_extract_thinking_from_raw_none() {
        let body = r#"{"choices": [{"message": {"role": "assistant", "content": "Answer"}}]}"#;
        let result = OpenAiCompatibleProvider::extract_thinking_from_raw(body);
        assert!(result.is_none());
    }

    #[test]
    fn test_cache_control_system_only_short_conversation() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are a helpful assistant."}),
            serde_json::json!({"role": "user", "content": "Hello"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // System message should be converted to multipart with cache_control
        let system = &result[0];
        assert_eq!(system["role"], "system");
        let content = system["content"]
            .as_array()
            .expect("content should be array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "You are a helpful assistant.");
        assert_eq!(content[0]["cache_control"]["type"], "ephemeral");

        // User message should remain unchanged (only 2 messages, no extra breakpoints)
        let user = &result[1];
        assert_eq!(user["role"], "user");
        assert_eq!(user["content"], "Hello");
    }

    #[test]
    fn test_cache_control_no_system() {
        let messages = vec![
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Hi there"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // Messages without system role should be unchanged
        assert_eq!(result[0]["content"], "Hello");
        assert_eq!(result[1]["content"], "Hi there");
    }

    #[test]
    fn test_cache_control_already_multipart() {
        let messages = vec![serde_json::json!({
            "role": "system",
            "content": [{"type": "text", "text": "Already multipart"}]
        })];

        let result = apply_prompt_cache_control(&messages);

        // Non-string content should pass through unchanged
        let content = result[0]["content"]
            .as_array()
            .expect("should remain array");
        assert_eq!(content[0]["text"], "Already multipart");
        assert!(content[0].get("cache_control").is_none());
    }

    #[test]
    fn test_cache_control_multi_breakpoint_with_tools() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Do something"}),
            serde_json::json!({"role": "assistant", "content": "I will use a tool"}),
            serde_json::json!({"role": "tool", "content": "Tool result 1", "tool_call_id": "call_1", "name": "MyTool"}),
            serde_json::json!({"role": "assistant", "content": "Based on results..."}),
            serde_json::json!({"role": "tool", "content": "Tool result 2", "tool_call_id": "call_2", "name": "MyTool"}),
            serde_json::json!({"role": "assistant", "content": "Final answer"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // BP1: System message marked
        assert!(result[0]["content"].is_array());
        assert_eq!(
            result[0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );

        // User: unchanged
        assert_eq!(result[1]["content"], "Do something");

        // First assistant: unchanged (not the last before tool)
        assert_eq!(result[2]["content"], "I will use a tool");

        // First tool: unchanged
        assert_eq!(result[3]["content"], "Tool result 1");

        // BP2: Last assistant before last tool (index 4)
        assert!(result[4]["content"].is_array());
        assert_eq!(
            result[4]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(result[4]["content"][0]["text"], "Based on results...");

        // BP3: Last tool message (index 5)
        assert!(result[5]["content"].is_array());
        assert_eq!(
            result[5]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(result[5]["content"][0]["text"], "Tool result 2");
        // Tool fields preserved
        assert_eq!(result[5]["tool_call_id"], "call_2");
        assert_eq!(result[5]["name"], "MyTool");

        // Last assistant: unchanged (new content, not cached)
        assert_eq!(result[6]["content"], "Final answer");
    }

    #[test]
    fn test_cache_control_no_tool_messages() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Response 1"}),
            serde_json::json!({"role": "user", "content": "Follow up"}),
            serde_json::json!({"role": "assistant", "content": "Response 2"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // BP1: System marked
        assert!(result[0]["content"].is_array());
        assert_eq!(
            result[0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );

        // No tool messages, so last_tool_idx is None.
        // Last message (index 4) is excluded from stable prefix.
        // BP2: assistant at index 2 (last assistant in stable prefix)
        assert!(result[2]["content"].is_array());
        assert_eq!(result[2]["content"][0]["text"], "Response 1");
        assert_eq!(
            result[2]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );

        // Other messages unchanged
        assert_eq!(result[1]["content"], "Hello");
        assert_eq!(result[3]["content"], "Follow up");
        assert_eq!(result[4]["content"], "Response 2"); // Last msg: new content, not marked
    }

    #[test]
    fn test_cache_control_last_message_is_assistant() {
        // When last message is assistant (no trailing tool_result)
        // BP2 should be the second-to-last assistant (before the last tool)
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Do something"}),
            serde_json::json!({"role": "assistant", "content": "Using tool"}),
            serde_json::json!({"role": "tool", "content": "Result", "tool_call_id": "call_1"}),
            serde_json::json!({"role": "assistant", "content": "New content"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // BP1: System
        assert!(result[0]["content"].is_array());

        // BP2: Assistant at index 2 (last assistant before last tool)
        assert!(result[2]["content"].is_array());
        assert_eq!(result[2]["content"][0]["text"], "Using tool");

        // BP3: Last tool at index 3
        assert!(result[3]["content"].is_array());
        assert_eq!(result[3]["content"][0]["text"], "Result");

        // Last assistant unchanged (new content)
        assert_eq!(result[4]["content"], "New content");
    }

    #[test]
    fn test_cache_control_idempotent() {
        // Realistic scenario: system + user + assistant + tool + assistant (new)
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Using tool"}),
            serde_json::json!({"role": "tool", "content": "Result", "tool_call_id": "call_1"}),
            serde_json::json!({"role": "assistant", "content": "Final answer"}),
        ];

        let first_pass = apply_prompt_cache_control(&messages);
        let second_pass = apply_prompt_cache_control(&first_pass);

        // Second pass should not add more breakpoints (multipart content is skipped)
        // System already multipart: skipped
        assert!(second_pass[0]["content"].is_array());
        let system_content = second_pass[0]["content"].as_array().unwrap();
        assert_eq!(system_content.len(), 1); // Still 1 part, not doubled

        // BP2 (assistant index 2) and BP3 (tool index 3) already multipart: skipped
        assert!(second_pass[2]["content"].is_array());
        assert!(second_pass[3]["content"].is_array());

        // Last message still plain string (new content, never marked)
        assert_eq!(second_pass[4]["content"], "Final answer");
    }

    #[test]
    fn test_cache_control_max_three_breakpoints() {
        // With many assistant/tool pairs, should only mark 3 positions
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System"}),
            serde_json::json!({"role": "user", "content": "Go"}),
            serde_json::json!({"role": "assistant", "content": "A1"}),
            serde_json::json!({"role": "tool", "content": "T1", "tool_call_id": "c1"}),
            serde_json::json!({"role": "assistant", "content": "A2"}),
            serde_json::json!({"role": "tool", "content": "T2", "tool_call_id": "c2"}),
            serde_json::json!({"role": "assistant", "content": "A3"}),
            serde_json::json!({"role": "tool", "content": "T3", "tool_call_id": "c3"}),
            serde_json::json!({"role": "assistant", "content": "A4"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // Count how many messages have cache_control
        let marked_count = result
            .iter()
            .filter(|msg| {
                msg["content"]
                    .as_array()
                    .map(|arr| arr.iter().any(|part| part.get("cache_control").is_some()))
                    .unwrap_or(false)
            })
            .count();

        // Should be exactly 3: system (BP1), last assistant before last tool (BP2=A3), last tool (BP3=T3)
        assert_eq!(marked_count, 3);

        // Verify correct positions
        assert!(result[0]["content"].is_array()); // System BP1
        assert_eq!(result[6]["content"][0]["text"], "A3"); // BP2
        assert_eq!(result[7]["content"][0]["text"], "T3"); // BP3

        // A4 (last assistant, new content) should NOT be marked
        assert_eq!(result[8]["content"], "A4");
    }
}
