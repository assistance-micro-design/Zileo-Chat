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

//! Mistral AI provider implementation using rig-core
//!
//! Supports both standard chat models and reasoning models (Magistral).
//! Reasoning models return a different response format with thinking blocks
//! that requires custom HTTP handling.

use super::provider::{LLMError, LLMProvider, LLMResponse, ProviderType};
use async_trait::async_trait;
use rig::completion::Prompt;
use rig::providers::mistral;
use serde::{Deserialize, Serialize};

// Trait required for .agent() method on rig::client::Client
use rig::client::CompletionClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

// ============================================================================
// Mistral API Response Types (supporting both standard and reasoning models)
// ============================================================================

/// API request body for Mistral chat completions
#[derive(Debug, Serialize)]
struct MistralChatRequest {
    model: String,
    messages: Vec<MistralMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

/// Message in Mistral API format
#[derive(Debug, Serialize, Deserialize)]
struct MistralMessage {
    role: String,
    content: String,
}

/// API response from Mistral (handles both standard and reasoning models)
#[derive(Debug, Deserialize)]
struct MistralChatResponse {
    #[allow(dead_code)]
    id: Option<String>,
    choices: Vec<MistralChoice>,
    usage: Option<MistralUsage>,
}

/// Choice in API response
#[derive(Debug, Deserialize)]
struct MistralChoice {
    message: MistralResponseMessage,
    finish_reason: Option<String>,
}

/// Parsed content from Mistral API response, separating text from thinking blocks
#[derive(Debug, Clone)]
pub struct ParsedContent {
    /// The text content (final answer)
    pub text: String,
    /// Thinking content from reasoning models (if present)
    pub thinking: Option<String>,
}

/// Response message - content can be string or array of content blocks
#[derive(Debug, Deserialize)]
struct MistralResponseMessage {
    #[allow(dead_code)]
    role: String,
    /// Content can be a simple string or an array of content blocks (reasoning models)
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
struct MistralUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

/// API error response
#[derive(Debug, Deserialize)]
struct MistralErrorResponse {
    #[serde(alias = "error")]
    message: Option<MistralErrorDetail>,
}

/// Error detail in API response
#[derive(Debug, Deserialize)]
struct MistralErrorDetail {
    message: String,
}

// ============================================================================
// Function Calling Types (JSON format - OpenAI compatible)
// ============================================================================

/// API request body for Mistral chat completions with tools
#[derive(Debug, Serialize)]
struct MistralToolChatRequest {
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

/// API response from Mistral with tool calls (used for JSON deserialization)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MistralToolChatResponse {
    id: Option<String>,
    choices: Vec<MistralToolChoice>,
    usage: Option<MistralUsage>,
}

/// Choice in API response with potential tool calls (used for JSON deserialization)
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct MistralToolChoice {
    message: MistralToolResponseMessage,
    finish_reason: Option<String>,
}

/// Response message with optional tool calls (used for JSON deserialization)
#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
struct MistralToolResponseMessage {
    role: String,
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<serde_json::Value>>,
}

/// Custom deserializer for content field that handles both string and array formats.
/// Returns ParsedContent with separated text and thinking content.
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

        // Standard models: content is a string
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

