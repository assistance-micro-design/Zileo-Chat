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

//! # Embedding Service Module
//!
//! This module is prepared for Phase 3 (MemoryTool implementation).
//! The `#[allow(dead_code)]` attribute is used until the module is integrated.
//!
//! This module provides vector embedding generation for semantic search and RAG operations.
//! It supports multiple providers (Mistral, Ollama) with a unified interface.
//!
//! ## Architecture
//!
//! - [`EmbeddingService`] - Main service for embedding generation
//! - [`EmbeddingProvider`] - Enum defining supported embedding providers
//! - [`EmbeddingConfig`] - Configuration for embedding models
//! - [`EmbeddingError`] - Error types for embedding operations
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::llm::embedding::{EmbeddingService, EmbeddingProvider};
//!
//! let provider = EmbeddingProvider::Mistral {
//!     api_key: "your-api-key".to_string(),
//!     model: "mistral-embed".to_string(),
//! };
//! let service = EmbeddingService::new(provider).await?;
//! let embedding = service.embed("Hello, world!").await?;
//! ```

// Allow dead code until MemoryTool integration in Phase 3
#![allow(dead_code)]

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

// ============================================================================
// Constants
// ============================================================================

/// Mistral embedding API endpoint
const MISTRAL_EMBEDDING_URL: &str = "https://api.mistral.ai/v1/embeddings";

/// Default Mistral embedding model
pub const MISTRAL_EMBED_MODEL: &str = "mistral-embed";

/// Mistral embed model dimension (1024D)
pub const MISTRAL_EMBED_DIMENSION: usize = 1024;

/// Default Ollama embedding endpoint
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Ollama nomic-embed-text dimension (768D)
pub const OLLAMA_NOMIC_DIMENSION: usize = 768;

/// Ollama mxbai-embed-large dimension (1024D)
pub const OLLAMA_MXBAI_DIMENSION: usize = 1024;

/// Default embedding model for Ollama
pub const DEFAULT_OLLAMA_EMBED_MODEL: &str = "nomic-embed-text";

/// Maximum text length for embedding (characters)
pub const MAX_EMBEDDING_TEXT_LENGTH: usize = 50_000;

/// Maximum batch size for embedding requests
pub const MAX_BATCH_SIZE: usize = 96;

/// Default timeout for embedding requests (milliseconds)
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

// ============================================================================
// Error Types
// ============================================================================

/// Errors that can occur during embedding operations
#[derive(Debug, Error)]
pub enum EmbeddingError {
    /// API request failed
    #[error("API request failed: {0}")]
    RequestFailed(String),

    /// Invalid response format from embedding API
    #[error("Invalid response format: {0}")]
    InvalidResponse(String),

    /// Provider not configured or missing credentials
    #[error("Provider not configured: {0}")]
    NotConfigured(String),

    /// Text too long for embedding
    #[error("Text too long: {0} chars, max {1}")]
    TextTooLong(usize, usize),

    /// Batch size exceeded
    #[error("Batch size exceeded: {0} items, max {1}")]
    BatchTooLarge(usize, usize),

    /// Model not available for embedding
    #[error("Embedding model not available: {0}")]
    ModelNotAvailable(String),

    /// Connection error (e.g., Ollama server not running)
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Timeout error
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Dimension mismatch (expected vs actual)
    #[error("Dimension mismatch: expected {0}, got {1}")]
    DimensionMismatch(usize, usize),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<reqwest::Error> for EmbeddingError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            EmbeddingError::Timeout(DEFAULT_TIMEOUT_MS)
        } else if err.is_connect() {
            EmbeddingError::ConnectionError(err.to_string())
        } else {
            EmbeddingError::RequestFailed(err.to_string())
        }
    }
}

// ============================================================================
// Provider Configuration
// ============================================================================

/// Embedding provider configuration
#[derive(Debug, Clone)]
pub enum EmbeddingProvider {
    /// Mistral AI embedding API
    Mistral {
        /// API key for authentication
        api_key: String,
        /// Embedding model (default: mistral-embed)
        model: String,
    },
    /// Ollama local embedding server
    Ollama {
        /// Server base URL (default: http://localhost:11434)
        base_url: String,
        /// Embedding model (e.g., nomic-embed-text, mxbai-embed-large)
        model: String,
    },
}

impl EmbeddingProvider {
    /// Creates a Mistral provider with default model
    pub fn mistral(api_key: &str) -> Self {
        EmbeddingProvider::Mistral {
            api_key: api_key.to_string(),
            model: MISTRAL_EMBED_MODEL.to_string(),
        }
    }

