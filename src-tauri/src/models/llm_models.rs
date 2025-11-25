// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! LLM model and provider settings types for CRUD operations.
//!
//! This module defines the data structures for managing LLM models (both builtin and custom)
//! and provider configuration settings. It supports Mistral and Ollama providers in Phase 1.

// Allow dead code temporarily - these types will be used in Phase 2 (Commands CRUD)
#![allow(dead_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// Provider Type
// ============================================================================

/// LLM provider type supported by the application.
///
/// Phase 1 supports Mistral (cloud API) and Ollama (local server).
/// Future phases may add Claude, GPT-4, Gemini, etc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    /// Mistral AI cloud API
    Mistral,
    /// Ollama local inference server
    Ollama,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::Mistral => write!(f, "mistral"),
            ProviderType::Ollama => write!(f, "ollama"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mistral" => Ok(ProviderType::Mistral),
            "ollama" => Ok(ProviderType::Ollama),
            _ => Err(format!("Unknown provider type: {}", s)),
        }
    }
}

// ============================================================================
// LLM Model
// ============================================================================

/// LLM model definition (builtin or custom).
///
/// Models can be either builtin (shipped with the application and immutable)
/// or custom (user-created and fully editable).
///
/// # Fields
/// - `id`: Unique identifier (UUID for custom, api_name for builtin)
/// - `provider`: Which provider this model belongs to
/// - `name`: Human-readable display name
/// - `api_name`: Model identifier used in API calls (e.g., "mistral-large-latest")
/// - `context_window`: Maximum context length in tokens (1024 - 2,000,000)
/// - `max_output_tokens`: Maximum generation length (256 - 128,000)
/// - `temperature_default`: Default sampling temperature (0.0 - 2.0)
/// - `is_builtin`: Whether this is a system-provided model (cannot be deleted)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMModel {
    /// Unique identifier
    pub id: String,
    /// Provider this model belongs to
    pub provider: ProviderType,
    /// Human-readable display name
    pub name: String,
    /// Model identifier used in API calls
    pub api_name: String,
    /// Maximum context length in tokens
    pub context_window: usize,
    /// Maximum generation length in tokens
    pub max_output_tokens: usize,
    /// Default sampling temperature (0.0 - 2.0)
    pub temperature_default: f32,
    /// Whether this is a builtin model (cannot be deleted)
    pub is_builtin: bool,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl LLMModel {
    /// Creates a new custom LLM model with the given parameters.
    ///
    /// # Arguments
    /// * `id` - Unique identifier (typically a UUID)
    /// * `provider` - Provider type (Mistral or Ollama)
    /// * `name` - Human-readable display name
    /// * `api_name` - Model identifier for API calls
    /// * `context_window` - Maximum context length in tokens
    /// * `max_output_tokens` - Maximum generation length
    /// * `temperature_default` - Default sampling temperature
    pub fn new_custom(
        id: String,
        provider: ProviderType,
        name: String,
        api_name: String,
        context_window: usize,
        max_output_tokens: usize,
        temperature_default: f32,
    ) -> Self {
        let now = Utc::now();
        Self {
            id,
            provider,
            name,
            api_name,
            context_window,
            max_output_tokens,
            temperature_default,
            is_builtin: false,
            created_at: now,
            updated_at: now,
        }
    }

    /// Creates a new builtin LLM model.
    ///
    /// Builtin models use their api_name as the id and cannot be deleted.
    pub fn new_builtin(
        provider: ProviderType,
        name: String,
        api_name: String,
        context_window: usize,
        max_output_tokens: usize,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: api_name.clone(),
            provider,
            name,
            api_name,
            context_window,
            max_output_tokens,
            temperature_default: 0.7,
            is_builtin: true,
            created_at: now,
            updated_at: now,
        }
    }
}

// ============================================================================
// Create Model Request
// ============================================================================

/// Request payload for creating a new custom model.
///
/// All fields except `temperature_default` are required.
/// The `temperature_default` will default to 0.7 if not provided.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateModelRequest {
    /// Provider this model belongs to
    pub provider: ProviderType,
    /// Human-readable display name (1-64 characters)
    pub name: String,
    /// Model identifier used in API calls (must be unique per provider)
    pub api_name: String,
    /// Maximum context length in tokens (1024 - 2,000,000)
    pub context_window: usize,
    /// Maximum generation length in tokens (256 - 128,000)
    pub max_output_tokens: usize,
    /// Default sampling temperature (0.0 - 2.0, defaults to 0.7)
    #[serde(default = "default_temperature")]
    pub temperature_default: f32,
}

