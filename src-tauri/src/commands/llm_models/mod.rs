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

//! LLM Model CRUD Commands
//!
//! Provides Tauri commands for managing LLM models and provider settings.
//!
//! ## Model Commands
//! - `list_models` - List all models (optionally filtered by provider)
//! - `get_model` - Get a single model by ID
//! - `create_model` - Create a new custom model
//! - `update_model` - Update an existing model
//! - `delete_model` - Delete a custom model (builtin models cannot be deleted)
//!
//! ## Provider Settings Commands
//! - `get_provider_settings` - Get settings for a provider
//! - `update_provider_settings` - Update provider settings
//!
//! ## Connection Commands
//! - `test_provider_connection` - Test connection to a provider
//!
//! ## Seed Commands
//! - `seed_builtin_models` - Seed the database with builtin models

// Submodules must be pub so Tauri's #[tauri::command] macro-generated items
// (__cmd__*) are visible through the re-exports.
pub mod connection;
pub mod crud;
pub mod provider_settings;
pub mod seed;

#[cfg(test)]
mod tests;

use crate::constants::commands as cmd_const;
use crate::llm::ProviderType;

/// Validates a model ID string.
///
/// Ensures the ID is non-empty, within length limits, and contains only safe
/// characters (alphanumeric, hyphens, underscores, dots) to prevent SurrealQL
/// injection when used in record ID positions.
pub(crate) fn validate_model_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("Model ID is required".into());
    }
    if id.len() > cmd_const::MAX_MODEL_ID_LEN {
        return Err(format!(
            "Model ID must be {} characters or less",
            cmd_const::MAX_MODEL_ID_LEN
        ));
    }
    // Strict character check: only allow chars safe for SurrealQL record IDs
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("Model ID contains invalid characters (only alphanumeric, hyphens, underscores, and dots allowed)".into());
    }
    Ok(())
}

/// Validates a provider string.
pub(crate) fn validate_provider_string(provider: &str) -> Result<ProviderType, String> {
    provider
        .parse::<ProviderType>()
        .map_err(|_| format!("Invalid provider '{}'", provider))
}
