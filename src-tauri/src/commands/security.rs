// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Security-related Tauri commands for API key management.
//!
//! Provides secure storage and retrieval of API keys for LLM providers
//! using OS keychain + AES-256-GCM encryption.

#[cfg(test)]
use crate::security::ValidationError;
use crate::security::{KeyStore, KeyStoreError, Validator};
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Thread-safe wrapper for KeyStore
pub struct SecureKeyStore {
    inner: KeyStore,
}

impl SecureKeyStore {
    /// Creates a new SecureKeyStore instance.
    pub fn new() -> Result<Self, KeyStoreError> {
        Ok(Self {
            inner: KeyStore::new()?,
        })
    }

    /// Creates a SecureKeyStore without encryption (for testing).
    pub fn new_without_encryption() -> Self {
        Self {
            inner: KeyStore::new_without_encryption(),
        }
    }
}

impl Default for SecureKeyStore {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self::new_without_encryption())
    }
}

/// Saves an API key for a provider.
///
/// The key is validated, encrypted with AES-256-GCM, and stored in the OS keychain.
#[tauri::command]
#[instrument(name = "save_api_key", skip(api_key, keystore), fields(provider = %provider))]
pub async fn save_api_key(
    provider: String,
    api_key: String,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    info!("Saving API key");

    // Validate provider
    let validated_provider = Validator::validate_provider(&provider).map_err(|e| {
        warn!(error = %e, "Invalid provider name");
        format!("Invalid provider: {}", e)
    })?;

    // Validate API key
    Validator::validate_api_key(&api_key).map_err(|e| {
        warn!(error = %e, "Invalid API key format");
        format!("Invalid API key: {}", e)
    })?;

    // Save to keystore
    keystore
        .inner
        .save(&validated_provider, &api_key)
        .map_err(|e| {
            error!(error = %e, "Failed to save API key");
            format!("Failed to save API key: {}", e)
        })?;

    info!("API key saved successfully");
    Ok(())
}

/// Retrieves an API key for a provider.
///
/// Returns the decrypted API key if found.
#[tauri::command]
#[instrument(name = "get_api_key", skip(keystore), fields(provider = %provider))]
pub async fn get_api_key(
    provider: String,
    keystore: State<'_, SecureKeyStore>,
) -> Result<String, String> {
    info!("Retrieving API key");

    // Validate provider
    let validated_provider = Validator::validate_provider(&provider).map_err(|e| {
        warn!(error = %e, "Invalid provider name");
        format!("Invalid provider: {}", e)
    })?;

    // Get from keystore
    let api_key = keystore.inner.get(&validated_provider).map_err(|e| {
        match &e {
            KeyStoreError::NotFound(_) => {
                warn!(provider = %validated_provider, "API key not found");
            }
            _ => {
                error!(error = %e, "Failed to retrieve API key");
            }
        }
        format!("Failed to retrieve API key: {}", e)
    })?;

    info!("API key retrieved successfully");
    Ok(api_key)
}

/// Deletes an API key for a provider.
#[tauri::command]
#[instrument(name = "delete_api_key", skip(keystore), fields(provider = %provider))]
pub async fn delete_api_key(
    provider: String,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    info!("Deleting API key");

    // Validate provider
    let validated_provider = Validator::validate_provider(&provider).map_err(|e| {
        warn!(error = %e, "Invalid provider name");
        format!("Invalid provider: {}", e)
    })?;

    // Delete from keystore
    keystore.inner.delete(&validated_provider).map_err(|e| {
        match &e {
            KeyStoreError::NotFound(_) => {
                warn!(provider = %validated_provider, "API key not found for deletion");
            }
            _ => {
                error!(error = %e, "Failed to delete API key");
            }
        }
        format!("Failed to delete API key: {}", e)
    })?;

    info!("API key deleted successfully");
    Ok(())
}

/// Checks if an API key exists for a provider.
#[tauri::command]
#[instrument(name = "has_api_key", skip(keystore), fields(provider = %provider))]
pub async fn has_api_key(
    provider: String,
    keystore: State<'_, SecureKeyStore>,
) -> Result<bool, String> {
    // Validate provider
    let validated_provider = Validator::validate_provider(&provider).map_err(|e| {
        warn!(error = %e, "Invalid provider name");
        format!("Invalid provider: {}", e)
    })?;

    let exists = keystore.inner.exists(&validated_provider);
    info!(exists = exists, "API key existence checked");
    Ok(exists)
}

/// Lists all providers that have stored API keys.
#[tauri::command]
#[instrument(name = "list_api_key_providers", skip(keystore))]
pub async fn list_api_key_providers(
    keystore: State<'_, SecureKeyStore>,
) -> Result<Vec<String>, String> {
    info!("Listing API key providers");
    let providers = keystore.inner.list_providers();
    info!(count = providers.len(), "API key providers listed");
    Ok(providers)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Converts internal validation errors to user-friendly messages (test helper).
    fn validation_error_to_string(e: ValidationError) -> String {
        match e {
            ValidationError::Empty { field } => format!("{} cannot be empty", field),
            ValidationError::TooLong { max, actual } => {
                format!("Input too long: maximum {} characters, got {}", max, actual)
            }
            ValidationError::TooShort { min, actual } => {
                format!(
                    "Input too short: minimum {} characters, got {}",
                    min, actual
                )
            }
            ValidationError::InvalidCharacters { details } => {
                format!("Invalid characters: {}", details)
            }
            ValidationError::InvalidFormat { field, details } => {
                format!("Invalid {}: {}", field, details)
            }
            ValidationError::InvalidUuid { value } => {
                format!("Invalid ID format: {}", value)
            }
        }
    }

    #[test]
    fn test_secure_keystore_creation() {
        // Should not panic - just verify creation works
        let _store = SecureKeyStore::new_without_encryption();
    }

    #[test]
    fn test_validation_error_to_string_empty() {
        let e = ValidationError::Empty {
            field: "provider".to_string(),
        };
        let msg = validation_error_to_string(e);
        assert!(msg.contains("provider"));
        assert!(msg.contains("empty"));
    }

    #[test]
    fn test_validation_error_to_string_too_long() {
        let e = ValidationError::TooLong {
            max: 100,
            actual: 150,
        };
        let msg = validation_error_to_string(e);
        assert!(msg.contains("100"));
        assert!(msg.contains("150"));
    }

    #[test]
    fn test_validation_error_to_string_too_short() {
        let e = ValidationError::TooShort { min: 16, actual: 5 };
        let msg = validation_error_to_string(e);
        assert!(msg.contains("16"));
        assert!(msg.contains("5"));
    }

    #[test]
    fn test_validation_error_to_string_invalid_chars() {
        let e = ValidationError::InvalidCharacters {
            details: "no spaces allowed".to_string(),
        };
        let msg = validation_error_to_string(e);
        assert!(msg.contains("no spaces allowed"));
    }
}