/// Default temperature value for new models.
fn default_temperature() -> f32 {
    0.7
}

impl CreateModelRequest {
    /// Validates the create request.
    ///
    /// # Returns
    /// - `Ok(())` if all validations pass
    /// - `Err(String)` with description of the first validation failure
    pub fn validate(&self) -> Result<(), String> {
        // Name validation
        if self.name.trim().is_empty() {
            return Err("Name is required".into());
        }
        if self.name.len() > 64 {
            return Err("Name must be 64 characters or less".into());
        }

        // API name validation
        if self.api_name.trim().is_empty() {
            return Err("API name is required".into());
        }
        if self.api_name.len() > 128 {
            return Err("API name must be 128 characters or less".into());
        }

        // Context window validation
        if self.context_window < 1024 {
            return Err("Context window must be at least 1024 tokens".into());
        }
        if self.context_window > 2_000_000 {
            return Err("Context window cannot exceed 2,000,000 tokens".into());
        }

        // Max output tokens validation
        if self.max_output_tokens < 256 {
            return Err("Max output tokens must be at least 256".into());
        }
        if self.max_output_tokens > 128_000 {
            return Err("Max output tokens cannot exceed 128,000".into());
        }

        // Temperature validation
        if !(0.0..=2.0).contains(&self.temperature_default) {
            return Err("Temperature must be between 0.0 and 2.0".into());
        }

        Ok(())
    }
}

// ============================================================================
// Update Model Request
// ============================================================================

/// Request payload for updating an existing model.
///
/// All fields are optional. Only provided fields will be updated.
/// For builtin models, only `temperature_default` can be modified.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateModelRequest {
    /// New display name (1-64 characters)
    pub name: Option<String>,
    /// New API name (must be unique per provider)
    pub api_name: Option<String>,
    /// New context window size (1024 - 2,000,000)
    pub context_window: Option<usize>,
    /// New max output tokens (256 - 128,000)
    pub max_output_tokens: Option<usize>,
    /// New default temperature (0.0 - 2.0)
    pub temperature_default: Option<f32>,
}

impl UpdateModelRequest {
    /// Validates the update request.
    ///
    /// # Arguments
    /// * `is_builtin` - Whether the target model is builtin (restricts editable fields)
    ///
    /// # Returns
    /// - `Ok(())` if all validations pass
    /// - `Err(String)` with description of the first validation failure
    pub fn validate(&self, is_builtin: bool) -> Result<(), String> {
        // For builtin models, only temperature can be changed
        if is_builtin {
            if self.name.is_some() {
                return Err("Cannot modify name of builtin model".into());
            }
            if self.api_name.is_some() {
                return Err("Cannot modify API name of builtin model".into());
            }
            if self.context_window.is_some() {
                return Err("Cannot modify context window of builtin model".into());
            }
            if self.max_output_tokens.is_some() {
                return Err("Cannot modify max output tokens of builtin model".into());
            }
        }

        // Name validation
        if let Some(ref name) = self.name {
            if name.trim().is_empty() {
                return Err("Name cannot be empty".into());
            }
            if name.len() > 64 {
                return Err("Name must be 64 characters or less".into());
            }
        }

        // API name validation
        if let Some(ref api_name) = self.api_name {
            if api_name.trim().is_empty() {
                return Err("API name cannot be empty".into());
            }
            if api_name.len() > 128 {
                return Err("API name must be 128 characters or less".into());
            }
        }

        // Context window validation
        if let Some(ctx) = self.context_window {
            if ctx < 1024 {
                return Err("Context window must be at least 1024 tokens".into());
            }
            if ctx > 2_000_000 {
                return Err("Context window cannot exceed 2,000,000 tokens".into());
            }
        }

        // Max output tokens validation
        if let Some(max_out) = self.max_output_tokens {
            if max_out < 256 {
                return Err("Max output tokens must be at least 256".into());
            }
            if max_out > 128_000 {
                return Err("Max output tokens cannot exceed 128,000".into());
            }
        }

        // Temperature validation
        if let Some(temp) = self.temperature_default {
            if !(0.0..=2.0).contains(&temp) {
                return Err("Temperature must be between 0.0 and 2.0".into());
            }
        }

        Ok(())
    }

    /// Returns true if no fields are set for update.
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.api_name.is_none()
            && self.context_window.is_none()
            && self.max_output_tokens.is_none()
            && self.temperature_default.is_none()
    }
}

// ============================================================================
// Provider Settings
// ============================================================================

