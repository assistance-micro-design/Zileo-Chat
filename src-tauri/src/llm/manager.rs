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

use super::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use super::mistral::MistralProvider;
use super::ollama::OllamaProvider;
use super::openai_compatible::OpenAiCompatibleProvider;
use super::provider::{
    CompletionParams, LLMError, LLMProvider, LLMResponse, ProviderType, ToolCompletionParams,
};
use super::retry::{with_retry, with_retry_cancellable, RetryConfig};
use crate::constants::llm_http::DEFAULT_READ_TIMEOUT_SECS;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Provider configuration state
#[derive(Debug, Clone)]
#[cfg_attr(not(test), allow(dead_code))]
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
            mistral_model: String::new(),
            ollama_model: String::new(),
            ollama_url: super::ollama::DEFAULT_OLLAMA_URL.to_string(),
        }
    }
}

/// Maximum idle connections per host for connection pooling.
const HTTP_POOL_MAX_IDLE_PER_HOST: usize = 5;

/// Manager for LLM providers
///
/// Provides a unified interface to manage and use multiple LLM providers.
/// Handles provider configuration, switching, and completion requests.
///
/// The manager maintains a shared HTTP client for all providers to benefit
/// from connection pooling and avoid repeated TLS handshakes.
///
/// Retry mechanism with exponential backoff handles transient
/// failures automatically.
///
/// Circuit breaker pattern protects against cascading failures
/// when providers are unavailable.
pub struct ProviderManager {
    /// Mistral provider instance
    mistral: Arc<MistralProvider>,
    /// Ollama provider instance
    ollama: Arc<OllamaProvider>,
    /// Custom OpenAI-compatible providers (keyed by provider name)
    custom_providers: Arc<RwLock<HashMap<String, Arc<OpenAiCompatibleProvider>>>>,
    /// Configuration state
    config: Arc<RwLock<ProviderConfig>>,
    /// Shared HTTP client for all providers (connection pooling)
    http_client: Arc<reqwest::Client>,
    /// Retry configuration for API calls
    retry_config: RetryConfig,
    /// Circuit breakers for each provider
    circuit_breakers: Arc<RwLock<HashMap<ProviderType, CircuitBreaker>>>,
}

