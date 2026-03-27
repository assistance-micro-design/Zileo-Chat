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

use super::cache_control::apply_prompt_cache_control;
use super::http::{self, ParsedContent};
use super::provider::{
    CompletionParams, LLMError, LLMResponse, ProviderType, ToolCompletionParams,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// API request body for chat completions
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
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
    #[serde(deserialize_with = "http::deserialize_content")]
    content: ParsedContent,
}

/// Usage statistics from API response
#[derive(Debug, Deserialize)]
struct ChatUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

/// API request body for chat completions with tools
#[derive(Debug, Serialize)]
struct ToolChatRequest {
    model: String,
    messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

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
        skip(self, params),
        fields(
            provider = %self.provider_name,
        )
    )]
    pub async fn complete(&self, params: CompletionParams) -> Result<LLMResponse, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured(self.provider_name.clone()))?;

        let base_url = self.base_url.read().await.clone().ok_or_else(|| {
            LLMError::NotConfigured(format!("Base URL not set for {}", self.provider_name))
        })?;

        let model = params.model.as_deref().unwrap_or("default");
        let system_text = params
            .system_prompt
            .as_deref()
            .unwrap_or("You are a helpful assistant.");

        let messages = vec![
            ChatMessage {
                role: "system".to_string(),
                content: system_text.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: params.prompt.clone(),
            },
        ];

        let request_body = ChatRequest {
            model: model.to_string(),
            messages,
            temperature: Some(params.temperature),
            max_tokens: Some(params.max_tokens),
            reasoning_effort: params.reasoning_effort.map(|e| e.as_str().to_string()),
        };

        let url = format!("{}/chat/completions", base_url);

        debug!(
            model = model,
            temperature = params.temperature,
            max_tokens = params.max_tokens,
            url = %url,
            "Making request to OpenAI-compatible API"
        );

        let (status, body) = http::send_and_read_body(
            self.http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await,
        )
        .await?;

        if !status.is_success() {
            return Err(http::parse_api_error(&self.provider_name, status, &body));
        }

        let chat_response: ChatResponse = http::parse_json_response(&self.provider_name, &body)?;

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

        let (tokens_input, tokens_output) = chat_response
            .usage
            .map(|u| (u.prompt_tokens, u.completion_tokens))
            .unwrap_or((0, 0));

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
        skip(self, params),
        fields(provider = %self.provider_name, model = %params.model, tools_count = params.tools.len())
    )]
    pub async fn complete_with_tools(
        &self,
        params: &ToolCompletionParams,
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
        let cached_messages = apply_prompt_cache_control(&params.messages);

        let request_body = ToolChatRequest {
            model: params.model.clone(),
            messages: cached_messages,
            temperature: Some(params.temperature),
            max_tokens: Some(params.max_tokens),
            tools: if params.tools.is_empty() {
                None
            } else {
                Some(params.tools.clone())
            },
            tool_choice: params.tool_choice.clone(),
            reasoning_effort: params
                .reasoning_effort
                .as_ref()
                .map(|e| e.as_str().to_string()),
        };

        let url = format!("{}/chat/completions", base_url);

        debug!(
            model = %params.model,
            temperature = params.temperature,
            max_tokens = params.max_tokens,
            context_window = ?params.context_window,
            reasoning_effort = ?params.reasoning_effort,
            tools_count = request_body.tools.as_ref().map(|t| t.len()).unwrap_or(0),
            "Making request with tools to OpenAI-compatible API"
        );

        let (status, body) = http::send_and_read_body(
            self.http_client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await,
        )
        .await?;

        if !status.is_success() {
            return Err(http::parse_api_error(&self.provider_name, status, &body));
        }

        let json_response: serde_json::Value =
            http::parse_json_response(&self.provider_name, &body)?;

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
    fn test_chat_response_uses_shared_content_deserializer() {
        // Integration test: ChatResponseMessage uses http::deserialize_content
        let json = r#"{"content": [
            {"type": "thinking", "thinking": "Step 1"},
            {"type": "text", "text": "Answer"}
        ]}"#;
        let msg: ChatResponseMessage = serde_json::from_str(json).expect("parse should succeed");
        assert_eq!(msg.content.text, "Answer");
        assert_eq!(msg.content.thinking, Some("Step 1".to_string()));
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
}