    /// Creates a Mistral provider with custom model
    pub fn mistral_with_model(api_key: &str, model: &str) -> Self {
        EmbeddingProvider::Mistral {
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }

    /// Creates an Ollama provider with default URL and model
    pub fn ollama() -> Self {
        EmbeddingProvider::Ollama {
            base_url: DEFAULT_OLLAMA_URL.to_string(),
            model: DEFAULT_OLLAMA_EMBED_MODEL.to_string(),
        }
    }

    /// Creates an Ollama provider with custom URL and model
    pub fn ollama_with_config(base_url: &str, model: &str) -> Self {
        EmbeddingProvider::Ollama {
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    /// Returns the expected dimension for the configured model
    pub fn dimension(&self) -> usize {
        match self {
            EmbeddingProvider::Mistral { .. } => {
                // All Mistral embedding models use 1024 dimensions
                MISTRAL_EMBED_DIMENSION
            }
            EmbeddingProvider::Ollama { model, .. } => {
                // mxbai-embed-large uses 1024D, all others (including nomic) use 768D
                if model.contains("mxbai") {
                    OLLAMA_MXBAI_DIMENSION
                } else {
                    OLLAMA_NOMIC_DIMENSION
                }
            }
        }
    }

    /// Returns the provider name as string
    pub fn name(&self) -> &'static str {
        match self {
            EmbeddingProvider::Mistral { .. } => "mistral",
            EmbeddingProvider::Ollama { .. } => "ollama",
        }
    }

    /// Returns the model name
    pub fn model(&self) -> &str {
        match self {
            EmbeddingProvider::Mistral { model, .. } => model,
            EmbeddingProvider::Ollama { model, .. } => model,
        }
    }
}

// ============================================================================
// Configuration Types
// ============================================================================

/// Embedding configuration for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Provider name: "mistral" or "ollama"
    pub provider: String,
    /// Model name for embeddings
    pub model: String,
    /// Vector dimension (auto-determined from model)
    pub dimension: usize,
    /// Maximum tokens per input (provider-specific)
    pub max_tokens: usize,
    /// Chunk size for long texts (characters)
    pub chunk_size: usize,
    /// Overlap between chunks (characters)
    pub chunk_overlap: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            provider: "mistral".to_string(),
            model: MISTRAL_EMBED_MODEL.to_string(),
            dimension: MISTRAL_EMBED_DIMENSION,
            max_tokens: 8192,
            chunk_size: 512,
            chunk_overlap: 50,
        }
    }
}

impl EmbeddingConfig {
    /// Creates config for Mistral embed
    pub fn mistral() -> Self {
        Self::default()
    }

    /// Creates config for Ollama nomic-embed-text
    pub fn ollama_nomic() -> Self {
        Self {
            provider: "ollama".to_string(),
            model: "nomic-embed-text".to_string(),
            dimension: OLLAMA_NOMIC_DIMENSION,
            max_tokens: 8192,
            chunk_size: 512,
            chunk_overlap: 50,
        }
    }

    /// Creates config for Ollama mxbai-embed-large
    pub fn ollama_mxbai() -> Self {
        Self {
            provider: "ollama".to_string(),
            model: "mxbai-embed-large".to_string(),
            dimension: OLLAMA_MXBAI_DIMENSION,
            max_tokens: 8192,
            chunk_size: 512,
            chunk_overlap: 50,
        }
    }
}

// ============================================================================
// API Request/Response Types
// ============================================================================

/// Mistral embedding API request
#[derive(Debug, Serialize)]
struct MistralEmbeddingRequest<'a> {
    /// Model to use for embedding
    model: &'a str,
    /// Input texts to embed (single or batch)
    input: Vec<&'a str>,
    /// Encoding format (always "float")
    encoding_format: &'a str,
}

/// Mistral embedding API response
#[derive(Debug, Deserialize)]
struct MistralEmbeddingResponse {
    /// Response ID
    #[allow(dead_code)]
    id: String,
    /// Object type
    #[allow(dead_code)]
    object: String,
    /// Embedding data array
    data: Vec<MistralEmbeddingData>,
    /// Model used
    #[allow(dead_code)]
    model: String,
    /// Usage statistics
    #[allow(dead_code)]
    usage: MistralUsage,
}

