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
//! Provides vector embedding generation for semantic search and RAG operations.
//! Supports multiple providers (Mistral, Ollama) with a unified interface.
//!
//! ## Architecture
//!
//! - [`EmbeddingService`] - Main service for embedding generation
//! - [`EmbeddingProvider`] - Enum defining supported embedding providers
//! - [`EmbeddingError`] - Error types for embedding operations

mod providers;
mod service;

#[cfg(test)]
mod tests;

pub use service::EmbeddingService;

use thiserror::Error;

/// Mistral embedding API endpoint
pub(crate) const MISTRAL_EMBEDDING_URL: &str = "https://api.mistral.ai/v1/embeddings";

/// Default Mistral embedding model
#[cfg(test)]
pub(crate) const MISTRAL_EMBED_MODEL: &str = "mistral-embed";

/// Mistral embed model dimension (1024D)
pub const MISTRAL_EMBED_DIMENSION: usize = 1024;

/// Ollama nomic-embed-text dimension (768D)
pub const OLLAMA_NOMIC_DIMENSION: usize = 768;

/// Ollama mxbai-embed-large dimension (1024D)
pub const OLLAMA_MXBAI_DIMENSION: usize = 1024;

/// Default embedding model for Ollama
#[cfg(test)]
pub(crate) const DEFAULT_OLLAMA_EMBED_MODEL: &str = "nomic-embed-text";

/// Maximum text length for embedding (characters)
pub const MAX_EMBEDDING_TEXT_LENGTH: usize = 50_000;

/// Default timeout for embedding requests (milliseconds)
pub const DEFAULT_TIMEOUT_MS: u64 = 30_000;

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

    /// Model not available for embedding
    #[error("Embedding model not available: {0}")]
    ModelNotAvailable(String),

    /// Connection error (e.g., Ollama server not running)
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Timeout error
    #[error("Request timed out after {0}ms")]
    Timeout(u64),
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
    /// Creates a Mistral provider with custom model
    pub fn mistral_with_model(api_key: &str, model: &str) -> Self {
        EmbeddingProvider::Mistral {
            api_key: api_key.to_string(),
            model: model.to_string(),
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
            EmbeddingProvider::Mistral { .. } => MISTRAL_EMBED_DIMENSION,
            EmbeddingProvider::Ollama { model, .. } => {
                if model.contains("mxbai") {
                    OLLAMA_MXBAI_DIMENSION
                } else {
                    OLLAMA_NOMIC_DIMENSION
                }
            }
        }
    }
}
