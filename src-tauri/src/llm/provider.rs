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

use crate::models::agent::ReasoningEffort;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// LLM provider type.
///
/// Mistral and Ollama are builtin providers with dedicated implementations.
/// Custom(String) represents user-created OpenAI-compatible providers
/// (RouterLab, OpenRouter, Together AI, etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProviderType {
    /// Mistral AI cloud API
    Mistral,
    /// Local Ollama server
    Ollama,
    /// User-created OpenAI-compatible provider (e.g., Custom("routerlab"))
    Custom(String),
}

impl ProviderType {
    /// Returns the lowercase provider identifier matching serde serialization.
    /// Use this instead of `.to_string()` (Display) for DB storage and queries.
    pub fn as_id(&self) -> &str {
        match self {
            ProviderType::Mistral => "mistral",
            ProviderType::Ollama => "ollama",
            ProviderType::Custom(name) => name,
        }
    }
}

impl Serialize for ProviderType {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            ProviderType::Mistral => s.serialize_str("mistral"),
            ProviderType::Ollama => s.serialize_str("ollama"),
            ProviderType::Custom(name) => s.serialize_str(name),
        }
    }
}

impl<'de> Deserialize<'de> for ProviderType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "mistral" => ProviderType::Mistral,
            "ollama" => ProviderType::Ollama,
            _ => ProviderType::Custom(s),
        })
    }
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Mistral => write!(f, "Mistral"),
            ProviderType::Ollama => write!(f, "Ollama"),
            ProviderType::Custom(name) => write!(f, "{}", name),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = LLMError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mistral" => Ok(ProviderType::Mistral),
            "ollama" => Ok(ProviderType::Ollama),
            other => {
                if other.is_empty() {
                    Err(LLMError::Internal(format!("Invalid provider: {}", s)))
                } else {
                    Ok(ProviderType::Custom(other.to_string()))
                }
            }
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
    /// Thinking content from reasoning models (e.g. Mistral Magistral)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub thinking_content: Option<String>,
    /// Number of tokens used for reasoning/thinking
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub thinking_tokens: Option<usize>,
    /// Cached prompt tokens (cache reads).
    /// `None` when the provider does not expose cache stats.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cached_tokens: Option<usize>,
    /// Prompt tokens written to cache (cache writes, first request).
    /// `None` when the provider does not expose cache stats.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub cache_write_tokens: Option<usize>,
    /// Provider-reported cost in USD when available (e.g. OpenRouter `usage.cost`).
    /// When `Some`, takes precedence over local pricing calculation.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(default)]
    pub provider_cost_usd: Option<f64>,
}

/// LLM error types
#[derive(Debug, Error)]
// Variants emitted from various lib code paths; not all are constructed in
// every test target, hence the module-level allow.
#[allow(dead_code)]
pub enum LLMError {
    /// Provider not configured
    #[error("Provider not configured: {0}")]
    NotConfigured(String),

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

