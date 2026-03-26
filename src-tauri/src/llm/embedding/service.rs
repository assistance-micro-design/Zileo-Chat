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

//! EmbeddingService - unified interface for generating vector embeddings.

use reqwest::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, instrument};

use super::providers;
#[cfg(test)]
use super::MISTRAL_EMBED_DIMENSION;
use super::{
    EmbeddingError, EmbeddingProvider, DEFAULT_TIMEOUT_MS, MAX_BATCH_SIZE,
    MAX_EMBEDDING_TEXT_LENGTH,
};

/// Service for generating vector embeddings
///
/// Provides a unified interface for generating embeddings
/// using either Mistral or Ollama as the backend provider.
pub struct EmbeddingService {
    /// HTTP client for API requests
    pub(super) client: Client,
    /// Configured provider
    pub(super) provider: Arc<RwLock<Option<EmbeddingProvider>>>,
    /// Expected embedding dimension
    pub(super) dimension: Arc<RwLock<usize>>,
}

impl EmbeddingService {
    /// Creates a new unconfigured EmbeddingService (test-only).
    #[cfg(test)]
    pub fn new() -> Result<Self, String> {
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(DEFAULT_TIMEOUT_MS))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            provider: Arc::new(RwLock::new(None)),
            dimension: Arc::new(RwLock::new(MISTRAL_EMBED_DIMENSION)),
        })
    }

    /// Creates a new EmbeddingService with the specified provider.
    pub fn with_provider(provider: EmbeddingProvider) -> Result<Self, String> {
        let dimension = provider.dimension();
        let client = Client::builder()
            .timeout(std::time::Duration::from_millis(DEFAULT_TIMEOUT_MS))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            client,
            provider: Arc::new(RwLock::new(Some(provider))),
            dimension: Arc::new(RwLock::new(dimension)),
        })
    }

    /// Configures the service with a new provider.
    #[allow(dead_code)]
    pub async fn configure(&self, provider: EmbeddingProvider) {
        let dimension = provider.dimension();
        *self.provider.write().await = Some(provider);
        *self.dimension.write().await = dimension;
        info!("Embedding service configured");
    }

    /// Clears the provider configuration.
    #[allow(dead_code)]
    pub async fn clear(&self) {
        *self.provider.write().await = None;
        info!("Embedding service cleared");
    }

    /// Checks if the service is configured.
    #[allow(dead_code)]
    pub fn is_configured(&self) -> bool {
        self.provider
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Returns the expected embedding dimension.
    #[allow(dead_code)]
    pub async fn dimension(&self) -> usize {
        *self.dimension.read().await
    }

    /// Validates input text before embedding.
    pub(super) fn validate_text(&self, text: &str) -> Result<(), EmbeddingError> {
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

    /// Generates an embedding for a single text.
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
                providers::embed_mistral(&self.client, &self.dimension, text, api_key, model).await
            }
            EmbeddingProvider::Ollama { base_url, model } => {
                providers::embed_ollama(&self.client, text, base_url, model).await
            }
        }
    }

    /// Generates embeddings for multiple texts in batch.
    #[allow(dead_code)]
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

        for text in texts {
            self.validate_text(text)?;
        }

        let provider_guard = self.provider.read().await;
        let provider = provider_guard.as_ref().ok_or_else(|| {
            EmbeddingError::NotConfigured("No embedding provider configured".to_string())
        })?;

        match provider {
            EmbeddingProvider::Mistral { api_key, model } => {
                providers::embed_batch_mistral(&self.client, texts, api_key, model).await
            }
            EmbeddingProvider::Ollama { base_url, model } => {
                providers::embed_batch_ollama(&self.client, texts, base_url, model).await
            }
        }
    }

    /// Tests the embedding service connection.
    #[allow(dead_code)]
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