/// Configuration settings for a provider.
///
/// Stores per-provider settings including enabled state, default model,
/// and optional base URL (primarily for Ollama).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    /// Provider type
    pub provider: ProviderType,
    /// Whether this provider is enabled
    pub enabled: bool,
    /// ID of the default model for this provider
    pub default_model_id: Option<String>,
    /// Whether an API key is configured (for Mistral)
    pub api_key_configured: bool,
    /// Custom base URL (primarily for Ollama, e.g., "http://localhost:11434")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl ProviderSettings {
    /// Creates default settings for a provider.
    pub fn default_for(provider: ProviderType) -> Self {
        Self {
            provider,
            enabled: true,
            default_model_id: None,
            api_key_configured: false,
            base_url: match provider {
                ProviderType::Ollama => Some("http://localhost:11434".into()),
                ProviderType::Mistral => None,
            },
            updated_at: Utc::now(),
        }
    }
}

// ============================================================================
// Connection Test Result
// ============================================================================

/// Result of a provider connection test.
///
/// Contains success status, latency measurement, and any error details.
#[derive(Debug, Clone, Serialize)]
pub struct ConnectionTestResult {
    /// Provider that was tested
    pub provider: ProviderType,
    /// Whether the connection was successful
    pub success: bool,
    /// Round-trip latency in milliseconds (if successful)
    pub latency_ms: Option<u64>,
    /// Error message (if failed)
    pub error_message: Option<String>,
    /// Model used for the test (if applicable)
    pub model_tested: Option<String>,
}

impl ConnectionTestResult {
    /// Creates a successful test result.
    pub fn success(provider: ProviderType, latency_ms: u64, model_tested: Option<String>) -> Self {
        Self {
            provider,
            success: true,
            latency_ms: Some(latency_ms),
            error_message: None,
            model_tested,
        }
    }

    /// Creates a failed test result.
    pub fn failure(provider: ProviderType, error_message: String) -> Self {
        Self {
            provider,
            success: false,
            latency_ms: None,
            error_message: Some(error_message),
            model_tested: None,
        }
    }
}

// ============================================================================
// Builtin Models Data
// ============================================================================

/// Mistral builtin models: (api_name, display_name, context_window, max_output_tokens)
pub const MISTRAL_BUILTIN_MODELS: &[(&str, &str, usize, usize)] = &[
    ("mistral-large-latest", "Mistral Large", 128_000, 8192),
    ("mistral-medium-latest", "Mistral Medium", 32_000, 8192),
    ("mistral-small-latest", "Mistral Small", 32_000, 8192),
    ("open-mistral-7b", "Open Mistral 7B", 32_000, 4096),
    ("open-mixtral-8x7b", "Open Mixtral 8x7B", 32_000, 4096),
    ("open-mixtral-8x22b", "Open Mixtral 8x22B", 64_000, 4096),
    ("codestral-latest", "Codestral", 256_000, 8192),
];

/// Ollama builtin models: (api_name, display_name, context_window, max_output_tokens)
pub const OLLAMA_BUILTIN_MODELS: &[(&str, &str, usize, usize)] = &[
    ("llama3.2", "Llama 3.2", 128_000, 4096),
    ("llama3.1", "Llama 3.1", 128_000, 4096),
    ("llama3", "Llama 3", 8_000, 4096),
    ("mistral", "Mistral (Ollama)", 32_000, 4096),
    ("mixtral", "Mixtral (Ollama)", 32_000, 4096),
    ("codellama", "Code Llama", 16_000, 4096),
    ("phi3", "Phi-3", 4_000, 2048),
    ("gemma2", "Gemma 2", 8_000, 4096),
    ("qwen2.5", "Qwen 2.5", 32_000, 4096),
];