/// Mistral embedding data item
#[derive(Debug, Deserialize)]
struct MistralEmbeddingData {
    /// Object type
    #[allow(dead_code)]
    object: String,
    /// Index in the input array
    #[allow(dead_code)]
    index: usize,
    /// Embedding vector
    embedding: Vec<f32>,
}

/// Mistral API usage statistics
#[derive(Debug, Deserialize)]
struct MistralUsage {
    /// Prompt tokens used
    #[allow(dead_code)]
    prompt_tokens: usize,
    /// Total tokens used
    #[allow(dead_code)]
    total_tokens: usize,
}

/// Ollama embedding API request
#[derive(Debug, Serialize)]
struct OllamaEmbeddingRequest<'a> {
    /// Model to use
    model: &'a str,
    /// Text to embed
    prompt: &'a str,
}

/// Ollama embedding API response
#[derive(Debug, Deserialize)]
struct OllamaEmbeddingResponse {
    /// Embedding vector
    embedding: Vec<f32>,
}

// ============================================================================
// Embedding Service
// ============================================================================

/// Service for generating vector embeddings
///
/// The EmbeddingService provides a unified interface for generating embeddings
/// using either Mistral or Ollama as the backend provider.
pub struct EmbeddingService {
    /// HTTP client for API requests
    client: Client,
    /// Configured provider
    provider: Arc<RwLock<Option<EmbeddingProvider>>>,
    /// Expected embedding dimension
    dimension: Arc<RwLock<usize>>,
    /// Request timeout in milliseconds
    timeout_ms: u64,
}

