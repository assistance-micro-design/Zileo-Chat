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
use async_trait::async_trait;
use rig::completion::Prompt;
use rig::providers::ollama;

// Import trait to bring completion_model method into scope
#[allow(unused_imports)]
use rig::client::CompletionClient;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, instrument, warn};

/// Available Ollama models (common defaults)
pub const OLLAMA_MODELS: &[&str] = &[
    "llama3.2",
    "llama3.1",
    "llama3",
    "mistral",
    "mixtral",
    "codellama",
    "phi3",
    "gemma2",
    "qwen2.5",
];

/// Default Ollama model
pub const DEFAULT_OLLAMA_MODEL: &str = "llama3.2";

/// Ollama models that support thinking/reasoning mode
const OLLAMA_THINKING_MODELS: &[&str] = &[
    "deepseek-r1",
    "deepseek-v3",
    "qwen3",
    "gpt-oss",
    "kimik",
    "thinking", // Catch-all for models with "thinking" in the name
];

/// Check if a model supports thinking mode
fn is_thinking_model(model: &str) -> bool {
    let model_lower = model.to_lowercase();
    OLLAMA_THINKING_MODELS
        .iter()
        .any(|m| model_lower.contains(m))
}

/// Get the appropriate think parameter value for a model
fn get_think_param(model: &str, enable_thinking: bool) -> serde_json::Value {
    if !enable_thinking {
        return serde_json::json!(false);
    }

    // gpt-oss uses level strings instead of booleans
    if model.to_lowercase().contains("gpt-oss") {
        serde_json::json!("medium")
    } else {
        serde_json::json!(true)
    }
}

/// Response structure for Ollama chat with thinking support
#[derive(Debug, Deserialize, Serialize)]
struct OllamaChatResponse {
    message: OllamaMessageResponse,
    #[serde(default)]
    done: bool,
}

/// Message response structure from Ollama
#[derive(Debug, Deserialize, Serialize)]
struct OllamaMessageResponse {
    role: String,
    content: String,
    #[serde(default)]
    thinking: Option<String>,
}

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
    /// and provides connection pooling for better performance (OPT-LLM-2).
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
    pub fn with_url(url: &str) -> Self {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("Failed to create HTTP client"),
        );
        Self {
            client: Arc::new(RwLock::new(None)),
            server_url: Arc::new(RwLock::new(url.to_string())),
            configured: Arc::new(RwLock::new(false)),
            http_client,
        }
    }

    /// Configures the provider (connects to the Ollama server)
    pub async fn configure(&self, url: Option<&str>) -> Result<(), LLMError> {
        let server_url = url.unwrap_or(DEFAULT_OLLAMA_URL);
        *self.server_url.write().await = server_url.to_string();

        // Create client with custom URL if provided
        let client = if server_url != DEFAULT_OLLAMA_URL {
            ollama::ClientBuilder::new().base_url(server_url).build()
        } else {
            ollama::Client::new()
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

    /// Complete with thinking support using direct HTTP call (bypasses rig-core)
    ///
    /// Returns a tuple of (LLMResponse, Option<thinking_content>)
    pub async fn complete_with_thinking(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
        enable_thinking: bool,
    ) -> Result<(LLMResponse, Option<String>), LLMError> {
        let server_url = self.server_url.read().await.clone();
        let url = format!("{}/api/chat", server_url);

        let model_name = model.unwrap_or(DEFAULT_OLLAMA_MODEL);
        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");

        let messages = vec![
            serde_json::json!({
                "role": "system",
                "content": system_text
            }),
            serde_json::json!({
                "role": "user",
                "content": prompt
            }),
        ];

        let body = serde_json::json!({
            "model": model_name,
            "messages": messages,
            "think": get_think_param(model_name, enable_thinking),
            "stream": false,
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens
            }
        });

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

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LLMError::RequestFailed(format!(
                "Ollama API error: {}",
                error_text
            )));
        }

        let chat_response: OllamaChatResponse = response.json().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to parse Ollama response: {}", e))
        })?;

        let thinking_content = chat_response.message.thinking;

        let tokens_input = crate::llm::utils::estimate_tokens(prompt)
            + crate::llm::utils::estimate_tokens(system_text);
        let tokens_output = crate::llm::utils::estimate_tokens(&chat_response.message.content);

        Ok((
            LLMResponse {
                content: chat_response.message.content,
                tokens_input,
                tokens_output,
                model: model_name.to_string(),
                provider: ProviderType::Ollama,
                finish_reason: Some("stop".to_string()),
            },
            thinking_content,
        ))
    }

    /// Check if a model name indicates a thinking model
    pub fn is_thinking_model_name(&self, model: &str) -> bool {
        is_thinking_model(model)
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
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
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
                    &response_text[..response_text.len().min(500)]
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
}

