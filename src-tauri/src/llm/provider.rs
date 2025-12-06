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

//! LLM Provider trait and common types

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// LLM provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// Mistral AI cloud API
    Mistral,
    /// Local Ollama server
    Ollama,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Mistral => write!(f, "Mistral"),
            ProviderType::Ollama => write!(f, "Ollama"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = LLMError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mistral" => Ok(ProviderType::Mistral),
            "ollama" => Ok(ProviderType::Ollama),
            _ => Err(LLMError::InvalidProvider(s.to_string())),
        }
    }
}

/// LLM response from a completion request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMResponse {
    /// Generated text content
    pub content: String,
    /// Number of input tokens (prompt)
    pub tokens_input: usize,
    /// Number of output tokens (completion)
    pub tokens_output: usize,
    /// Model used for generation
    pub model: String,
    /// Provider used
    pub provider: ProviderType,
    /// Finish reason (if available)
    pub finish_reason: Option<String>,
}

/// LLM error types
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum LLMError {
    /// Provider not configured
    #[error("Provider not configured: {0}")]
    NotConfigured(String),

    /// Invalid provider name
    #[error("Invalid provider: {0}")]
    InvalidProvider(String),

    /// API key missing
    #[error("API key missing for provider: {0}")]
    MissingApiKey(String),

    /// API request failed
    #[error("API request failed: {0}")]
    RequestFailed(String),

    /// Model not found
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Connection error (for Ollama)
    #[error("Connection error: {0}")]
    ConnectionError(String),

    /// Streaming error
    #[error("Streaming error: {0}")]
    StreamingError(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<anyhow::Error> for LLMError {
    fn from(err: anyhow::Error) -> Self {
        LLMError::Internal(err.to_string())
    }
}

/// Common trait for all LLM providers
#[async_trait]
#[allow(dead_code)]
pub trait LLMProvider: Send + Sync {
    /// Returns the provider type
    fn provider_type(&self) -> ProviderType;

    /// Returns available model names
    fn available_models(&self) -> Vec<String>;

    /// Returns the default model name
    fn default_model(&self) -> String;

    /// Checks if the provider is properly configured
    fn is_configured(&self) -> bool;

    /// Generates a completion for the given prompt
    ///
    /// # Arguments
    /// * `prompt` - The user prompt
    /// * `system_prompt` - Optional system prompt
    /// * `model` - Model to use (None for default)
    /// * `temperature` - Sampling temperature (0.0-1.0)
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    /// LLMResponse with the generated content and metrics
    async fn complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<LLMResponse, LLMError>;

    /// Generates a streaming completion
    ///
    /// Returns a receiver for streaming chunks. Each chunk contains partial content.
    ///
    /// # Arguments
    /// Same as `complete`
    ///
    /// # Returns
    /// A channel receiver for streaming text chunks
    async fn complete_stream(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: Option<&str>,
        temperature: f32,
        max_tokens: usize,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<String, LLMError>>, LLMError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_display() {
        assert_eq!(ProviderType::Mistral.to_string(), "Mistral");
        assert_eq!(ProviderType::Ollama.to_string(), "Ollama");
    }

    #[test]
    fn test_provider_type_from_str() {
        assert_eq!(
            "mistral".parse::<ProviderType>().unwrap(),
            ProviderType::Mistral
        );
        assert_eq!(
            "Mistral".parse::<ProviderType>().unwrap(),
            ProviderType::Mistral
        );
        assert_eq!(
            "ollama".parse::<ProviderType>().unwrap(),
            ProviderType::Ollama
        );
        assert_eq!(
            "OLLAMA".parse::<ProviderType>().unwrap(),
            ProviderType::Ollama
        );
    }

    #[test]
    fn test_provider_type_from_str_invalid() {
        let result = "invalid".parse::<ProviderType>();
        assert!(result.is_err());
    }

    #[test]
    fn test_provider_type_serialization() {
        let provider = ProviderType::Mistral;
        let json = serde_json::to_string(&provider).unwrap();
        assert_eq!(json, "\"mistral\"");

        let deserialized: ProviderType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProviderType::Mistral);
    }

    #[test]
    fn test_llm_response_serialization() {
        let response = LLMResponse {
            content: "Hello, world!".to_string(),
            tokens_input: 10,
            tokens_output: 5,
            model: "mistral-large".to_string(),
            provider: ProviderType::Mistral,
            finish_reason: Some("stop".to_string()),
        };

        let json = serde_json::to_string(&response).unwrap();
        let deserialized: LLMResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, response.content);
        assert_eq!(deserialized.tokens_input, response.tokens_input);
        assert_eq!(deserialized.tokens_output, response.tokens_output);
        assert_eq!(deserialized.model, response.model);
        assert_eq!(deserialized.provider, response.provider);
    }

    #[test]
    fn test_llm_error_display() {
        let err = LLMError::NotConfigured("Mistral".to_string());
        assert!(err.to_string().contains("not configured"));

        let err = LLMError::MissingApiKey("Mistral".to_string());
        assert!(err.to_string().contains("API key missing"));
    }
}