impl EmbeddingService {
    /// Creates a new unconfigured EmbeddingService
    ///
    /// The service must be configured with a provider before use.
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(DEFAULT_TIMEOUT_MS))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            provider: Arc::new(RwLock::new(None)),
            dimension: Arc::new(RwLock::new(MISTRAL_EMBED_DIMENSION)),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Creates a new EmbeddingService with the specified provider
    ///
    /// # Arguments
    /// * `provider` - The embedding provider configuration
    ///
    /// # Returns
    /// A configured EmbeddingService ready for use
    pub fn with_provider(provider: EmbeddingProvider) -> Self {
        let dimension = provider.dimension();
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(DEFAULT_TIMEOUT_MS))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            provider: Arc::new(RwLock::new(Some(provider))),
            dimension: Arc::new(RwLock::new(dimension)),
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }

    /// Configures the service with a new provider
    ///
    /// # Arguments
    /// * `provider` - The new embedding provider configuration
    pub async fn configure(&self, provider: EmbeddingProvider) {
        let dimension = provider.dimension();
        *self.provider.write().await = Some(provider);
        *self.dimension.write().await = dimension;
        info!("Embedding service configured");
    }

    /// Clears the provider configuration
    pub async fn clear(&self) {
        *self.provider.write().await = None;
        info!("Embedding service cleared");
    }

    /// Checks if the service is configured
    pub fn is_configured(&self) -> bool {
        self.provider
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Returns the expected embedding dimension
    pub async fn dimension(&self) -> usize {
        *self.dimension.read().await
    }

    /// Validates input text before embedding
    fn validate_text(&self, text: &str) -> Result<(), EmbeddingError> {
        if text.is_empty() {
            return Err(EmbeddingError::InvalidResponse(
                "Empty text cannot be embedded".to_string(),
            ));
        }
        if text.len() > MAX_EMBEDDING_TEXT_LENGTH {
            return Err(EmbeddingError::TextTooLong(
                text.len(),
                MAX_EMBEDDING_TEXT_LENGTH,
            ));
        }
        Ok(())
    }

    /// Generates an embedding for a single text
    ///
    /// # Arguments
    /// * `text` - The text to embed
    ///
    /// # Returns
    /// A vector of f32 values representing the embedding
    ///
    /// # Errors
    /// Returns an error if the provider is not configured or the API request fails
    #[instrument(
        name = "embed",
        skip(self, text),
        fields(text_len = text.len())
    )]
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        self.validate_text(text)?;

        let provider_guard = self.provider.read().await;
        let provider = provider_guard.as_ref().ok_or_else(|| {
            EmbeddingError::NotConfigured("No embedding provider configured".to_string())
        })?;

        match provider {
            EmbeddingProvider::Mistral { api_key, model } => {
                self.embed_mistral(text, api_key, model).await
            }
            EmbeddingProvider::Ollama { base_url, model } => {
                self.embed_ollama(text, base_url, model).await
            }
        }
    }

    /// Generates embeddings for multiple texts in batch
    ///
    /// # Arguments
    /// * `texts` - Slice of texts to embed
    ///
    /// # Returns
    /// A vector of embedding vectors, one per input text
    ///
    /// # Errors
    /// Returns an error if the batch is too large or any embedding fails
    #[instrument(
        name = "embed_batch",
        skip(self, texts),
        fields(batch_size = texts.len())
    )]
    pub async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        if texts.len() > MAX_BATCH_SIZE {
            return Err(EmbeddingError::BatchTooLarge(texts.len(), MAX_BATCH_SIZE));
        }

        // Validate all texts
        for text in texts {
            self.validate_text(text)?;
        }

        let provider_guard = self.provider.read().await;
        let provider = provider_guard.as_ref().ok_or_else(|| {
            EmbeddingError::NotConfigured("No embedding provider configured".to_string())
        })?;

        match provider {
            EmbeddingProvider::Mistral { api_key, model } => {
                self.embed_batch_mistral(texts, api_key, model).await
            }
            EmbeddingProvider::Ollama { base_url, model } => {
                // Ollama doesn't support batch, so we process sequentially
                self.embed_batch_ollama(texts, base_url, model).await
            }
        }
    }

    /// Embeds text using Mistral API
    async fn embed_mistral(
        &self,
        text: &str,
        api_key: &str,
        model: &str,
    ) -> Result<Vec<f32>, EmbeddingError> {
        let request = MistralEmbeddingRequest {
            model,
            input: vec![text],
            encoding_format: "float",
        };

        debug!(model = model, "Sending Mistral embedding request");

        let response = self
            .client
            .post(MISTRAL_EMBEDDING_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::RequestFailed(format!(
                "Mistral API returned {}: {}",
                status, body
            )));
        }

        let result: MistralEmbeddingResponse = response
            .json()
            .await
            .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

        let embedding = result
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .ok_or_else(|| {
                EmbeddingError::InvalidResponse("No embedding in response".to_string())
            })?;

        // Validate dimension
        let expected_dim = *self.dimension.read().await;
        if embedding.len() != expected_dim {
            warn!(
                expected = expected_dim,
                actual = embedding.len(),
                "Embedding dimension mismatch"
            );
        }

        debug!(
            dimension = embedding.len(),
            "Mistral embedding generated successfully"
        );

        Ok(embedding)
    }

    /// Embeds batch using Mistral API (native batch support)
    async fn embed_batch_mistral(
        &self,
        texts: &[&str],
        api_key: &str,
        model: &str,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let request = MistralEmbeddingRequest {
            model,
            input: texts.to_vec(),
            encoding_format: "float",
        };

        debug!(
            model = model,
            batch_size = texts.len(),
            "Sending Mistral batch embedding request"
        );

        let response = self
            .client
            .post(MISTRAL_EMBEDDING_URL)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::RequestFailed(format!(
                "Mistral API returned {}: {}",
                status, body
            )));
        }

        let result: MistralEmbeddingResponse = response
            .json()
            .await
            .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

        // Sort by index to maintain input order
        let mut embeddings: Vec<_> = result.data.into_iter().collect();
        embeddings.sort_by_key(|d| d.index);

        let embeddings: Vec<Vec<f32>> = embeddings.into_iter().map(|d| d.embedding).collect();

        debug!(
            count = embeddings.len(),
            "Mistral batch embedding generated successfully"
        );

        Ok(embeddings)
    }

    /// Embeds text using Ollama API
    async fn embed_ollama(
        &self,
        text: &str,
        base_url: &str,
        model: &str,
    ) -> Result<Vec<f32>, EmbeddingError> {
        let url = format!("{}/api/embeddings", base_url);
        let request = OllamaEmbeddingRequest {
            model,
            prompt: text,
        };

        debug!(model = model, url = %url, "Sending Ollama embedding request");

        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    EmbeddingError::ConnectionError(format!(
                        "Cannot connect to Ollama server at {}. Is Ollama running?",
                        base_url
                    ))
                } else {
                    EmbeddingError::from(e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // Check for model not found error
            if body.contains("not found") || body.contains("does not exist") {
                return Err(EmbeddingError::ModelNotAvailable(format!(
                    "Model '{}' not found. Try: ollama pull {}",
                    model, model
                )));
            }

            return Err(EmbeddingError::RequestFailed(format!(
                "Ollama API returned {}: {}",
                status, body
            )));
        }

        let result: OllamaEmbeddingResponse = response
            .json()
            .await
            .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

        debug!(
            dimension = result.embedding.len(),
            "Ollama embedding generated successfully"
        );

        Ok(result.embedding)
    }

    /// Embeds batch using Ollama API (sequential, no native batch support)
    async fn embed_batch_ollama(
        &self,
        texts: &[&str],
        base_url: &str,
        model: &str,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        debug!(
            batch_size = texts.len(),
            "Processing Ollama batch sequentially"
        );

        let mut embeddings = Vec::with_capacity(texts.len());
        for text in texts {
            let embedding = self.embed_ollama(text, base_url, model).await?;
            embeddings.push(embedding);
        }

        debug!(count = embeddings.len(), "Ollama batch embedding completed");

        Ok(embeddings)
    }

    /// Tests the embedding service connection
    ///
    /// Generates a small test embedding to verify the service is working.
    ///
    /// # Returns
    /// Ok(dimension) if successful, Err if connection fails
    pub async fn test_connection(&self) -> Result<usize, EmbeddingError> {
        let test_text = "test";
        let embedding = self.embed(test_text).await?;
        info!(
            dimension = embedding.len(),
            "Embedding service test successful"
        );
        Ok(embedding.len())
    }
}

