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

//! Ollama local provider implementation using rig-core

use super::provider::{LLMError, LLMProvider, LLMResponse, ProviderType};
use crate::models::agent::ReasoningEffort;
use crate::tools::utils::safe_truncate;
use async_trait::async_trait;
use rig::client::Nothing;
use rig::completion::Prompt;
use rig::providers::ollama;

// Trait required for .agent() method on rig::client::Client
use rig::client::CompletionClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

/// Default Ollama server URL
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Ollama local provider implementation
pub struct OllamaProvider {
    /// Ollama client
    client: Arc<RwLock<Option<ollama::Client>>>,
    /// Server URL
    server_url: Arc<RwLock<String>>,
    /// Configured flag
    configured: Arc<RwLock<bool>>,
    /// Shared HTTP client for direct API calls (connection pooling)
    http_client: Arc<reqwest::Client>,
}

#[allow(dead_code)]
impl OllamaProvider {
    /// Creates a new Ollama provider with default settings and a shared HTTP client.
    ///
    /// The HTTP client is used for direct API calls (thinking models, tool calls)
    /// and provides connection pooling for better performance.
    pub fn new(http_client: Arc<reqwest::Client>) -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            server_url: Arc::new(RwLock::new(DEFAULT_OLLAMA_URL.to_string())),
            configured: Arc::new(RwLock::new(false)),
            http_client,
        }
    }

    /// Creates a new Ollama provider with a custom server URL and a default HTTP client.
    ///
    /// Note: For production use, prefer using `new()` with a shared HTTP client
    /// from ProviderManager to benefit from connection pooling.
    ///
    /// # Errors
    /// Returns an error if the HTTP client fails to initialize.
    pub fn with_url(url: &str) -> Result<Self, String> {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?,
        );
        Ok(Self {
            client: Arc::new(RwLock::new(None)),
            server_url: Arc::new(RwLock::new(url.to_string())),
            configured: Arc::new(RwLock::new(false)),
            http_client,
        })
    }

    /// Configures the provider (connects to the Ollama server)
    pub async fn configure(&self, url: Option<&str>) -> Result<(), LLMError> {
        let server_url = url.unwrap_or(DEFAULT_OLLAMA_URL);
        *self.server_url.write().await = server_url.to_string();

        // Create client with custom URL if provided
        let client = if server_url != DEFAULT_OLLAMA_URL {
            ollama::Client::builder()
                .api_key(Nothing)
                .base_url(server_url)
                .build()
                .map_err(|e| {
                    LLMError::ConnectionError(format!("Failed to create Ollama client: {}", e))
                })?
        } else {
            ollama::Client::new(Nothing).map_err(|e| {
                LLMError::ConnectionError(format!("Failed to create Ollama client: {}", e))
            })?
        };

        *self.client.write().await = Some(client);
        *self.configured.write().await = true;

        info!(url = server_url, "Ollama provider configured");
        Ok(())
    }

    /// Clears the provider configuration
    pub async fn clear(&self) {
        *self.client.write().await = None;
        *self.configured.write().await = false;
        info!("Ollama provider cleared");
    }

    /// Gets the current server URL
    pub async fn get_server_url(&self) -> String {
        self.server_url.read().await.clone()
    }

    /// Tests connection to the Ollama server
    pub async fn test_connection(&self) -> Result<bool, LLMError> {
        let url = self.server_url.read().await.clone();
        let test_url = format!("{}/api/version", url);

        let response = self
            .http_client
            .get(&test_url)
            .send()
            .await
            .map_err(|e| LLMError::ConnectionError(e.to_string()))?;

        Ok(response.status().is_success())
    }

    /// Makes a direct HTTP call to Ollama API with function calling support.
    ///
    /// This method sends tools definitions and handles tool_calls in responses.
    /// Uses Ollama's OpenAI-compatible API endpoint for tools.
    ///
    /// # Arguments
    /// * `messages` - Conversation history as JSON messages
    /// * `tools` - Tool definitions in OpenAI format
    /// * `model` - Model to use (must support tools: qwen2.5, llama3.1+, mistral)
    /// * `temperature` - Sampling temperature
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    /// Raw JSON response from the API (caller should use adapter to parse)
    ///
    /// # Note
    /// Not all Ollama models support tools. Recommended models:
    /// - qwen2.5 (best tool support)
    /// - llama3.1, llama3.2
    /// - mistral, mistral-nemo
    #[instrument(
        name = "ollama_complete_with_tools",
        skip(self, messages, tools),
        fields(provider = "ollama", model = %model, tools_count = tools.len())
    )]
    pub async fn complete_with_tools(
        &self,
        messages: &[serde_json::Value],
        tools: &[serde_json::Value],
        model: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<serde_json::Value, LLMError> {
        let server_url = self.server_url.read().await.clone();
        let url = format!("{}/api/chat", server_url);

        // Build request body with tools
        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens
            }
        });

        // Add tools if provided
        if !tools.is_empty() {
            body["tools"] = serde_json::json!(tools);
        }

        debug!(
            model = model,
            temperature = temperature,
            max_tokens = max_tokens,
            tools_count = tools.len(),
            "Making Ollama API request with tools"
        );

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ConnectionError(format!(
                    "Cannot connect to Ollama server at {}: {}",
                    server_url, e
                ))
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to read Ollama response: {}", e))
        })?;

        if !status.is_success() {
            return Err(LLMError::RequestFailed(format!(
                "Ollama API error ({}): {}",
                status, response_text
            )));
        }

        // Parse to JSON Value (caller will use adapter to extract specific fields)
        let json_response: serde_json::Value =
            serde_json::from_str(&response_text).map_err(|e| {
                LLMError::RequestFailed(format!(
                    "Failed to parse Ollama response: {}. Body: {}",
                    e,
                    safe_truncate(&response_text, 500, true)
                ))
            })?;

        // Log basic info
        let has_tool_calls = json_response
            .pointer("/message/tool_calls")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false);

        info!(
            has_tool_calls = has_tool_calls,
            done = json_response
                .get("done")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            "Ollama tool completion successful"
        );

        Ok(json_response)
    }

    /// Makes a direct HTTP call to Ollama API with thinking support.
    ///
    /// Sends the `think` parameter to enable reasoning in supported models.
    /// Extracts `message.thinking` from the response as thinking content.
    ///
    /// # Arguments
    /// * `prompt` - The user prompt
    /// * `system_text` - System prompt text
    /// * `model` - Model name
    /// * `temperature` - Sampling temperature
    /// * `max_tokens` - Maximum tokens to generate
    /// * `effort` - Reasoning effort level (mapped to think parameter)
    async fn thinking_complete(
        &self,
        prompt: &str,
        system_text: &str,
        model: &str,
        temperature: f32,
        max_tokens: usize,
        effort: &ReasoningEffort,
    ) -> Result<LLMResponse, LLMError> {
        let server_url = self.server_url.read().await.clone();
        let url = format!("{}/api/chat", server_url);

        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_text },
                { "role": "user", "content": prompt }
            ],
            "stream": false,
            "think": true,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens
            }
        });

        // Ollama only supports think: true/false (no granular effort levels).
        // The effort parameter is accepted for future compatibility but currently
        // all levels map to think: true.
        if *effort != ReasoningEffort::High {
            warn!(
                model = model,
                effort = ?effort,
                "Ollama does not support granular reasoning effort; using think=true regardless"
            );
        }

        debug!(model = model, "Making Ollama API request with thinking");

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ConnectionError(format!(
                    "Cannot connect to Ollama server at {}: {}",
                    server_url, e
                ))
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to read Ollama response: {}", e))
        })?;

        if !status.is_success() {
            return Err(LLMError::RequestFailed(format!(
                "Ollama API error ({}): {}",
                status, response_text
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&response_text).map_err(|e| {
            LLMError::RequestFailed(format!("Failed to parse Ollama response: {}", e))
        })?;

        let content = json
            .pointer("/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let thinking_content = json
            .pointer("/message/thinking")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());

        // Use actual token counts from Ollama if available
        let tokens_input = json
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| {
                crate::llm::utils::estimate_tokens(prompt) as u64
                    + crate::llm::utils::estimate_tokens(system_text) as u64
            }) as usize;

        let tokens_output = json
            .get("eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or_else(|| crate::llm::utils::estimate_tokens(&content) as u64)
            as usize;

        info!(
            tokens_input = tokens_input,
            tokens_output = tokens_output,
            response_len = content.len(),
            has_thinking = thinking_content.is_some(),
            "Ollama thinking completion successful"
        );

        Ok(LLMResponse {
            content,
            tokens_input,
            tokens_output,
            model: model.to_string(),
            provider: ProviderType::Ollama,
            finish_reason: Some("stop".to_string()),
            thinking_tokens: thinking_content
                .as_ref()
                .map(|t| crate::llm::utils::estimate_tokens(t)),
            thinking_content,
        })
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Ollama
    }

    fn available_models(&self) -> Vec<String> {
        Vec::new()
    }

    fn default_model(&self) -> String {
        String::new()
    }

    fn is_configured(&self) -> bool {
        // Use try_read to avoid blocking - returns false if lock unavailable
        self.configured
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    #[instrument(
        name = "ollama_complete",
        skip(self, prompt, system_prompt),
        fields(
            provider = "ollama",
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
        reasoning_effort: Option<ReasoningEffort>,
    ) -> Result<LLMResponse, LLMError> {
        let model_name = model.unwrap_or("llama3.2");
        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");

        // When reasoning_effort is set, use direct HTTP call to send `think` parameter
        if let Some(ref effort) = reasoning_effort {
            debug!(
                model = model_name,
                effort = ?effort,
                "Using direct HTTP call for Ollama thinking model"
            );
            return self
                .thinking_complete(
                    prompt,
                    system_text,
                    model_name,
                    temperature,
                    max_tokens,
                    effort,
                )
                .await;
        }

        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| LLMError::NotConfigured("Ollama".to_string()))?;

        debug!(
            model = model_name,
            temperature = temperature,
            max_tokens = max_tokens,
            "Starting Ollama completion"
        );

        let tokens_input_estimate = crate::llm::utils::estimate_tokens(prompt)
            + crate::llm::utils::estimate_tokens(system_text);

        // Build agent and execute prompt
        let agent = client
            .agent(model_name)
            .preamble(system_text)
            .temperature(temperature as f64)
            .max_tokens(max_tokens as u64)
            .build();

        let response = agent.prompt(prompt).await.map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("connection") || err_str.contains("refused") {
                LLMError::ConnectionError(format!(
                    "Cannot connect to Ollama server. Make sure Ollama is running: {}",
                    err_str
                ))
            } else if err_str.contains("not found") || err_str.contains("model") {
                LLMError::ModelNotFound(format!(
                    "Model '{}' not found. Try: ollama pull {}",
                    model_name, model_name
                ))
            } else {
                LLMError::RequestFailed(err_str)
            }
        })?;

        let tokens_output_estimate = crate::llm::utils::estimate_tokens(&response);

        info!(
            tokens_input = tokens_input_estimate,
            tokens_output = tokens_output_estimate,
            response_len = response.len(),
            "Ollama completion successful"
        );

        Ok(LLMResponse {
            content: response,
            tokens_input: tokens_input_estimate,
            tokens_output: tokens_output_estimate,
            model: model_name.to_string(),
            provider: ProviderType::Ollama,
            finish_reason: Some("stop".to_string()),
            thinking_content: None,
            thinking_tokens: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates an OllamaProvider with a test HTTP client.
    fn test_ollama_provider() -> OllamaProvider {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("test HTTP client"),
        );
        OllamaProvider::new(http_client)
    }

    #[test]
    fn test_ollama_provider_new() {
        let provider = test_ollama_provider();
        assert_eq!(provider.provider_type(), ProviderType::Ollama);
    }

    #[test]
    fn test_ollama_available_models_empty() {
        // Models are now managed in DB, not hardcoded
        let provider = test_ollama_provider();
        let models = provider.available_models();
        assert!(models.is_empty());
    }

    #[test]
    fn test_ollama_default_model_empty() {
        // Default model is now managed in DB, not hardcoded
        let provider = test_ollama_provider();
        assert!(provider.default_model().is_empty());
    }

    #[tokio::test]
    async fn test_ollama_provider_configure() {
        let provider = test_ollama_provider();

        // Initially not configured
        assert!(!provider.is_configured());

        // Configure
        let result = provider.configure(None).await;
        assert!(result.is_ok());

        // Now should be configured
        assert!(provider.is_configured());

        // Check default URL
        assert_eq!(provider.get_server_url().await, DEFAULT_OLLAMA_URL);

        // Clear
        provider.clear().await;
        assert!(!provider.is_configured());
    }

    #[tokio::test]
    async fn test_ollama_provider_custom_url() {
        let provider = test_ollama_provider();

        let custom_url = "http://192.168.1.100:11434";
        provider.configure(Some(custom_url)).await.unwrap();

        assert_eq!(provider.get_server_url().await, custom_url);
    }

    #[tokio::test]
    async fn test_ollama_provider_complete_not_configured() {
        let provider = test_ollama_provider();

        let result = provider
            .complete("Hello", None, None, 0.7, 1000, None)
            .await;

        assert!(result.is_err());
        match result {
            Err(LLMError::NotConfigured(_)) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[test]
    fn test_ollama_with_url() {
        let custom_url = "http://localhost:11435";
        let provider = OllamaProvider::with_url(custom_url).expect("test with_url");
        assert_eq!(provider.provider_type(), ProviderType::Ollama);
    }
}