impl Default for OllamaProvider {
    /// Creates a default OllamaProvider with a new HTTP client.
    ///
    /// Note: For production use, prefer using `new()` with a shared HTTP client
    /// from ProviderManager to benefit from connection pooling.
    fn default() -> Self {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(300))
                .build()
                .expect("Failed to create HTTP client"),
        );
        Self::new(http_client)
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Ollama
    }

    fn available_models(&self) -> Vec<String> {
        OLLAMA_MODELS.iter().map(|s| s.to_string()).collect()
    }

    fn default_model(&self) -> String {
        DEFAULT_OLLAMA_MODEL.to_string()
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
            model = %model.unwrap_or(DEFAULT_OLLAMA_MODEL),
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
    ) -> Result<LLMResponse, LLMError> {
        let client_guard = self.client.read().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| LLMError::NotConfigured("Ollama".to_string()))?;

        let model_name = model.unwrap_or(DEFAULT_OLLAMA_MODEL);

        debug!(
            model = model_name,
            temperature = temperature,
            max_tokens = max_tokens,
            "Starting Ollama completion"
        );

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
        })
    }

    #[instrument(
        name = "ollama_complete_stream",
        skip(self, prompt, system_prompt),
        fields(
            provider = "ollama",
            model = %model.unwrap_or(DEFAULT_OLLAMA_MODEL)
        )
    )]
    async fn complete_stream(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<mpsc::Receiver<Result<String, LLMError>>, LLMError> {
        // Simulate streaming by chunking non-streaming response
        let (tx, rx) = mpsc::channel(100);

        let response = self
            .complete(prompt, system_prompt, model, temperature, max_tokens)
            .await?;

        tokio::spawn(async move {
            let content = response.content;
            let chunk_size = 20;

            for chunk in content.as_bytes().chunks(chunk_size) {
                let chunk_str = String::from_utf8_lossy(chunk).to_string();
                if tx.send(Ok(chunk_str)).await.is_err() {
                    warn!("Streaming receiver dropped");
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_models() {
        assert!(OLLAMA_MODELS.contains(&"llama3.2"));
        assert!(OLLAMA_MODELS.contains(&"mistral"));
        assert!(OLLAMA_MODELS.len() >= 5);
    }

    #[test]
    fn test_ollama_provider_new() {
        let provider = OllamaProvider::default();
        assert_eq!(provider.provider_type(), ProviderType::Ollama);
        assert_eq!(provider.default_model(), DEFAULT_OLLAMA_MODEL);
    }

    #[test]
    fn test_ollama_provider_default() {
        let provider = OllamaProvider::default();
        assert_eq!(provider.provider_type(), ProviderType::Ollama);
    }

    #[test]
    fn test_ollama_available_models() {
        let provider = OllamaProvider::default();
        let models = provider.available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"llama3.2".to_string()));
    }

    #[tokio::test]
    async fn test_ollama_provider_configure() {
        let provider = OllamaProvider::default();

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
        let provider = OllamaProvider::default();

        let custom_url = "http://192.168.1.100:11434";
        provider.configure(Some(custom_url)).await.unwrap();

        assert_eq!(provider.get_server_url().await, custom_url);
    }

    #[tokio::test]
    async fn test_ollama_provider_complete_not_configured() {
        let provider = OllamaProvider::default();

        let result = provider.complete("Hello", None, None, 0.7, 1000).await;

        assert!(result.is_err());
        match result {
            Err(LLMError::NotConfigured(_)) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[test]
    fn test_ollama_with_url() {
        let custom_url = "http://localhost:11435";
        let provider = OllamaProvider::with_url(custom_url);
        assert_eq!(provider.provider_type(), ProviderType::Ollama);
    }

    #[test]
    fn test_is_thinking_model() {
        assert!(is_thinking_model("deepseek-r1"));
        assert!(is_thinking_model("deepseek-r1:7b"));
        assert!(is_thinking_model("deepseek-v3"));
        assert!(is_thinking_model("deepseek-v3:14b"));
        assert!(is_thinking_model("qwen3"));
        assert!(is_thinking_model("qwen3:14b"));
        assert!(is_thinking_model("spiah/kimik2thinking:latest"));
        assert!(is_thinking_model("gpt-oss"));
        assert!(is_thinking_model("gpt-oss:latest"));
        assert!(is_thinking_model("my-thinking-model"));
        assert!(!is_thinking_model("llama3.2"));
        assert!(!is_thinking_model("mistral"));
        assert!(!is_thinking_model("codellama"));
    }

    #[test]
    fn test_get_think_param() {
        // Test enabled thinking
        assert_eq!(
            get_think_param("deepseek-r1", true),
            serde_json::json!(true)
        );
        assert_eq!(get_think_param("qwen3", true), serde_json::json!(true));
        assert_eq!(
            get_think_param("spiah/kimik2thinking:latest", true),
            serde_json::json!(true)
        );

        // Test gpt-oss special case
        assert_eq!(
            get_think_param("gpt-oss", true),
            serde_json::json!("medium")
        );
        assert_eq!(
            get_think_param("gpt-oss:latest", true),
            serde_json::json!("medium")
        );

        // Test disabled thinking
        assert_eq!(
            get_think_param("deepseek-r1", false),
            serde_json::json!(false)
        );
        assert_eq!(get_think_param("gpt-oss", false), serde_json::json!(false));
        assert_eq!(get_think_param("qwen3", false), serde_json::json!(false));
    }

    #[test]
    fn test_is_thinking_model_name_method() {
        let provider = OllamaProvider::default();

        assert!(provider.is_thinking_model_name("deepseek-r1"));
        assert!(provider.is_thinking_model_name("qwen3"));
        assert!(provider.is_thinking_model_name("spiah/kimik2thinking:latest"));
        assert!(!provider.is_thinking_model_name("llama3.2"));
        assert!(!provider.is_thinking_model_name("mistral"));
    }
}