/// Returns all builtin models for seeding the database.
pub fn get_all_builtin_models() -> Vec<LLMModel> {
    let mut models = Vec::with_capacity(MISTRAL_BUILTIN_MODELS.len() + OLLAMA_BUILTIN_MODELS.len());

    for (api_name, name, ctx, max_out) in MISTRAL_BUILTIN_MODELS {
        models.push(LLMModel::new_builtin(
            ProviderType::Mistral,
            (*name).to_string(),
            (*api_name).to_string(),
            *ctx,
            *max_out,
        ));
    }

    for (api_name, name, ctx, max_out) in OLLAMA_BUILTIN_MODELS {
        models.push(LLMModel::new_builtin(
            ProviderType::Ollama,
            (*name).to_string(),
            (*api_name).to_string(),
            *ctx,
            *max_out,
        ));
    }

    models
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_type_display() {
        assert_eq!(ProviderType::Mistral.to_string(), "mistral");
        assert_eq!(ProviderType::Ollama.to_string(), "ollama");
    }

    #[test]
    fn test_provider_type_from_str() {
        assert_eq!(
            "mistral".parse::<ProviderType>().unwrap(),
            ProviderType::Mistral
        );
        assert_eq!(
            "OLLAMA".parse::<ProviderType>().unwrap(),
            ProviderType::Ollama
        );
        assert!("unknown".parse::<ProviderType>().is_err());
    }

    #[test]
    fn test_create_model_request_validation() {
        let valid = CreateModelRequest {
            provider: ProviderType::Mistral,
            name: "Test Model".into(),
            api_name: "test-model".into(),
            context_window: 32000,
            max_output_tokens: 4096,
            temperature_default: 0.7,
        };
        assert!(valid.validate().is_ok());

        // Empty name
        let invalid = CreateModelRequest {
            name: "".into(),
            ..valid.clone()
        };
        assert!(invalid.validate().is_err());

        // Name too long
        let invalid = CreateModelRequest {
            name: "a".repeat(65),
            ..valid.clone()
        };
        assert!(invalid.validate().is_err());

        // Context window too small
        let invalid = CreateModelRequest {
            context_window: 512,
            ..valid.clone()
        };
        assert!(invalid.validate().is_err());

        // Temperature out of range
        let invalid = CreateModelRequest {
            temperature_default: 3.0,
            ..valid
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_update_model_request_builtin_validation() {
        let update = UpdateModelRequest {
            name: Some("New Name".into()),
            api_name: None,
            context_window: None,
            max_output_tokens: None,
            temperature_default: None,
        };

        // Should fail for builtin models
        assert!(update.validate(true).is_err());

        // Should pass for custom models
        assert!(update.validate(false).is_ok());

        // Temperature update should work for builtin
        let temp_update = UpdateModelRequest {
            name: None,
            api_name: None,
            context_window: None,
            max_output_tokens: None,
            temperature_default: Some(0.5),
        };
        assert!(temp_update.validate(true).is_ok());
    }

    #[test]
    fn test_llm_model_new_custom() {
        let model = LLMModel::new_custom(
            "test-id".into(),
            ProviderType::Ollama,
            "Test Model".into(),
            "test-model".into(),
            32000,
            4096,
            0.7,
        );

        assert_eq!(model.id, "test-id");
        assert!(!model.is_builtin);
        assert_eq!(model.provider, ProviderType::Ollama);
    }

    #[test]
    fn test_llm_model_new_builtin() {
        let model = LLMModel::new_builtin(
            ProviderType::Mistral,
            "Mistral Large".into(),
            "mistral-large-latest".into(),
            128000,
            8192,
        );

        assert_eq!(model.id, "mistral-large-latest");
        assert!(model.is_builtin);
        assert_eq!(model.temperature_default, 0.7);
    }

    #[test]
    fn test_get_all_builtin_models() {
        let models = get_all_builtin_models();
        assert_eq!(
            models.len(),
            MISTRAL_BUILTIN_MODELS.len() + OLLAMA_BUILTIN_MODELS.len()
        );

        // Check all models are builtin
        assert!(models.iter().all(|m| m.is_builtin));

        // Check we have both providers
        assert!(models.iter().any(|m| m.provider == ProviderType::Mistral));
        assert!(models.iter().any(|m| m.provider == ProviderType::Ollama));
    }

    #[test]
    fn test_connection_test_result() {
        let success = ConnectionTestResult::success(
            ProviderType::Mistral,
            150,
            Some("mistral-large-latest".into()),
        );
        assert!(success.success);
        assert_eq!(success.latency_ms, Some(150));
        assert!(success.error_message.is_none());

        let failure = ConnectionTestResult::failure(ProviderType::Ollama, "Connection refused".into());
        assert!(!failure.success);
        assert!(failure.latency_ms.is_none());
        assert!(failure.error_message.is_some());
    }

    #[test]
    fn test_provider_settings_default() {
        let mistral = ProviderSettings::default_for(ProviderType::Mistral);
        assert!(mistral.enabled);
        assert!(mistral.base_url.is_none());

        let ollama = ProviderSettings::default_for(ProviderType::Ollama);
        assert!(ollama.enabled);
        assert_eq!(
            ollama.base_url,
            Some("http://localhost:11434".into())
        );
    }
}
