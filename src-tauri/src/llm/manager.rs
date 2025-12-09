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

//! LLM Provider Manager - orchestrates multiple providers

use super::mistral::MistralProvider;
use super::ollama::OllamaProvider;
use super::provider::{LLMError, LLMProvider, LLMResponse, ProviderType};
use super::retry::{with_retry, RetryConfig};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// Provider configuration state
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    /// Currently active provider
    pub active_provider: ProviderType,
    /// Default model for Mistral
    pub mistral_model: String,
    /// Default model for Ollama
    pub ollama_model: String,
    /// Ollama server URL
    pub ollama_url: String,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            active_provider: ProviderType::Ollama, // Default to local
            mistral_model: super::mistral::DEFAULT_MISTRAL_MODEL.to_string(),
            ollama_model: super::ollama::DEFAULT_OLLAMA_MODEL.to_string(),
            ollama_url: super::ollama::DEFAULT_OLLAMA_URL.to_string(),
        }
    }
}

/// Default HTTP timeout for LLM requests (5 minutes for long completions)
const HTTP_TIMEOUT_SECS: u64 = 300;

/// Maximum idle connections per host for connection pooling
const HTTP_POOL_MAX_IDLE_PER_HOST: usize = 5;

/// Manager for LLM providers
///
/// Provides a unified interface to manage and use multiple LLM providers.
/// Handles provider configuration, switching, and completion requests.
///
/// The manager maintains a shared HTTP client for all providers to benefit
/// from connection pooling and avoid repeated TLS handshakes (OPT-LLM-2).
///
/// Retry mechanism with exponential backoff (OPT-LLM-4) handles transient
/// failures automatically.
#[allow(dead_code)]
pub struct ProviderManager {
    /// Mistral provider instance
    mistral: Arc<MistralProvider>,
    /// Ollama provider instance
    ollama: Arc<OllamaProvider>,
    /// Configuration state
    config: Arc<RwLock<ProviderConfig>>,
    /// Shared HTTP client for all providers (connection pooling)
    http_client: Arc<reqwest::Client>,
    /// Retry configuration for API calls (OPT-LLM-4)
    retry_config: RetryConfig,
}

#[allow(dead_code)]
impl ProviderManager {
    /// Creates a new provider manager with default configuration.
    ///
    /// Initializes a shared HTTP client with connection pooling for all providers.
    /// This improves performance by reusing connections and avoiding TLS handshake
    /// overhead on subsequent requests (OPT-LLM-2).
    ///
    /// Also initializes retry configuration with exponential backoff (OPT-LLM-4).
    pub fn new() -> Self {
        // Create shared HTTP client with connection pooling
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
                .pool_max_idle_per_host(HTTP_POOL_MAX_IDLE_PER_HOST)
                .build()
                .expect("Failed to create HTTP client"),
        );