        // Reasoning models: content is an array of blocks
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

/// Mistral AI provider implementation
pub struct MistralProvider {
    /// Mistral client (wrapped in RwLock for interior mutability)
    client: Arc<RwLock<Option<mistral::Client>>>,
    /// API key (stored for reconfiguration)
    api_key: Arc<RwLock<Option<String>>>,
    /// Shared HTTP client for direct API calls (connection pooling)
    http_client: Arc<reqwest::Client>,
}

/// Mistral API base URL
const MISTRAL_API_URL: &str = "https://api.mistral.ai/v1/chat/completions";

#[allow(dead_code)]
impl MistralProvider {
    /// Creates a new unconfigured Mistral provider with a shared HTTP client.
    ///
    /// The HTTP client is used for direct API calls (reasoning models, tool calls)
    /// and provides connection pooling for better performance.
    pub fn new(http_client: Arc<reqwest::Client>) -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(None)),
            http_client,
        }
    }

    /// Creates a new Mistral provider with the given API key and a default HTTP client.
    ///
    /// Note: For production use, prefer using `new()` with a shared HTTP client
    /// from ProviderManager to benefit from connection pooling.
    pub fn with_api_key(api_key: &str) -> Result<Self, LLMError> {
        let client = mistral::Client::new(api_key).map_err(|e| {
            LLMError::RequestFailed(format!("Failed to create Mistral client: {}", e))
        })?;
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| {
                    LLMError::RequestFailed(format!("Failed to create HTTP client: {}", e))
                })?,
        );
        Ok(Self {
            client: Arc::new(RwLock::new(Some(client))),
            api_key: Arc::new(RwLock::new(Some(api_key.to_string()))),
            http_client,
        })
    }

    /// Configures the provider with an API key
    pub async fn configure(&self, api_key: &str) -> Result<(), LLMError> {
        let client = mistral::Client::new(api_key).map_err(|e| {
            LLMError::RequestFailed(format!("Failed to create Mistral client: {}", e))
        })?;
        *self.client.write().await = Some(client);
        *self.api_key.write().await = Some(api_key.to_string());
        info!("Mistral provider configured");
        Ok(())
    }

    /// Clears the provider configuration
    pub async fn clear(&self) {
        *self.client.write().await = None;
        *self.api_key.write().await = None;
        info!("Mistral provider cleared");
    }

    /// Gets the API key if configured
    pub async fn get_api_key(&self) -> Option<String> {
        self.api_key.read().await.clone()
    }

    /// Makes a direct HTTP call to Mistral API
    /// This is used for reasoning models that return a different response format
    async fn custom_complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<LLMResponse, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured("Mistral".to_string()))?;

        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");

        // Build messages array
        let messages = vec![
            MistralMessage {
                role: "system".to_string(),
                content: system_text.to_string(),
            },
            MistralMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ];

        let request_body = MistralChatRequest {
            model: model.to_string(),
            messages,
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
        };

        debug!(
            model = model,
            temperature = temperature,
            max_tokens = max_tokens,
            "Making direct HTTP request to Mistral API (reasoning model)"
        );

        let response = self
            .http_client
            .post(MISTRAL_API_URL)
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
            // Try to parse error response
            let error_msg =
                if let Ok(error_response) = serde_json::from_str::<MistralErrorResponse>(&body) {
                    error_response
                        .message
                        .map(|e| e.message)
                        .unwrap_or_else(|| body.clone())
                } else {
                    body.clone()
                };
            return Err(LLMError::RequestFailed(format!(
                "Mistral API error ({}): {}",
                status, error_msg
            )));
        }

        // Parse successful response
        let chat_response: MistralChatResponse = serde_json::from_str(&body).map_err(|e| {
            LLMError::RequestFailed(format!(
                "Failed to parse Mistral response: {}. Body: {}",
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
        let thinking_content = parsed.thinking;
        let finish_reason = choice.finish_reason;

        // Get token counts from usage if available, otherwise estimate
        let (tokens_input, tokens_output) = if let Some(usage) = chat_response.usage {
            (usage.prompt_tokens, usage.completion_tokens)
        } else {
            // Estimate tokens
            let estimate = |text: &str| -> usize {
                let word_count = text.split_whitespace().count();
                ((word_count as f64) * 1.5).ceil() as usize
            };
            (estimate(prompt) + estimate(system_text), estimate(&content))
        };

        if thinking_content.is_some() {
            info!(
                tokens_input = tokens_input,
                tokens_output = tokens_output,
                response_len = content.len(),
                thinking_len = thinking_content.as_ref().map_or(0, |t| t.len()),
                "Mistral reasoning model completion with thinking"
            );
        } else {
            info!(
                tokens_input = tokens_input,
                tokens_output = tokens_output,
                response_len = content.len(),
                "Mistral reasoning model completion successful"
            );
        }

        Ok(LLMResponse {
            content,
            tokens_input,
            tokens_output,
            model: model.to_string(),
            provider: ProviderType::Mistral,
            finish_reason,
            thinking_content,
        })
    }

    /// Makes a direct HTTP call to Mistral API with function calling support.
    ///
    /// This method sends tools definitions and handles tool_calls in responses.
    ///
    /// # Arguments
    /// * `messages` - Conversation history as JSON messages
    /// * `tools` - Tool definitions in OpenAI format
    /// * `tool_choice` - How the model should use tools ("auto", "any", "none")
    /// * `model` - Model to use
    /// * `temperature` - Sampling temperature
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    /// Raw JSON response from the API (caller should use adapter to parse)
    #[instrument(
        name = "mistral_complete_with_tools",
        skip(self, messages, tools, tool_choice),
        fields(provider = "mistral", model = %model, tools_count = tools.len())
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
            .ok_or_else(|| LLMError::NotConfigured("Mistral".to_string()))?;

        let request_body = MistralToolChatRequest {
            model: model.to_string(),
            messages: messages.to_vec(),
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            tools: if tools.is_empty() {
                None
            } else {
                Some(tools.to_vec())
            },
            tool_choice,
        };

        debug!(
            model = model,
            temperature = temperature,
            max_tokens = max_tokens,
            tools_count = request_body.tools.as_ref().map(|t| t.len()).unwrap_or(0),
            "Making Mistral API request with tools"
        );

        let response = self
            .http_client
            .post(MISTRAL_API_URL)
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
                if let Ok(error_response) = serde_json::from_str::<MistralErrorResponse>(&body) {
                    error_response
                        .message
                        .map(|e| e.message)
                        .unwrap_or_else(|| body.clone())
                } else {
                    body.clone()
                };
            return Err(LLMError::RequestFailed(format!(
                "Mistral API error ({}): {}",
                status, error_msg
            )));
        }

        // Parse to JSON Value (caller will use adapter to extract specific fields)
        let json_response: serde_json::Value = serde_json::from_str(&body).map_err(|e| {
            LLMError::RequestFailed(format!(
                "Failed to parse Mistral response: {}. Body: {}",
                e,
                &body[..body.len().min(500)]
            ))
        })?;

        // Log usage if available
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
                tokens_input = prompt_tokens,
                tokens_output = completion_tokens,
                "Mistral tool completion successful"
            );
        }

        Ok(json_response)
    }
}