impl Default for EmbeddingService {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // EmbeddingError Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_embedding_error_display() {
        let err = EmbeddingError::TextTooLong(60000, 50000);
        assert!(err.to_string().contains("60000"));
        assert!(err.to_string().contains("50000"));

        let err = EmbeddingError::NotConfigured("test".to_string());
        assert!(err.to_string().contains("not configured"));

        let err = EmbeddingError::BatchTooLarge(100, 96);
        assert!(err.to_string().contains("100"));
        assert!(err.to_string().contains("96"));
    }

    // -------------------------------------------------------------------------
    // EmbeddingProvider Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_embedding_provider_mistral() {
        let provider = EmbeddingProvider::mistral("test-key");
        assert_eq!(provider.name(), "mistral");
        assert_eq!(provider.model(), MISTRAL_EMBED_MODEL);
        assert_eq!(provider.dimension(), MISTRAL_EMBED_DIMENSION);
    }

    #[test]
    fn test_embedding_provider_ollama() {
        let provider = EmbeddingProvider::ollama();
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model(), DEFAULT_OLLAMA_EMBED_MODEL);
        assert_eq!(provider.dimension(), OLLAMA_NOMIC_DIMENSION);
    }

    #[test]
    fn test_embedding_provider_ollama_mxbai() {
        let provider =
            EmbeddingProvider::ollama_with_config("http://localhost:11434", "mxbai-embed-large");
        assert_eq!(provider.dimension(), OLLAMA_MXBAI_DIMENSION);
    }

    #[test]
    fn test_embedding_provider_ollama_nomic() {
        let provider =
            EmbeddingProvider::ollama_with_config("http://localhost:11434", "nomic-embed-text");
        assert_eq!(provider.dimension(), OLLAMA_NOMIC_DIMENSION);
    }

    // -------------------------------------------------------------------------
    // EmbeddingConfig Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.provider, "mistral");
        assert_eq!(config.model, MISTRAL_EMBED_MODEL);
        assert_eq!(config.dimension, MISTRAL_EMBED_DIMENSION);
    }

    #[test]
    fn test_embedding_config_ollama_nomic() {
        let config = EmbeddingConfig::ollama_nomic();
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.model, "nomic-embed-text");
        assert_eq!(config.dimension, OLLAMA_NOMIC_DIMENSION);
    }

    #[test]
    fn test_embedding_config_ollama_mxbai() {
        let config = EmbeddingConfig::ollama_mxbai();
        assert_eq!(config.provider, "ollama");
        assert_eq!(config.model, "mxbai-embed-large");
        assert_eq!(config.dimension, OLLAMA_MXBAI_DIMENSION);
    }

    #[test]
    fn test_embedding_config_serialization() {
        let config = EmbeddingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.provider, config.provider);
        assert_eq!(deserialized.model, config.model);
        assert_eq!(deserialized.dimension, config.dimension);
    }

    // -------------------------------------------------------------------------
    // EmbeddingService Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_embedding_service_new() {
        let service = EmbeddingService::new();
        assert!(!service.is_configured());
    }

    #[test]
    fn test_embedding_service_with_provider() {
        let provider = EmbeddingProvider::mistral("test-key");
        let service = EmbeddingService::with_provider(provider);
        assert!(service.is_configured());
    }

    #[tokio::test]
    async fn test_embedding_service_configure() {
        let service = EmbeddingService::new();
        assert!(!service.is_configured());

        let provider = EmbeddingProvider::mistral("test-key");
        service.configure(provider).await;
        assert!(service.is_configured());

        service.clear().await;
        assert!(!service.is_configured());
    }

    #[tokio::test]
    async fn test_embedding_service_dimension() {
        let provider = EmbeddingProvider::mistral("test-key");
        let service = EmbeddingService::with_provider(provider);
        assert_eq!(service.dimension().await, MISTRAL_EMBED_DIMENSION);
    }

    #[tokio::test]
    async fn test_embedding_service_dimension_ollama() {
        let provider = EmbeddingProvider::ollama();
        let service = EmbeddingService::with_provider(provider);
        assert_eq!(service.dimension().await, OLLAMA_NOMIC_DIMENSION);
    }

    // -------------------------------------------------------------------------
    // Validation Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_validate_text_empty() {
        let service = EmbeddingService::new();
        let result = service.validate_text("");
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::InvalidResponse(_) => {}
            _ => panic!("Expected InvalidResponse error"),
        }
    }

    #[test]
    fn test_validate_text_too_long() {
        let service = EmbeddingService::new();
        let long_text = "x".repeat(MAX_EMBEDDING_TEXT_LENGTH + 1);
        let result = service.validate_text(&long_text);
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::TextTooLong(len, max) => {
                assert_eq!(len, MAX_EMBEDDING_TEXT_LENGTH + 1);
                assert_eq!(max, MAX_EMBEDDING_TEXT_LENGTH);
            }
            _ => panic!("Expected TextTooLong error"),
        }
    }

    #[test]
    fn test_validate_text_valid() {
        let service = EmbeddingService::new();
        let result = service.validate_text("Hello, world!");
        assert!(result.is_ok());
    }

    // -------------------------------------------------------------------------
    // Error Handling Tests
    // -------------------------------------------------------------------------

    #[tokio::test]
    async fn test_embed_not_configured() {
        let service = EmbeddingService::new();
        let result = service.embed("test").await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::NotConfigured(_) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[tokio::test]
    async fn test_embed_batch_not_configured() {
        let service = EmbeddingService::new();
        let result = service.embed_batch(&["test1", "test2"]).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::NotConfigured(_) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[tokio::test]
    async fn test_embed_batch_empty() {
        let service = EmbeddingService::new();
        let result = service.embed_batch(&[]).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_embed_batch_too_large() {
        let provider = EmbeddingProvider::mistral("test-key");
        let service = EmbeddingService::with_provider(provider);

        let texts: Vec<&str> = (0..MAX_BATCH_SIZE + 1).map(|_| "test").collect();
        let result = service.embed_batch(&texts).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            EmbeddingError::BatchTooLarge(size, max) => {
                assert_eq!(size, MAX_BATCH_SIZE + 1);
                assert_eq!(max, MAX_BATCH_SIZE);
            }
            _ => panic!("Expected BatchTooLarge error"),
        }
    }

    // -------------------------------------------------------------------------
    // Request/Response Serialization Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_mistral_request_serialization() {
        let request = MistralEmbeddingRequest {
            model: "mistral-embed",
            input: vec!["Hello", "World"],
            encoding_format: "float",
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("mistral-embed"));
        assert!(json.contains("Hello"));
        assert!(json.contains("World"));
        assert!(json.contains("float"));
    }

    #[test]
    fn test_mistral_response_deserialization() {
        let json = r#"{
            "id": "emb-123",
            "object": "list",
            "data": [
                {
                    "object": "embedding",
                    "index": 0,
                    "embedding": [0.1, 0.2, 0.3]
                }
            ],
            "model": "mistral-embed",
            "usage": {
                "prompt_tokens": 10,
                "total_tokens": 10
            }
        }"#;

        let response: MistralEmbeddingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "emb-123");
        assert_eq!(response.data.len(), 1);
        assert_eq!(response.data[0].embedding, vec![0.1, 0.2, 0.3]);
    }

    #[test]
    fn test_ollama_request_serialization() {
        let request = OllamaEmbeddingRequest {
            model: "nomic-embed-text",
            prompt: "Hello, world!",
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("nomic-embed-text"));
        assert!(json.contains("Hello, world!"));
    }

    #[test]
    fn test_ollama_response_deserialization() {
        let json = r#"{
            "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
        }"#;

        let response: OllamaEmbeddingResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.embedding.len(), 5);
        assert_eq!(response.embedding[0], 0.1);
    }
}
