// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Ollama local provider implementation using rig-core

use super::provider::{LLMError, LLMProvider, LLMResponse, ProviderType};
use async_trait::async_trait;
use rig::completion::Prompt;
use rig::providers::ollama;

// Import trait to bring completion_model method into scope
#[allow(unused_imports)]
use rig::client::CompletionClient;
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
}

#[allow(dead_code)]
impl OllamaProvider {
    /// Creates a new Ollama provider with default settings
    pub fn new() -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            server_url: Arc::new(RwLock::new(DEFAULT_OLLAMA_URL.to_string())),
            configured: Arc::new(RwLock::new(false)),
        }
    }

    /// Creates a new Ollama provider with a custom server URL
    pub fn with_url(url: &str) -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            server_url: Arc::new(RwLock::new(url.to_string())),
            configured: Arc::new(RwLock::new(false)),
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

        let response = reqwest::get(&test_url)
            .await
            .map_err(|e| LLMError::ConnectionError(e.to_string()))?;

        Ok(response.status().is_success())
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
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

        // Estimate tokens using word-based approximation
        // French/English text averages ~1.3 tokens per word
        let estimate_tokens = |text: &str| -> usize {
            let word_count = text.split_whitespace().count();
            let estimate = ((word_count as f64) * 1.5).ceil() as usize;
            estimate.max(1)
        };

        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");
        let tokens_input_estimate = estimate_tokens(prompt) + estimate_tokens(system_text);

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

        let tokens_output_estimate = estimate_tokens(&response);

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
        let provider = OllamaProvider::new();
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
        let provider = OllamaProvider::new();
        let models = provider.available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"llama3.2".to_string()));
    }

    #[tokio::test]
    async fn test_ollama_provider_configure() {
        let provider = OllamaProvider::new();

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
        let provider = OllamaProvider::new();

        let custom_url = "http://192.168.1.100:11434";
        provider.configure(Some(custom_url)).await.unwrap();

        assert_eq!(provider.get_server_url().await, custom_url);
    }

    #[tokio::test]
    async fn test_ollama_provider_complete_not_configured() {
        let provider = OllamaProvider::new();

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
}