#[async_trait]
impl LLMProvider for MistralProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Mistral
    }

    fn available_models(&self) -> Vec<String> {
        Vec::new()
    }

    fn default_model(&self) -> String {
        String::new()
    }

    fn is_configured(&self) -> bool {
        // Use try_read to avoid blocking - returns false if lock unavailable
        self.client
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    #[instrument(
        name = "mistral_complete",
        skip(self, prompt, system_prompt),
        fields(
            provider = "mistral",
            model = %model.unwrap_or("unknown"),
            prompt_len = prompt.len()
        )
    )]
    async fn complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
        is_reasoning: bool,
    ) -> Result<LLMResponse, LLMError> {
        let model_name = model.unwrap_or("mistral-large-latest");

        // Use custom HTTP client for reasoning models (e.g. Magistral)
        // because rig-core doesn't support their response format.
        // is_reasoning comes from the DB (set by user when creating the model).
        if is_reasoning {
            debug!(
                model = model_name,
                "Using custom HTTP client for reasoning model"
            );
            return self
                .custom_complete(prompt, system_prompt, model_name, temperature, max_tokens)
                .await;
        }

        // Standard models use rig-core client
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| LLMError::NotConfigured("Mistral".to_string()))?;

        debug!(
            model = model_name,
            temperature = temperature,
            max_tokens = max_tokens,
            "Starting Mistral completion"
        );

        // Include system prompt in input token count
        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");
        let tokens_input_estimate = crate::llm::utils::estimate_tokens(prompt)
            + crate::llm::utils::estimate_tokens(system_text);

        // Build agent and execute prompt
        // Use temperature and max_tokens from agent config
        let agent = client
            .agent(model_name)
            .preamble(system_text)
            .temperature(temperature as f64)
            .max_tokens(max_tokens as u64)
            .build();

        let response = agent
            .prompt(prompt)
            .await
            .map_err(|e| LLMError::RequestFailed(e.to_string()))?;

        // Estimate output tokens
        let tokens_output_estimate = crate::llm::utils::estimate_tokens(&response);

        info!(
            tokens_input = tokens_input_estimate,
            tokens_output = tokens_output_estimate,
            response_len = response.len(),
            "Mistral completion successful"
        );

        Ok(LLMResponse {
            content: response,
            tokens_input: tokens_input_estimate,
            tokens_output: tokens_output_estimate,
            model: model_name.to_string(),
            provider: ProviderType::Mistral,
            finish_reason: Some("stop".to_string()),
            thinking_content: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a MistralProvider with a test HTTP client.
    fn test_mistral_provider() -> MistralProvider {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("test HTTP client"),
        );
        MistralProvider::new(http_client)
    }

    #[test]
    fn test_mistral_provider_new() {
        let provider = test_mistral_provider();
        assert_eq!(provider.provider_type(), ProviderType::Mistral);
    }

    #[test]
    fn test_mistral_available_models_empty() {
        // Models are now managed in DB, not hardcoded
        let provider = test_mistral_provider();
        let models = provider.available_models();
        assert!(models.is_empty());
    }

    #[test]
    fn test_mistral_default_model_empty() {
        // Default model is now managed in DB, not hardcoded
        let provider = test_mistral_provider();
        assert!(provider.default_model().is_empty());
    }

    #[tokio::test]
    async fn test_mistral_provider_configure() {
        let provider = test_mistral_provider();

        // Initially not configured
        assert!(!provider.is_configured());

        // Configure with a fake key (won't make real API calls in test)
        let result = provider.configure("test-api-key").await;
        assert!(result.is_ok());

        // Now should be configured
        assert!(provider.is_configured());

        // Clear
        provider.clear().await;
        assert!(!provider.is_configured());
    }

    #[tokio::test]
    async fn test_mistral_provider_get_api_key() {
        let provider = test_mistral_provider();

        // Initially no key
        assert!(provider.get_api_key().await.is_none());

        // After configure
        provider.configure("my-secret-key").await.unwrap();
        assert_eq!(
            provider.get_api_key().await,
            Some("my-secret-key".to_string())
        );
    }

    #[tokio::test]
    async fn test_mistral_provider_complete_not_configured() {
        let provider = test_mistral_provider();

        let result = provider
            .complete("Hello", None, None, 0.7, 1000, false)
            .await;

        assert!(result.is_err());
        match result {
            Err(LLMError::NotConfigured(_)) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[test]
    fn test_deserialize_standard_content() {
        // Test standard string content - no thinking
        let json = r#"{"role": "assistant", "content": "Hello world"}"#;
        let msg: MistralResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "Hello world");
        assert!(msg.content.thinking.is_none());
    }

    #[test]
    fn test_deserialize_reasoning_content_extracts_thinking() {
        // Test reasoning model content (array format) - thinking is now captured
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [{"type": "text", "text": "Let me think..."}]},
                {"type": "text", "text": "The answer is 42"}
            ]
        }"#;
        let msg: MistralResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "The answer is 42");
        assert_eq!(msg.content.thinking, Some("Let me think...".to_string()));
    }

    #[test]
    fn test_deserialize_reasoning_multiple_thinking_blocks() {
        // Test reasoning model with multiple thinking blocks concatenated
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "thinking", "thinking": [{"type": "text", "text": "Step 1: analyze"}]},
                {"type": "thinking", "thinking": [{"type": "text", "text": "Step 2: compute"}]},
                {"type": "text", "text": "The result is 7"}
            ]
        }"#;
        let msg: MistralResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "The result is 7");
        assert_eq!(
            msg.content.thinking,
            Some("Step 1: analyze\nStep 2: compute".to_string())
        );
    }

    #[test]
    fn test_deserialize_reasoning_multiple_text_blocks() {
        // Test reasoning model with multiple text blocks, no thinking
        let json = r#"{
            "role": "assistant",
            "content": [
                {"type": "text", "text": "First part"},
                {"type": "text", "text": "Second part"}
            ]
        }"#;
        let msg: MistralResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "First part\nSecond part");
        assert!(msg.content.thinking.is_none());
    }

    #[test]
    fn test_deserialize_thinking_with_multiple_sub_blocks() {
        // Test thinking block with multiple text sub-blocks
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
        let msg: MistralResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "Final answer");
        assert_eq!(
            msg.content.thinking,
            Some("First thought\nSecond thought".to_string())
        );
    }
}
