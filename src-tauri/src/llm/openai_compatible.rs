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
use super::utils::simulate_streaming;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
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

/// Response message - content can be string or array of content blocks
#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    #[allow(dead_code)]
    role: String,
    #[serde(deserialize_with = "deserialize_content")]
    content: String,
}

/// Content block for reasoning models (thinking or text)
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "thinking")]
    Thinking {
        thinking: Vec<TextBlock>,
    },
    #[serde(rename = "text")]
    Text {
        text: String,
    },
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
/// Copied from mistral.rs to avoid modifying that file.
fn deserialize_content<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct ContentVisitor;

    impl<'de> Visitor<'de> for ContentVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or an array of content blocks")
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
            A: de::SeqAccess<'de>,
        {
            let mut result = String::new();

            while let Some(block) = seq.next_element::<ContentBlock>()? {
                match block {
                    ContentBlock::Thinking { thinking } => {
                        debug!("Reasoning model thinking blocks: {} items", thinking.len());
                    }
                    ContentBlock::Text { text } => {
                        if !result.is_empty() {
                            result.push('\n');
                        }
                        result.push_str(&text);
                    }
                }
            }

            Ok(result)
        }
    }

    deserializer.deserialize_any(ContentVisitor)
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
    ) -> Result<LLMResponse, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured(self.provider_name.clone()))?;

        let base_url = self
            .base_url
            .read()
            .await
            .clone()
            .ok_or_else(|| {
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

        let content = choice.message.content;
        let finish_reason = choice.finish_reason;

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
            "Custom provider completion successful"
        );

        Ok(LLMResponse {
            content,
            tokens_input,
            tokens_output,
            model: model.to_string(),
            provider: ProviderType::Custom(self.provider_name.clone()),
            finish_reason,
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
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
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

        let base_url = self
            .base_url
            .read()
            .await
            .clone()
            .ok_or_else(|| {
                LLMError::NotConfigured(format!("Base URL not set for {}", self.provider_name))
            })?;

        let request_body = ToolChatRequest {
            model: model.to_string(),
            messages,
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            tools: if tools.is_empty() { None } else { Some(tools) },
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

    /// Streaming completion via simulate_streaming (consistent with Mistral/Ollama).
    pub async fn complete_stream(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<mpsc::Receiver<Result<String, LLMError>>, LLMError> {
        let response = self
            .complete(prompt, system_prompt, model, temperature, max_tokens)
            .await?;
        Ok(simulate_streaming(response.content, None, None))
    }

    /// Tests connection by making a GET request to `{base_url}/models`.
    pub async fn test_connection(&self) -> Result<bool, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured(self.provider_name.clone()))?;

        let base_url = self
            .base_url
            .read()
            .await
            .clone()
            .ok_or_else(|| {
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

        assert_eq!(
            provider.get_api_key().await,
            Some("test-key".to_string())
        );
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
        assert_eq!(msg.content, "Hello world");
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
        assert_eq!(msg.content, "The answer is 42");
    }
}