    /// Circuit breaker is open (provider temporarily unavailable)
    #[error("Circuit breaker open for provider: {0}")]
    CircuitOpen(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Operation was cancelled via a `CancellationToken`.
    /// Non-retryable: cancellation is intentional, not a transient failure.
    #[error("Operation cancelled")]
    Cancelled,

    /// SSE streaming response or single payload exceeded the safety cap.
    /// Prevents OOM when a misbehaving upstream sends a runaway response.
    #[error("Response too large: {what}")]
    ResponseTooLarge {
        /// What exceeded the cap (e.g. "SSE buffer", "single payload")
        what: &'static str,
    },

    /// HTTP 4xx client error (excluding 429 which remains retryable as
    /// rate-limiting). Marker variant: callers receive a status hint and
    /// `is_retryable` short-circuits to false. Distinguishes auth /
    /// validation failures from transient server-side issues that
    /// `RequestFailed` represents.
    #[error("Client error: HTTP {status}: {message}")]
    ClientError {
        /// HTTP status code (400-499 except 429)
        status: u16,
        /// User-facing error message
        message: String,
    },
}

impl From<anyhow::Error> for LLMError {
    fn from(err: anyhow::Error) -> Self {
        LLMError::Internal(err.to_string())
    }
}

/// Parameters for a completion request.
///
/// Groups all parameters passed to `LLMProvider::complete()` to avoid
/// long positional argument lists.
#[derive(Debug, Clone)]
pub struct CompletionParams {
    /// The user prompt
    pub prompt: String,
    /// Optional system prompt
    pub system_prompt: Option<String>,
    /// Model to use (None for default)
    pub model: Option<String>,
    /// Sampling temperature (0.0-1.0)
    pub temperature: f64,
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Reasoning effort level (None = no thinking)
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Context window size from model config (e.g. Ollama num_ctx)
    pub context_window: Option<usize>,
}

/// Parameters for a tool-augmented completion request.
#[derive(Debug, Clone)]
pub struct ToolCompletionParams {
    /// Conversation history as JSON messages
    pub messages: Vec<serde_json::Value>,
    /// Tool definitions in OpenAI format
    pub tools: Vec<serde_json::Value>,
    /// How the model should use tools (provider-specific)
    pub tool_choice: Option<serde_json::Value>,
    /// Model to use
    pub model: String,
    /// Sampling temperature
    pub temperature: f64,
    /// Maximum tokens to generate
    pub max_tokens: usize,
    /// Context window size (e.g. Ollama num_ctx)
    pub context_window: Option<usize>,
    /// Reasoning effort level for thinking models (e.g. Mistral reasoning_effort)
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Common trait for all LLM providers
#[async_trait]
// Trait used via dyn dispatch by the LLM layer; reachable from lib code only.
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
    async fn complete(&self, params: CompletionParams) -> Result<LLMResponse, LLMError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_display() {
        assert_eq!(ProviderType::Mistral.to_string(), "Mistral");
        assert_eq!(ProviderType::Ollama.to_string(), "Ollama");
        assert_eq!(
            ProviderType::Custom("routerlab".to_string()).to_string(),
            "routerlab"
        );
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
    fn test_provider_type_from_str_custom() {
        let result = "routerlab".parse::<ProviderType>();
        assert_eq!(
            result.unwrap(),
            ProviderType::Custom("routerlab".to_string())
        );
    }

    #[test]
    fn test_provider_type_from_str_empty() {
        let result = "".parse::<ProviderType>();
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
    fn test_provider_type_as_id() {
        assert_eq!(ProviderType::Mistral.as_id(), "mistral");
        assert_eq!(ProviderType::Ollama.as_id(), "ollama");
        assert_eq!(
            ProviderType::Custom("routerlab".to_string()).as_id(),
            "routerlab"
        );
    }

    #[test]
    fn test_provider_type_deserialize_case_insensitive() {
        let mistral: ProviderType = serde_json::from_str("\"Mistral\"").unwrap();
        assert_eq!(mistral, ProviderType::Mistral);

        let ollama: ProviderType = serde_json::from_str("\"Ollama\"").unwrap();
        assert_eq!(ollama, ProviderType::Ollama);

        let upper: ProviderType = serde_json::from_str("\"MISTRAL\"").unwrap();
        assert_eq!(upper, ProviderType::Mistral);

        // Custom providers preserve original casing
        let custom: ProviderType = serde_json::from_str("\"RouterLab\"").unwrap();
        assert_eq!(custom, ProviderType::Custom("RouterLab".to_string()));
    }

    /// Helper for tests: builds a baseline response with all optional fields None.
    fn baseline_response() -> LLMResponse {
        LLMResponse {
            content: "Hello, world!".to_string(),
            tokens_input: 10,
            tokens_output: 5,
            model: "mistral-large".to_string(),
            provider: ProviderType::Mistral,
            finish_reason: Some("stop".to_string()),
            thinking_content: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            provider_cost_usd: None,
        }
    }

    #[test]
    fn test_llm_response_serialization() {
        let response = baseline_response();
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: LLMResponse = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, response.content);
        assert_eq!(deserialized.tokens_input, response.tokens_input);
        assert_eq!(deserialized.tokens_output, response.tokens_output);
        assert_eq!(deserialized.model, response.model);
        assert_eq!(deserialized.provider, response.provider);
        assert_eq!(deserialized.thinking_content, None);
    }

    #[test]
    fn test_llm_response_with_thinking_content() {
        let response = LLMResponse {
            content: "The answer is 42.".to_string(),
            tokens_input: 15,
            tokens_output: 8,
            model: "mistral-magistral".to_string(),
            provider: ProviderType::Mistral,
            finish_reason: Some("stop".to_string()),
            thinking_content: Some("Let me reason about this...".to_string()),
            thinking_tokens: Some(6),
            cached_tokens: None,
            cache_write_tokens: None,
            provider_cost_usd: None,
        };

        // When Some, thinking_content IS serialized
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("thinking_content"));
        assert!(json.contains("Let me reason about this..."));

        let deserialized: LLMResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(
            deserialized.thinking_content,
            Some("Let me reason about this...".to_string())
        );
    }

    #[test]
    fn test_llm_response_thinking_none_omitted_from_json() {
        let response = LLMResponse {
            content: "Hello".to_string(),
            tokens_input: 5,
            tokens_output: 1,
            model: "test".to_string(),
            provider: ProviderType::Ollama,
            finish_reason: None,
            thinking_content: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            provider_cost_usd: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(!json.contains("thinking_content"));
        assert!(!json.contains("thinking_tokens"));
        // Cache + provider cost fields also omitted when None.
        assert!(!json.contains("cached_tokens"));
        assert!(!json.contains("cache_write_tokens"));
        assert!(!json.contains("provider_cost_usd"));
    }

    #[test]
    fn test_llm_response_cache_fields_serialized_when_some() {
        let response = LLMResponse {
            cached_tokens: Some(100),
            cache_write_tokens: Some(50),
            provider_cost_usd: Some(0.0123),
            ..baseline_response()
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"cached_tokens\":100"));
        assert!(json.contains("\"cache_write_tokens\":50"));
        assert!(json.contains("\"provider_cost_usd\":0.0123"));

        let deserialized: LLMResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.cached_tokens, Some(100));
        assert_eq!(deserialized.cache_write_tokens, Some(50));
        assert_eq!(deserialized.provider_cost_usd, Some(0.0123));
    }

    #[test]
    fn test_llm_response_deserializes_legacy_payload_without_cache_fields() {
        // A legacy persisted payload lacks the new optional fields.
        // serde(default) must produce None for all of them.
        let legacy_json = r#"{
            "content": "ok",
            "tokens_input": 1,
            "tokens_output": 2,
            "model": "m",
            "provider": "mistral",
            "finish_reason": "stop"
        }"#;
        let parsed: LLMResponse = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(parsed.cached_tokens, None);
        assert_eq!(parsed.cache_write_tokens, None);
        assert_eq!(parsed.provider_cost_usd, None);
    }

    #[test]
    fn test_llm_error_display() {
        let err = LLMError::NotConfigured("Mistral".to_string());
        assert!(err.to_string().contains("not configured"));

        let err = LLMError::MissingApiKey("Mistral".to_string());
        assert!(err.to_string().contains("API key missing"));
    }
}