impl ProviderManager {
    /// Creates a new provider manager with default configuration.
    ///
    /// Initializes a shared HTTP client with connection pooling for all providers.
    /// This improves performance by reusing connections and avoiding TLS handshake
    /// overhead on subsequent requests.
    ///
    /// Also initializes retry configuration with exponential backoff
    /// and circuit breakers for each provider.
    ///
    /// # Errors
    /// Returns an error if the HTTP client fails to initialize.
    pub fn new() -> Result<Self, String> {
        // Create shared HTTP client with connection pooling.
        //
        // `read_timeout` (per-read, resets on each successful read) replaces
        // the old total `.timeout()` so wire-level streaming
        // (`complete_with_tools` over SSE) can sit idle through long
        // thinking phases without tripping reqwest. Cloudflare's
        // ~100s origin-idle limit is defeated separately because the
        // server keeps emitting SSE chunks during thinking. Uses the same
        // shared `DEFAULT_READ_TIMEOUT_SECS` as `mistral.rs` /
        // `openai_compatible.rs` test clients to keep stall behavior
        // uniform across provider HTTP pools.
        let http_client = Arc::new(
            reqwest::Client::builder()
                .read_timeout(Duration::from_secs(DEFAULT_READ_TIMEOUT_SECS))
                .pool_max_idle_per_host(HTTP_POOL_MAX_IDLE_PER_HOST)
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?,
        );

        // Initialize circuit breakers for each provider
        let mut circuit_breakers = HashMap::new();
        circuit_breakers.insert(
            ProviderType::Mistral,
            CircuitBreaker::new(
                CircuitBreakerConfig::for_llm_provider(),
                "Mistral".to_string(),
            ),
        );
        circuit_breakers.insert(
            ProviderType::Ollama,
            CircuitBreaker::new(
                CircuitBreakerConfig::for_llm_provider(),
                "Ollama".to_string(),
            ),
        );

        Ok(Self {
            mistral: Arc::new(MistralProvider::new(http_client.clone())),
            ollama: Arc::new(OllamaProvider::new(http_client.clone())),
            custom_providers: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(ProviderConfig::default())),
            http_client,
            retry_config: RetryConfig::default(),
            circuit_breakers: Arc::new(RwLock::new(circuit_breakers)),
        })
    }

    /// Returns a reference to the shared HTTP client.
    ///
    /// This can be used by external code that needs to make HTTP requests
    /// while benefiting from the manager's connection pool.
    pub fn http_client(&self) -> &Arc<reqwest::Client> {
        &self.http_client
    }

    /// Checks if the circuit breaker allows requests to the given provider.
    ///
    /// Returns Ok(()) if the circuit is available, or CircuitOpen error if not.
    async fn check_circuit_breaker(&self, provider: ProviderType) -> Result<(), LLMError> {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(&provider) {
            if breaker.is_available().await {
                Ok(())
            } else {
                warn!(
                    provider = %provider,
                    "Circuit breaker is open, rejecting request"
                );
                Err(LLMError::CircuitOpen(provider.to_string()))
            }
        } else {
            // No circuit breaker for this provider, allow
            Ok(())
        }
    }

    /// Records a successful request for the circuit breaker.
    async fn record_circuit_success(&self, provider: ProviderType) {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(&provider) {
            breaker.record_success().await;
        }
    }

    /// Records a failed request for the circuit breaker.
    async fn record_circuit_failure(&self, provider: ProviderType) {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(&provider) {
            breaker.record_failure().await;
        }
    }

    /// Registers a custom OpenAI-compatible provider.
    pub async fn register_custom_provider(
        &self,
        name: &str,
        provider: Arc<OpenAiCompatibleProvider>,
    ) {
        self.custom_providers
            .write()
            .await
            .insert(name.to_string(), provider);
        info!(provider = name, "Custom provider registered in manager");
    }

    /// Unregisters a custom provider.
    pub async fn unregister_custom_provider(&self, name: &str) {
        self.custom_providers.write().await.remove(name);
        info!(provider = name, "Custom provider unregistered from manager");
    }

    /// Gets a custom provider by name.
    pub async fn get_custom_provider(&self, name: &str) -> Option<Arc<OpenAiCompatibleProvider>> {
        self.custom_providers.read().await.get(name).cloned()
    }

    /// Gets the current configuration
    #[cfg(test)]
    pub async fn get_config(&self) -> ProviderConfig {
        self.config.read().await.clone()
    }

    /// Sets the active provider
    #[cfg(test)]
    pub async fn set_active_provider(&self, provider: ProviderType) -> Result<(), LLMError> {
        // Verify the provider is configured
        let is_configured = match &provider {
            ProviderType::Mistral => self.mistral.is_configured(),
            ProviderType::Ollama => self.ollama.is_configured(),
            ProviderType::Custom(ref name) => self
                .custom_providers
                .read()
                .await
                .get(name)
                .map(|p| p.is_configured())
                .unwrap_or(false),
        };

        if !is_configured {
            return Err(LLMError::NotConfigured(provider.to_string()));
        }

        self.config.write().await.active_provider = provider.clone();
        info!(?provider, "Active provider changed");
        Ok(())
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
    #[cfg(test)]
    pub async fn set_default_model(&self, provider: ProviderType, model: &str) {
        let mut config = self.config.write().await;
        match provider {
            ProviderType::Mistral => config.mistral_model = model.to_string(),
            ProviderType::Ollama => config.ollama_model = model.to_string(),
            ProviderType::Custom(_) => {
                // Custom providers don't have a config-level default model;
                // their default model is managed via provider_settings in the DB
            }
        }
        debug!(?provider, model, "Default model updated");
    }

    /// Gets available models for a provider
    #[cfg(test)]
    pub fn get_available_models(&self, provider: ProviderType) -> Vec<String> {
        match provider {
            ProviderType::Mistral => self.mistral.available_models(),
            ProviderType::Ollama => self.ollama.available_models(),
            ProviderType::Custom(_) => Vec::new(), // Custom providers list models from DB
        }
    }

    /// Checks if a provider is configured
    pub fn is_provider_configured(&self, provider: ProviderType) -> bool {
        match provider {
            ProviderType::Mistral => self.mistral.is_configured(),
            ProviderType::Ollama => self.ollama.is_configured(),
            ProviderType::Custom(ref name) => self
                .custom_providers
                .try_read()
                .map(|guard| guard.get(name).map(|p| p.is_configured()).unwrap_or(false))
                .unwrap_or(false),
        }
    }

    /// Completes a prompt using a specific provider with automatic retry.
    ///
    /// This method wraps the provider completion with retry logic
    /// and circuit breaker protection.
    pub async fn complete_with_provider(
        &self,
        provider: ProviderType,
        params: CompletionParams,
    ) -> Result<LLMResponse, LLMError> {
        // Check circuit breaker before making request
        self.check_circuit_breaker(provider.clone()).await?;

        let result = match &provider {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry(
                    || {
                        let p = params.clone();
                        let prov = mistral.clone();
                        async move { prov.complete(p).await }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                with_retry(
                    || {
                        let p = params.clone();
                        let prov = ollama.clone();
                        async move { prov.complete(p).await }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Custom(ref name) => {
                let custom = self
                    .custom_providers
                    .read()
                    .await
                    .get(name)
                    .cloned()
                    .ok_or_else(|| LLMError::NotConfigured(name.clone()))?;
                with_retry(
                    || {
                        let p = params.clone();
                        let prov = custom.clone();
                        async move { prov.complete(p).await }
                    },
                    &self.retry_config,
                )
                .await
            }
        };

        // Record result for circuit breaker
        match &result {
            Ok(_) => self.record_circuit_success(provider).await,
            Err(_) => self.record_circuit_failure(provider).await,
        }

        result
    }

    /// Completes with tools using a specific provider with automatic retry.
    ///
    /// This method is used for JSON function calling with tool definitions.
    /// Includes retry logic with exponential backoff and circuit
    /// breaker protection.
    ///
    /// `ToolCompletionParams` includes `reasoning_effort` for providers that support
    /// simultaneous tool calling and extended thinking (e.g. Mistral, OpenAI-compatible).
    ///
    /// # Arguments
    /// * `provider` - Which provider to use
    /// * `params` - Tool completion parameters (messages, tools, model, etc.)
    ///
    /// # Returns
    /// Raw JSON response from the API (caller should use adapter to parse)
    pub async fn complete_with_tools(
        &self,
        provider: ProviderType,
        params: ToolCompletionParams,
    ) -> Result<serde_json::Value, LLMError> {
        // Check circuit breaker before making request
        self.check_circuit_breaker(provider.clone()).await?;

        debug!(
            ?provider,
            model = %params.model,
            tools_count = params.tools.len(),
            "Executing completion with tools via manager"
        );

        let result = match &provider {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry(
                    || {
                        let p = params.clone();
                        let prov = mistral.clone();
                        async move { prov.complete_with_tools(&p).await }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                with_retry(
                    || {
                        let p = params.clone();
                        let prov = ollama.clone();
                        async move { prov.complete_with_tools(&p).await }
                    },
                    &self.retry_config,
                )
                .await
            }
            ProviderType::Custom(ref name) => {
                let custom = self
                    .custom_providers
                    .read()
                    .await
                    .get(name)
                    .cloned()
                    .ok_or_else(|| LLMError::NotConfigured(name.clone()))?;
                with_retry(
                    || {
                        let p = params.clone();
                        let prov = custom.clone();
                        async move { prov.complete_with_tools(&p).await }
                    },
                    &self.retry_config,
                )
                .await
            }
        };

        // Record result for circuit breaker
        match &result {
            Ok(_) => self.record_circuit_success(provider).await,
            Err(_) => self.record_circuit_failure(provider).await,
        }

        result
    }

    /// Like [`Self::complete_with_provider`] but races each attempt and each
    /// retry-sleep against `cancellation_token`.
    ///
    /// Used by the simple (no-tools) execution path to honor a workflow
    /// cancellation request even when no tool calls are in flight. Dropping
    /// the in-flight request future drops the underlying `reqwest` connection,
    /// so the HTTP request itself is cancelled.
    ///
    /// `cancellation_token = None` is equivalent to calling `complete_with_provider`.
    pub async fn complete_with_provider_cancellable(
        &self,
        provider: ProviderType,
        params: CompletionParams,
        cancellation_token: Option<tokio_util::sync::CancellationToken>,
    ) -> Result<LLMResponse, LLMError> {
        if cancellation_token.is_none() {
            return self.complete_with_provider(provider, params).await;
        }

        let token = cancellation_token.as_ref();

        self.check_circuit_breaker(provider.clone()).await?;

        let result = match &provider {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry_cancellable(
                    || {
                        let p = params.clone();
                        let prov = mistral.clone();
                        async move { prov.complete(p).await }
                    },
                    &self.retry_config,
                    token,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                with_retry_cancellable(
                    || {
                        let p = params.clone();
                        let prov = ollama.clone();
                        async move { prov.complete(p).await }
                    },
                    &self.retry_config,
                    token,
                )
                .await
            }
            ProviderType::Custom(ref name) => {
                let custom = self
                    .custom_providers
                    .read()
                    .await
                    .get(name)
                    .cloned()
                    .ok_or_else(|| LLMError::NotConfigured(name.clone()))?;
                with_retry_cancellable(
                    || {
                        let p = params.clone();
                        let prov = custom.clone();
                        async move { prov.complete(p).await }
                    },
                    &self.retry_config,
                    token,
                )
                .await
            }
        };

        // Cancellation is intentional, not a provider failure: don't trip the
        // circuit breaker. Treat any other Err as a real failure.
        match &result {
            Ok(_) => self.record_circuit_success(provider).await,
            Err(LLMError::Cancelled) => debug!("Skipping circuit breaker for cancellation"),
            Err(_) => self.record_circuit_failure(provider).await,
        }

        result
    }

    /// Like [`Self::complete_with_tools`] but races each provider call (and
    /// each retry-sleep) against `cancellation_token`.
    ///
    /// When the token fires, returns [`LLMError::Cancelled`] within
    /// roughly the next polling cycle (sub-second in practice). Dropping the
    /// in-flight request future also drops the underlying `reqwest` connection,
    /// so the HTTP request itself is cancelled — no polling of a stuck call.
    ///
    /// `cancellation_token = None` is equivalent to calling `complete_with_tools`.
    pub async fn complete_with_tools_cancellable(
        &self,
        provider: ProviderType,
        params: ToolCompletionParams,
        cancellation_token: Option<tokio_util::sync::CancellationToken>,
    ) -> Result<serde_json::Value, LLMError> {
        // Fast path: no token -> reuse the original method (preserves circuit
        // breaker semantics + exact retry behavior, byte-for-byte).
        if cancellation_token.is_none() {
            return self.complete_with_tools(provider, params).await;
        }

        let token = cancellation_token.as_ref();

        self.check_circuit_breaker(provider.clone()).await?;
        debug!(
            ?provider,
            model = %params.model,
            tools_count = params.tools.len(),
            "Executing cancellable completion with tools via manager"
        );

        let result = match &provider {
            ProviderType::Mistral => {
                let mistral = self.mistral.clone();
                with_retry_cancellable(
                    || {
                        let p = params.clone();
                        let prov = mistral.clone();
                        async move { prov.complete_with_tools(&p).await }
                    },
                    &self.retry_config,
                    token,
                )
                .await
            }
            ProviderType::Ollama => {
                let ollama = self.ollama.clone();
                with_retry_cancellable(
                    || {
                        let p = params.clone();
                        let prov = ollama.clone();
                        async move { prov.complete_with_tools(&p).await }
                    },
                    &self.retry_config,
                    token,
                )
                .await
            }
            ProviderType::Custom(ref name) => {
                let custom = self
                    .custom_providers
                    .read()
                    .await
                    .get(name)
                    .cloned()
                    .ok_or_else(|| LLMError::NotConfigured(name.clone()))?;
                with_retry_cancellable(
                    || {
                        let p = params.clone();
                        let prov = custom.clone();
                        async move { prov.complete_with_tools(&p).await }
                    },
                    &self.retry_config,
                    token,
                )
                .await
            }
        };

        // Cancellation is intentional, not a provider failure: don't trip the
        // circuit breaker. Treat any other Err as a real failure.
        match &result {
            Ok(_) => self.record_circuit_success(provider).await,
            Err(LLMError::Cancelled) => debug!("Skipping circuit breaker for cancellation"),
            Err(_) => self.record_circuit_failure(provider).await,
        }

        result
    }

    /// Gets reference to Ollama provider
    pub fn ollama(&self) -> &Arc<OllamaProvider> {
        &self.ollama
    }
}

#[cfg(test)]
impl ProviderManager {
    /// Gets the circuit breaker status for a provider (test-only).
    pub async fn get_circuit_breaker_status(
        &self,
        provider: ProviderType,
    ) -> Option<super::circuit_breaker::CircuitBreakerStats> {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(&provider) {
            Some(breaker.stats().await)
        } else {
            None
        }
    }

    /// Resets the circuit breaker for a provider (test-only).
    pub async fn reset_circuit_breaker(&self, provider: ProviderType) {
        let breakers = self.circuit_breakers.read().await;
        if let Some(breaker) = breakers.get(&provider) {
            breaker.reset().await;
            info!(provider = %provider, "Circuit breaker manually reset");
        }
    }

    /// Gets the active provider type (test-only).
    pub async fn get_active_provider(&self) -> ProviderType {
        self.config.read().await.active_provider.clone()
    }

    /// Gets the default model for a provider (test-only).
    pub async fn get_default_model(&self, provider: ProviderType) -> String {
        let config = self.config.read().await;
        match provider {
            ProviderType::Mistral => config.mistral_model.clone(),
            ProviderType::Ollama => config.ollama_model.clone(),
            ProviderType::Custom(_) => String::new(),
        }
    }

    /// Gets all configured providers (test-only).
    pub fn get_configured_providers(&self) -> Vec<ProviderType> {
        let mut providers = Vec::new();
        if self.mistral.is_configured() {
            providers.push(ProviderType::Mistral);
        }
        if self.ollama.is_configured() {
            providers.push(ProviderType::Ollama);
        }
        if let Ok(guard) = self.custom_providers.try_read() {
            for (name, p) in guard.iter() {
                if p.is_configured() {
                    providers.push(ProviderType::Custom(name.clone()));
                }
            }
        }
        providers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_provider_manager_new() {
        let manager = ProviderManager::new().expect("test provider manager");
        let config = manager.get_config().await;

        // Default to Ollama (local)
        assert_eq!(config.active_provider, ProviderType::Ollama);
    }

    #[tokio::test]
    async fn test_get_available_models_empty() {
        // Models are now managed in DB, not hardcoded in providers
        let manager = ProviderManager::new().expect("test provider manager");

        let mistral_models = manager.get_available_models(ProviderType::Mistral);
        assert!(mistral_models.is_empty());

        let ollama_models = manager.get_available_models(ProviderType::Ollama);
        assert!(ollama_models.is_empty());
    }

    #[tokio::test]
    async fn test_set_default_model() {
        let manager = ProviderManager::new().expect("test provider manager");

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
        let manager = ProviderManager::new().expect("test provider manager");

        // Initially not configured
        assert!(!manager.is_provider_configured(ProviderType::Mistral));
        assert!(!manager.is_provider_configured(ProviderType::Ollama));
    }

    #[tokio::test]
    async fn test_get_configured_providers() {
        let manager = ProviderManager::new().expect("test provider manager");

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
        let manager = ProviderManager::new().expect("test provider manager");

        let result = manager.configure_ollama(None).await;
        assert!(result.is_ok());
        assert!(manager.is_provider_configured(ProviderType::Ollama));
    }

    #[tokio::test]
    async fn test_configure_ollama_custom_url() {
        let manager = ProviderManager::new().expect("test provider manager");

        let custom_url = "http://192.168.1.100:11434";
        manager.configure_ollama(Some(custom_url)).await.unwrap();

        let config = manager.get_config().await;
        assert_eq!(config.ollama_url, custom_url);
    }

    #[tokio::test]
    async fn test_configure_mistral() {
        let manager = ProviderManager::new().expect("test provider manager");

        // Configure with fake API key (won't make real calls)
        let result = manager.configure_mistral("test-api-key").await;
        assert!(result.is_ok());
        assert!(manager.is_provider_configured(ProviderType::Mistral));
    }

    #[tokio::test]
    async fn test_set_active_provider_not_configured() {
        let manager = ProviderManager::new().expect("test provider manager");

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
        let manager = ProviderManager::new().expect("test provider manager");

        // Configure Mistral first
        manager.configure_mistral("test-key").await.unwrap();

        // Now should be able to set as active
        let result = manager.set_active_provider(ProviderType::Mistral).await;
        assert!(result.is_ok());
        assert_eq!(manager.get_active_provider().await, ProviderType::Mistral);
    }

    #[tokio::test]
    async fn test_complete_no_provider_configured() {
        let manager = ProviderManager::new().expect("test provider manager");

        let result = manager
            .complete_with_provider(
                ProviderType::Mistral,
                CompletionParams {
                    prompt: "Hello".to_string(),
                    system_prompt: None,
                    model: None,
                    temperature: 0.7,
                    max_tokens: 1000,
                    reasoning_effort: None,
                    context_window: None,
                },
            )
            .await;

        assert!(result.is_err());
    }

    // Circuit breaker tests

    #[tokio::test]
    async fn test_circuit_breaker_initial_status() {
        use super::super::circuit_breaker::CircuitState;

        let manager = ProviderManager::new().expect("test provider manager");

        // Both providers should start with closed circuit
        let mistral_status = manager
            .get_circuit_breaker_status(ProviderType::Mistral)
            .await;
        assert!(mistral_status.is_some());
        assert_eq!(mistral_status.unwrap().state, CircuitState::Closed);

        let ollama_status = manager
            .get_circuit_breaker_status(ProviderType::Ollama)
            .await;
        assert!(ollama_status.is_some());
        assert_eq!(ollama_status.unwrap().state, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_reset() {
        use super::super::circuit_breaker::CircuitState;

        let manager = ProviderManager::new().expect("test provider manager");

        // Manually record some failures to affect state
        manager.record_circuit_failure(ProviderType::Mistral).await;
        manager.record_circuit_failure(ProviderType::Mistral).await;

        let status = manager
            .get_circuit_breaker_status(ProviderType::Mistral)
            .await
            .unwrap();
        assert_eq!(status.consecutive_failures, 2);

        // Reset should clear failures
        manager.reset_circuit_breaker(ProviderType::Mistral).await;

        let status = manager
            .get_circuit_breaker_status(ProviderType::Mistral)
            .await
            .unwrap();
        assert_eq!(status.state, CircuitState::Closed);
        assert_eq!(status.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_check_allows_closed() {
        let manager = ProviderManager::new().expect("test provider manager");

        // With closed circuit, check should pass
        let result = manager.check_circuit_breaker(ProviderType::Mistral).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_circuit_breaker_records_success() {
        let manager = ProviderManager::new().expect("test provider manager");

        // Record a failure then a success
        manager.record_circuit_failure(ProviderType::Ollama).await;
        let status = manager
            .get_circuit_breaker_status(ProviderType::Ollama)
            .await
            .unwrap();
        assert_eq!(status.consecutive_failures, 1);

        manager.record_circuit_success(ProviderType::Ollama).await;
        let status = manager
            .get_circuit_breaker_status(ProviderType::Ollama)
            .await
            .unwrap();
        assert_eq!(status.consecutive_failures, 0);
    }
}
