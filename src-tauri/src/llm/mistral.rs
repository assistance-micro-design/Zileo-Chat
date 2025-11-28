// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Mistral AI provider implementation using rig-core

use super::provider::{LLMError, LLMProvider, LLMResponse, ProviderType};
use async_trait::async_trait;
use rig::completion::Prompt;
use rig::providers::mistral;

// Import trait to bring completion_model method into scope
#[allow(unused_imports)]
use rig::client::CompletionClient;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, instrument, warn};

/// Available Mistral models
pub const MISTRAL_MODELS: &[&str] = &[
    "mistral-large-latest",
    "mistral-medium-latest",
    "mistral-small-latest",
    "open-mistral-7b",
    "open-mixtral-8x7b",
    "open-mixtral-8x22b",
    "codestral-latest",
];

/// Default Mistral model
pub const DEFAULT_MISTRAL_MODEL: &str = "mistral-large-latest";

/// Mistral AI provider implementation
pub struct MistralProvider {
    /// Mistral client (wrapped in RwLock for interior mutability)
    client: Arc<RwLock<Option<mistral::Client>>>,
    /// API key (stored for reconfiguration)
    api_key: Arc<RwLock<Option<String>>>,
}

#[allow(dead_code)]
impl MistralProvider {
    /// Creates a new unconfigured Mistral provider
    pub fn new() -> Self {
        Self {
            client: Arc::new(RwLock::new(None)),
            api_key: Arc::new(RwLock::new(None)),
        }
    }

    /// Creates a new Mistral provider with the given API key
    pub fn with_api_key(api_key: &str) -> Result<Self, LLMError> {
        let client = mistral::Client::new(api_key);
        Ok(Self {
            client: Arc::new(RwLock::new(Some(client))),
            api_key: Arc::new(RwLock::new(Some(api_key.to_string()))),
        })
    }

    /// Configures the provider with an API key
    pub async fn configure(&self, api_key: &str) -> Result<(), LLMError> {
        let client = mistral::Client::new(api_key);
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
}

impl Default for MistralProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LLMProvider for MistralProvider {
    fn provider_type(&self) -> ProviderType {
        ProviderType::Mistral
    }

    fn available_models(&self) -> Vec<String> {
        MISTRAL_MODELS.iter().map(|s| s.to_string()).collect()
    }

    fn default_model(&self) -> String {
        DEFAULT_MISTRAL_MODEL.to_string()
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
            model = %model.unwrap_or(DEFAULT_MISTRAL_MODEL),
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
            .ok_or_else(|| LLMError::NotConfigured("Mistral".to_string()))?;

        let model_name = model.unwrap_or(DEFAULT_MISTRAL_MODEL);

        debug!(
            model = model_name,
            temperature = temperature,
            max_tokens = max_tokens,
            "Starting Mistral completion"
        );

        // Estimate tokens using word-based approximation
        // French/English text averages ~1.3 tokens per word, ~4-5 chars per word
        // Using word count * 1.5 gives better accuracy than char/4
        let estimate_tokens = |text: &str| -> usize {
            let word_count = text.split_whitespace().count();
            // Add a minimum of 1 token for very short inputs
            let estimate = ((word_count as f64) * 1.5).ceil() as usize;
            estimate.max(1)
        };

        // Include system prompt in input token count
        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");
        let tokens_input_estimate = estimate_tokens(prompt) + estimate_tokens(system_text);

        // Build agent and execute prompt
        let agent = client.agent(model_name).preamble(system_text).build();

        let response = agent
            .prompt(prompt)
            .await
            .map_err(|e| LLMError::RequestFailed(e.to_string()))?;

        // Estimate output tokens
        let tokens_output_estimate = estimate_tokens(&response);

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
        })
    }

    #[instrument(
        name = "mistral_complete_stream",
        skip(self, prompt, system_prompt),
        fields(
            provider = "mistral",
            model = %model.unwrap_or(DEFAULT_MISTRAL_MODEL)
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
        // For now, we'll simulate streaming by chunking the non-streaming response
        // True streaming will require updates to rig-core's streaming API
        let (tx, rx) = mpsc::channel(100);

        let response = self
            .complete(prompt, system_prompt, model, temperature, max_tokens)
            .await?;

        // Spawn task to send chunks
        tokio::spawn(async move {
            // Split response into chunks (simulated streaming)
            let content = response.content;
            let chunk_size = 20; // characters per chunk

            for chunk in content.as_bytes().chunks(chunk_size) {
                let chunk_str = String::from_utf8_lossy(chunk).to_string();
                if tx.send(Ok(chunk_str)).await.is_err() {
                    warn!("Streaming receiver dropped");
                    break;
                }
                // Small delay to simulate streaming
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
    fn test_mistral_models() {
        assert!(MISTRAL_MODELS.contains(&"mistral-large-latest"));
        assert!(MISTRAL_MODELS.contains(&"mistral-small-latest"));
        assert!(MISTRAL_MODELS.len() >= 5);
    }

    #[test]
    fn test_mistral_provider_new() {
        let provider = MistralProvider::new();
        assert_eq!(provider.provider_type(), ProviderType::Mistral);
        assert_eq!(provider.default_model(), DEFAULT_MISTRAL_MODEL);
    }

    #[test]
    fn test_mistral_provider_default() {
        let provider = MistralProvider::default();
        assert_eq!(provider.provider_type(), ProviderType::Mistral);
    }

    #[test]
    fn test_mistral_available_models() {
        let provider = MistralProvider::new();
        let models = provider.available_models();
        assert!(!models.is_empty());
        assert!(models.contains(&"mistral-large-latest".to_string()));
    }

    #[tokio::test]
    async fn test_mistral_provider_configure() {
        let provider = MistralProvider::new();

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
        let provider = MistralProvider::new();

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
        let provider = MistralProvider::new();

        let result = provider.complete("Hello", None, None, 0.7, 1000).await;

        assert!(result.is_err());
        match result {
            Err(LLMError::NotConfigured(_)) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }
}
