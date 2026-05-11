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

//! LLM model and provider settings types for CRUD operations.
//!
//! This module defines the data structures for managing LLM models (both builtin and custom)
//! and provider configuration settings.

use crate::llm::{ProviderType, DEFAULT_OLLAMA_URL};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

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
/// - `is_reasoning`: Whether this is a reasoning/thinking model (Magistral, DeepSeek-R1, etc.)
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
    pub temperature_default: f64,
    /// Whether this is a builtin model (cannot be deleted)
    pub is_builtin: bool,
    /// Whether this is a reasoning/thinking model (enables thinking output)
    #[serde(default)]
    pub is_reasoning: bool,
    /// Price per million input tokens (USD) - user configurable
    #[serde(default)]
    pub input_price_per_mtok: f64,
    /// Price per million output tokens (USD) - user configurable
    #[serde(default)]
    pub output_price_per_mtok: f64,
    /// Price per million cache-read input tokens (USD).
    /// Applied to `cached_tokens` from API response (cache hits).
    /// OpenRouter typical: 0.25x to 0.50x of input_price_per_mtok. 0.0 = free.
    #[serde(default)]
    pub cache_read_price_per_mtok: f64,
    /// Price per million cache-write input tokens (USD).
    /// Applied to `cache_write_tokens` from API response (cache misses written).
    /// OpenRouter typical: 1.0x to 1.25x of input_price_per_mtok. 0.0 = same as input.
    #[serde(default)]
    pub cache_write_price_per_mtok: f64,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl LLMModel {
    /// Creates a new custom LLM model from a create request.
    ///
    /// # Arguments
    /// * `id` - Unique identifier (typically a UUID)
    /// * `request` - The creation request with model parameters
    pub fn from_create_request(id: String, request: &CreateModelRequest) -> Self {
        let now = Utc::now();
        Self {
            id,
            provider: request.provider.clone(),
            name: request.name.clone(),
            api_name: request.api_name.clone(),
            context_window: request.context_window,
            max_output_tokens: request.max_output_tokens,
            temperature_default: request.temperature_default,
            is_builtin: false,
            is_reasoning: request.is_reasoning,
            input_price_per_mtok: request.input_price_per_mtok,
            output_price_per_mtok: request.output_price_per_mtok,
            cache_read_price_per_mtok: request.cache_read_price_per_mtok,
            cache_write_price_per_mtok: request.cache_write_price_per_mtok,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Request payload for creating a new custom model.
///
/// All fields except `temperature_default` and `is_reasoning` are required.
/// The `temperature_default` will default to 0.7 if not provided.
/// The `is_reasoning` will default to false if not provided.
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
    pub temperature_default: f64,
    /// Whether this is a reasoning/thinking model (defaults to false)
    #[serde(default)]
    pub is_reasoning: bool,
    /// Price per million input tokens (USD, defaults to 0.0)
    #[serde(default)]
    pub input_price_per_mtok: f64,
    /// Price per million output tokens (USD, defaults to 0.0)
    #[serde(default)]
    pub output_price_per_mtok: f64,
    /// Price per million cache-read input tokens (USD, defaults to 0.0 = free)
    #[serde(default)]
    pub cache_read_price_per_mtok: f64,
    /// Price per million cache-write input tokens (USD, defaults to 0.0 = same as input)
    #[serde(default)]
    pub cache_write_price_per_mtok: f64,
}

/// Default temperature value for new models.
fn default_temperature() -> f64 {
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

        // Pricing validation
        if self.input_price_per_mtok < 0.0 || self.input_price_per_mtok > 1000.0 {
            return Err("Input price must be between 0 and 1000 USD per million tokens".into());
        }
        if self.output_price_per_mtok < 0.0 || self.output_price_per_mtok > 1000.0 {
            return Err("Output price must be between 0 and 1000 USD per million tokens".into());
        }
        if self.cache_read_price_per_mtok < 0.0 || self.cache_read_price_per_mtok > 1000.0 {
            return Err(
                "Cache read price must be between 0 and 1000 USD per million tokens".into(),
            );
        }
        if self.cache_write_price_per_mtok < 0.0 || self.cache_write_price_per_mtok > 1000.0 {
            return Err(
                "Cache write price must be between 0 and 1000 USD per million tokens".into(),
            );
        }

        Ok(())
    }
}

/// Request payload for updating an existing model.
///
/// All fields are optional. Only provided fields will be updated.
/// For builtin models, only `temperature_default` and `is_reasoning` can be modified.
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
    pub temperature_default: Option<f64>,
    /// Whether this is a reasoning/thinking model
    pub is_reasoning: Option<bool>,
    /// New price per million input tokens (USD)
    pub input_price_per_mtok: Option<f64>,
    /// New price per million output tokens (USD)
    pub output_price_per_mtok: Option<f64>,
    /// New price per million cache-read input tokens (USD)
    pub cache_read_price_per_mtok: Option<f64>,
    /// New price per million cache-write input tokens (USD)
    pub cache_write_price_per_mtok: Option<f64>,
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

        // Pricing validation
        if let Some(price_in) = self.input_price_per_mtok {
            if !(0.0..=1000.0).contains(&price_in) {
                return Err("Input price must be between 0 and 1000 USD per million tokens".into());
            }
        }
        if let Some(price_out) = self.output_price_per_mtok {
            if !(0.0..=1000.0).contains(&price_out) {
                return Err(
                    "Output price must be between 0 and 1000 USD per million tokens".into(),
                );
            }
        }
        if let Some(price_cached) = self.cache_read_price_per_mtok {
            if !(0.0..=1000.0).contains(&price_cached) {
                return Err(
                    "Cache read price must be between 0 and 1000 USD per million tokens".into(),
                );
            }
        }
        if let Some(price_write) = self.cache_write_price_per_mtok {
            if !(0.0..=1000.0).contains(&price_write) {
                return Err(
                    "Cache write price must be between 0 and 1000 USD per million tokens".into(),
                );
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
            && self.is_reasoning.is_none()
            && self.input_price_per_mtok.is_none()
            && self.output_price_per_mtok.is_none()
            && self.cache_read_price_per_mtok.is_none()
            && self.cache_write_price_per_mtok.is_none()
    }
}

/// Configuration settings for a provider.
///
/// Stores per-provider settings including enabled state
/// and optional base URL (primarily for Ollama).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    /// Provider type
    pub provider: ProviderType,
    /// Whether this provider is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether an API key is configured (for Mistral)
    #[serde(default)]
    pub api_key_configured: bool,
    /// Custom base URL (primarily for Ollama, e.g., "http://localhost:11434")
    #[serde(default)]
    pub base_url: Option<String>,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

/// Default value for enabled field (true)
fn default_enabled() -> bool {
    true
}

impl ProviderSettings {
    /// Creates default settings for a provider.
    pub fn default_for(provider: ProviderType) -> Self {
        let base_url = match &provider {
            ProviderType::Ollama => Some(DEFAULT_OLLAMA_URL.into()),
            ProviderType::Mistral => None,
            ProviderType::Custom(_) => None,
        };
        Self {
            provider,
            enabled: true,
            api_key_configured: false,
            base_url,
            updated_at: Utc::now(),
        }
    }
}

/// Result of a provider connection test.
///
/// Contains success status, latency measurement, and any error details.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Returns all builtin models for seeding the database.
/// Currently returns empty - users add their own custom models.
pub fn get_all_builtin_models() -> Vec<LLMModel> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ProviderType Display and FromStr tests are in llm/provider.rs
    // (canonical location after consolidation)

    #[test]
    fn test_create_model_request_validation() {
        let valid = CreateModelRequest {
            provider: ProviderType::Mistral,
            name: "Test Model".into(),
            api_name: "test-model".into(),
            context_window: 32000,
            max_output_tokens: 4096,
            temperature_default: 0.7,
            is_reasoning: false,
            input_price_per_mtok: 2.0,
            output_price_per_mtok: 6.0,
            cache_read_price_per_mtok: 1.0,
            cache_write_price_per_mtok: 2.0,
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
            is_reasoning: None,
            input_price_per_mtok: None,
            output_price_per_mtok: None,
            cache_read_price_per_mtok: None,
            cache_write_price_per_mtok: None,
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
            is_reasoning: None,
            input_price_per_mtok: Some(2.0),
            output_price_per_mtok: Some(6.0),
            cache_read_price_per_mtok: None,
            cache_write_price_per_mtok: None,
        };
        assert!(temp_update.validate(true).is_ok());
    }

    #[test]
    fn test_llm_model_from_create_request() {
        let request = CreateModelRequest {
            provider: ProviderType::Ollama,
            name: "Test Model".into(),
            api_name: "test-model".into(),
            context_window: 32000,
            max_output_tokens: 4096,
            temperature_default: 0.7,
            is_reasoning: false,
            input_price_per_mtok: 0.0,
            output_price_per_mtok: 0.0,
            cache_read_price_per_mtok: 0.0,
            cache_write_price_per_mtok: 0.0,
        };
        let model = LLMModel::from_create_request("test-id".into(), &request);

        assert_eq!(model.id, "test-id");
        assert!(!model.is_builtin);
        assert!(!model.is_reasoning);
        assert_eq!(model.provider, ProviderType::Ollama);
        assert_eq!(model.input_price_per_mtok, 0.0);
        assert_eq!(model.output_price_per_mtok, 0.0);
    }

    #[test]
    fn test_get_all_builtin_models() {
        let models = get_all_builtin_models();
        // No builtin models - users add their own
        assert!(models.is_empty());
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

        let failure =
            ConnectionTestResult::failure(ProviderType::Ollama, "Connection refused".into());
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
        assert_eq!(ollama.base_url, Some("http://localhost:11434".into()));
    }

    /// base_url must always be present in serialized JSON.
    /// When None, it should serialize as `null` (not be absent),
    /// because TS declares `base_url: string | null`.
    #[test]
    fn test_provider_settings_base_url_serializes_as_null_when_none() {
        let settings = ProviderSettings::default_for(ProviderType::Mistral);
        assert!(settings.base_url.is_none());

        let json = serde_json::to_value(&settings).unwrap();
        assert!(
            json.get("base_url").is_some(),
            "base_url must be present in JSON output even when None"
        );
        assert!(
            json.get("base_url").unwrap().is_null(),
            "base_url should be null, not absent"
        );
    }

    /// base_url serializes correctly when set.
    #[test]
    fn test_provider_settings_base_url_serializes_when_set() {
        let settings = ProviderSettings::default_for(ProviderType::Ollama);
        assert!(settings.base_url.is_some());

        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(
            json.get("base_url").unwrap().as_str().unwrap(),
            "http://localhost:11434"
        );
    }

    /// base_url roundtrip (serialize then deserialize).
    #[test]
    fn test_provider_settings_base_url_roundtrip() {
        // With base_url = None (Mistral)
        let original = ProviderSettings::default_for(ProviderType::Mistral);
        let json_str = serde_json::to_string(&original).unwrap();
        let restored: ProviderSettings = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.base_url, None);

        // With base_url = Some (Ollama)
        let original = ProviderSettings::default_for(ProviderType::Ollama);
        let json_str = serde_json::to_string(&original).unwrap();
        let restored: ProviderSettings = serde_json::from_str(&json_str).unwrap();
        assert_eq!(restored.base_url, Some("http://localhost:11434".into()));
    }
}