        Self {
            mistral: Arc::new(MistralProvider::new(http_client.clone())),
            ollama: Arc::new(OllamaProvider::new(http_client.clone())),
            config: Arc::new(RwLock::new(ProviderConfig::default())),
            http_client,
            retry_config: RetryConfig::default(),
        }
    }

    /// Creates a new provider manager with custom retry configuration.
    pub fn with_retry_config(retry_config: RetryConfig) -> Self {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
                .pool_max_idle_per_host(HTTP_POOL_MAX_IDLE_PER_HOST)
                .build()
                .expect("Failed to create HTTP client"),
        );

        Self {
            mistral: Arc::new(MistralProvider::new(http_client.clone())),
            ollama: Arc::new(OllamaProvider::new(http_client.clone())),
            config: Arc::new(RwLock::new(ProviderConfig::default())),
            http_client,
            retry_config,
        }
    }

    /// Returns a reference to the shared HTTP client.
    ///
    /// This can be used by external code that needs to make HTTP requests
    /// while benefiting from the manager's connection pool.
    pub fn http_client(&self) -> &Arc<reqwest::Client> {
        &self.http_client
    }

    /// Gets the current configuration
    pub async fn get_config(&self) -> ProviderConfig {
        self.config.read().await.clone()
    }

    /// Sets the active provider
    pub async fn set_active_provider(&self, provider: ProviderType) -> Result<(), LLMError> {
        // Verify the provider is configured
        let is_configured = match provider {
            ProviderType::Mistral => self.mistral.is_configured(),
            ProviderType::Ollama => self.ollama.is_configured(),
        };

        if !is_configured {
            return Err(LLMError::NotConfigured(provider.to_string()));
        }

        self.config.write().await.active_provider = provider;
        info!(?provider, "Active provider changed");
        Ok(())
    }

    /// Gets the active provider type
    pub async fn get_active_provider(&self) -> ProviderType {
        self.config.read().await.active_provider
    }

    /// Configures the Mistral provider with an API key
    pub async fn configure_mistral(&self, api_key: &str) -> Result<(), LLMError> {
        self.mistral.configure(api_key).await?;
        info!("Mistral provider configured via manager");
        Ok(())
    }

    /// Configures the Ollama provider
    pub async fn configure_ollama(&self, url: Option<&str>) -> Result<(), LLMError> {
        let url_to_use = match url {
            Some(u) => u.to_string(),
            None => self.config.read().await.ollama_url.clone(),
        };

        self.ollama.configure(Some(&url_to_use)).await?;

        if let Some(u) = url {
            self.config.write().await.ollama_url = u.to_string();
        }

        info!("Ollama provider configured via manager");
        Ok(())
    }

    /// Sets the default model for a provider
    pub async fn set_default_model(&self, provider: ProviderType, model: &str) {
        let mut config = self.config.write().await;
        match provider {
            ProviderType::Mistral => config.mistral_model = model.to_string(),
            ProviderType::Ollama => config.ollama_model = model.to_string(),
        }
        debug!(?provider, model, "Default model updated");
    }

    /// Gets the default model for a provider
    pub async fn get_default_model(&self, provider: ProviderType) -> String {
        let config = self.config.read().await;
        match provider {
            ProviderType::Mistral => config.mistral_model.clone(),
            ProviderType::Ollama => config.ollama_model.clone(),
        }
    }

    /// Gets available models for a provider
    pub fn get_available_models(&self, provider: ProviderType) -> Vec<String> {
        match provider {
            ProviderType::Mistral => self.mistral.available_models(),
            ProviderType::Ollama => self.ollama.available_models(),
        }
    }

    /// Checks if a provider is configured
    pub fn is_provider_configured(&self, provider: ProviderType) -> bool {
        match provider {
            ProviderType::Mistral => self.mistral.is_configured(),
            ProviderType::Ollama => self.ollama.is_configured(),
        }
    }

    /// Gets all configured providers
    pub fn get_configured_providers(&self) -> Vec<ProviderType> {
        let mut providers = Vec::new();
        if self.mistral.is_configured() {
            providers.push(ProviderType::Mistral);
        }
        if self.ollama.is_configured() {
            providers.push(ProviderType::Ollama);
        }
        providers
    }

    /// Completes a prompt using the active provider with automatic retry.
    ///
    /// This method wraps the provider completion with retry logic (OPT-LLM-4).
    /// Transient errors (network issues, rate limits) are retried with exponential
    /// backoff, while non-recoverable errors (auth failures, bad requests) fail immediately.
    #[instrument(
        name = "manager_complete",
        skip(self, prompt, system_prompt),
        fields(prompt_len = prompt.len())
    )]
    pub async fn complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<LLMResponse, LLMError> {
        let (provider_type, model_to_use) = {
            let config = self.config.read().await;
            let provider = config.active_provider;
            let model_str = model
                .map(|m| m.to_string())
                .unwrap_or_else(|| match provider {
                    ProviderType::Mistral => config.mistral_model.clone(),
                    ProviderType::Ollama => config.ollama_model.clone(),
                });
            (provider, model_str)
        };

        debug!(
            ?provider_type,
            model = %model_to_use,
            "Executing completion via manager"
        );

        // Clone values for the retry closure
        let prompt_owned = prompt.to_string();
        let system_prompt_owned = system_prompt.map(|s| s.to_string());
        let model_owned = model_to_use.clone();

        // Execute with retry (OPT-LLM-4)
        match provider_type {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry(
                    || {
                        let p = prompt_owned.clone();
                        let sp = system_prompt_owned.clone();
                        let m = model_owned.clone();
                        let provider = mistral.clone();
                        async move {
                            provider
                                .complete(&p, sp.as_deref(), Some(&m), temperature, max_tokens)
                                .await
                        }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                with_retry(
                    || {
                        let p = prompt_owned.clone();
                        let sp = system_prompt_owned.clone();
                        let m = model_owned.clone();
                        let provider = ollama.clone();
                        async move {
                            provider
                                .complete(&p, sp.as_deref(), Some(&m), temperature, max_tokens)
                                .await
                        }
                    },
                    &self.retry_config,
                )
                .await
            }
        }
    }

    /// Completes a prompt using a specific provider with automatic retry.
    ///
    /// This method wraps the provider completion with retry logic (OPT-LLM-4).
    pub async fn complete_with_provider(
        &self,
        provider: ProviderType,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<LLMResponse, LLMError> {
        // Clone values for the retry closure
        let prompt_owned = prompt.to_string();
        let system_prompt_owned = system_prompt.map(|s| s.to_string());
        let model_owned = model.map(|s| s.to_string());

        match provider {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry(
                    || {
                        let p = prompt_owned.clone();
                        let sp = system_prompt_owned.clone();
                        let m = model_owned.clone();
                        let provider = mistral.clone();
                        async move {
                            provider
                                .complete(&p, sp.as_deref(), m.as_deref(), temperature, max_tokens)
                                .await
                        }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                with_retry(
                    || {
                        let p = prompt_owned.clone();
                        let sp = system_prompt_owned.clone();
                        let m = model_owned.clone();
                        let provider = ollama.clone();
                        async move {
                            provider
                                .complete(&p, sp.as_deref(), m.as_deref(), temperature, max_tokens)
                                .await
                        }
                    },
                    &self.retry_config,
                )
                .await
            }
        }
    }

    /// Completes with tools using a specific provider with automatic retry.
    ///
    /// This method is used for JSON function calling with tool definitions.
    /// Includes retry logic with exponential backoff (OPT-LLM-4).
    ///
    /// # Arguments
    /// * `provider` - Which provider to use
    /// * `messages` - Conversation history as JSON messages
    /// * `tools` - Tool definitions in OpenAI format
    /// * `tool_choice` - How the model should use tools (provider-specific)
    /// * `model` - Model to use
    /// * `temperature` - Sampling temperature
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    /// Raw JSON response from the API (caller should use adapter to parse)
    #[allow(clippy::too_many_arguments)]
    #[instrument(
        name = "manager_complete_with_tools",
        skip(self, messages, tools, tool_choice),
        fields(provider = ?provider, tools_count = tools.len())
    )]
    pub async fn complete_with_tools(
        &self,
        provider: ProviderType,
        messages: Vec<serde_json::Value>,
        tools: Vec<serde_json::Value>,
        tool_choice: Option<serde_json::Value>,
        model: &str,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<serde_json::Value, LLMError> {
        debug!(
            ?provider,
            model = model,
            tools_count = tools.len(),
            "Executing completion with tools via manager"
        );

        // Clone values for the retry closure
        let model_owned = model.to_string();

        match provider {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry(
                    || {
                        let msgs = messages.clone();
                        let tls = tools.clone();
                        let tc = tool_choice.clone();
                        let m = model_owned.clone();
                        let p = mistral.clone();
                        async move {
                            p.complete_with_tools(msgs, tls, tc, &m, temperature, max_tokens)
                                .await
                        }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                // Ollama doesn't use tool_choice, so we ignore it
                with_retry(
                    || {
                        let msgs = messages.clone();
                        let tls = tools.clone();
                        let m = model_owned.clone();
                        let p = ollama.clone();
                        async move {
                            p.complete_with_tools(msgs, tls, &m, temperature, max_tokens)
                                .await
                        }
                    },
                    &self.retry_config,
                )
                .await
            }
        }
    }

    /// Streaming completion using the active provider
    pub async fn complete_stream(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LLMError>>, LLMError> {
        let (provider_type, model_to_use) = {
            let config = self.config.read().await;
            let provider = config.active_provider;
            let model_str = model
                .map(|m| m.to_string())
                .unwrap_or_else(|| match provider {
                    ProviderType::Mistral => config.mistral_model.clone(),
                    ProviderType::Ollama => config.ollama_model.clone(),
                });
            (provider, model_str)
        };

        match provider_type {
            ProviderType::Mistral => {
                self.mistral
                    .complete_stream(
                        prompt,
                        system_prompt,
                        Some(&model_to_use),
                        temperature,
                        max_tokens,
                    )
                    .await
            }
            ProviderType::Ollama => {
                self.ollama
                    .complete_stream(
                        prompt,
                        system_prompt,
                        Some(&model_to_use),
                        temperature,
                        max_tokens,
                    )
                    .await
            }
        }
    }

    /// Gets reference to Mistral provider
    pub fn mistral(&self) -> &Arc<MistralProvider> {
        &self.mistral
    }

    /// Gets reference to Ollama provider
    pub fn ollama(&self) -> &Arc<OllamaProvider> {
        &self.ollama
    }
}

impl Default for ProviderManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_manager_new() {
        let manager = ProviderManager::new();
        let config = manager.get_config().await;

        // Default to Ollama (local)
        assert_eq!(config.active_provider, ProviderType::Ollama);
    }

    #[tokio::test]
    async fn test_provider_manager_default() {
        let manager = ProviderManager::default();
        assert_eq!(manager.get_active_provider().await, ProviderType::Ollama);
    }

    #[tokio::test]
    async fn test_get_available_models() {
        let manager = ProviderManager::new();

        let mistral_models = manager.get_available_models(ProviderType::Mistral);
        assert!(!mistral_models.is_empty());
        assert!(mistral_models.contains(&"mistral-large-latest".to_string()));

        let ollama_models = manager.get_available_models(ProviderType::Ollama);
        assert!(!ollama_models.is_empty());
        assert!(ollama_models.contains(&"llama3.2".to_string()));
    }

    #[tokio::test]
    async fn test_set_default_model() {
        let manager = ProviderManager::new();

        manager
            .set_default_model(ProviderType::Mistral, "mistral-small-latest")
            .await;
        assert_eq!(
            manager.get_default_model(ProviderType::Mistral).await,
            "mistral-small-latest"
        );

        manager
            .set_default_model(ProviderType::Ollama, "llama3")
            .await;
        assert_eq!(
            manager.get_default_model(ProviderType::Ollama).await,
            "llama3"
        );
    }

    #[tokio::test]
    async fn test_is_provider_configured() {
        let manager = ProviderManager::new();

        // Initially not configured
        assert!(!manager.is_provider_configured(ProviderType::Mistral));
        assert!(!manager.is_provider_configured(ProviderType::Ollama));
    }

    #[tokio::test]
    async fn test_get_configured_providers() {
        let manager = ProviderManager::new();

        // Initially none configured
        let providers = manager.get_configured_providers();
        assert!(providers.is_empty());

        // Configure Ollama
        manager.configure_ollama(None).await.unwrap();

        let providers = manager.get_configured_providers();
        assert_eq!(providers.len(), 1);
        assert!(providers.contains(&ProviderType::Ollama));
    }

    #[tokio::test]
    async fn test_configure_ollama() {
        let manager = ProviderManager::new();

        let result = manager.configure_ollama(None).await;
        assert!(result.is_ok());
        assert!(manager.is_provider_configured(ProviderType::Ollama));
    }

    #[tokio::test]
    async fn test_configure_ollama_custom_url() {
        let manager = ProviderManager::new();

        let custom_url = "http://192.168.1.100:11434";
        manager.configure_ollama(Some(custom_url)).await.unwrap();

        let config = manager.get_config().await;
        assert_eq!(config.ollama_url, custom_url);
    }

    #[tokio::test]
    async fn test_configure_mistral() {
        let manager = ProviderManager::new();

        // Configure with fake API key (won't make real calls)
        let result = manager.configure_mistral("test-api-key").await;
        assert!(result.is_ok());
        assert!(manager.is_provider_configured(ProviderType::Mistral));
    }

    #[tokio::test]
    async fn test_set_active_provider_not_configured() {
        let manager = ProviderManager::new();

        // Try to set Mistral as active without configuring
        let result = manager.set_active_provider(ProviderType::Mistral).await;
        assert!(result.is_err());

        match result {
            Err(LLMError::NotConfigured(_)) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[tokio::test]
    async fn test_set_active_provider_configured() {
        let manager = ProviderManager::new();

        // Configure Mistral first
        manager.configure_mistral("test-key").await.unwrap();

        // Now should be able to set as active
        let result = manager.set_active_provider(ProviderType::Mistral).await;
        assert!(result.is_ok());
        assert_eq!(manager.get_active_provider().await, ProviderType::Mistral);
    }

    #[tokio::test]
    async fn test_complete_no_provider_configured() {
        let manager = ProviderManager::new();

        let result = manager.complete("Hello", None, None, 0.7, 1000).await;

        assert!(result.is_err());
    }
}
