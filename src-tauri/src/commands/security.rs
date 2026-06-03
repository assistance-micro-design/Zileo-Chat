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

//! Security-related Tauri commands for API key management.
//!
//! Provides secure storage and status checks for LLM provider API keys
//! using OS keychain + AES-256-GCM encryption. Secrets are never returned
//! through Tauri IPC.

use crate::llm::ProviderManager;
use crate::security::{KeyStore, KeyStoreError, Validator};
use crate::state::AppState;
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

    /// Checks if an API key exists for a provider.
    pub fn has_key(&self, provider: &str) -> bool {
        self.inner.get(provider).is_ok()
    }

    /// Gets the API key for a provider, if it exists.
    pub fn get_key(&self, provider: &str) -> Option<String> {
        self.inner.get(provider).ok()
    }

    /// Saves an API key for a provider.
    pub fn set_key(&self, provider: &str, api_key: &str) -> Result<(), KeyStoreError> {
        self.inner.save(provider, api_key)
    }

    /// Deletes an API key for a provider.
    pub fn delete_key(&self, provider: &str) -> Result<(), KeyStoreError> {
        self.inner.delete(provider)
    }

    /// Returns a reference to the underlying [`KeyStore`].
    ///
    /// Used by modules that need the lower-level `save / get / delete`
    /// API (e.g. `mcp::secrets` for per-server MCP secret storage in
    /// the same keyring service).
    pub fn inner(&self) -> &KeyStore {
        &self.inner
    }
}

/// Maps a validated provider identifier to its canonical keystore key.
///
/// The frontend sends provider ids in their lowercase product form
/// (`"mistral"`, `"ollama"`, custom ids verbatim), but every backend read
/// site for built-in providers — boot init (`state.rs`), STT
/// (`commands/stt.rs`), the connection test, embedding config and provider
/// settings — looks the key up under the capitalized display form
/// (`"Mistral"`, `"Ollama"`). On case-sensitive OS keychains those are
/// distinct entries, so a fresh save under `"mistral"` could never be found
/// again. Normalizing built-ins to their display form keeps the write path
/// in sync with the read path and stays backward compatible with keys
/// already stored under `"Mistral"`/`"Ollama"`. Custom providers keep their
/// identifier verbatim, matching how the `custom_provider` commands store
/// and read them.
fn canonical_keystore_key(validated_provider: &str) -> String {
    match validated_provider.to_lowercase().as_str() {
        "mistral" => "Mistral".to_string(),
        "ollama" => "Ollama".to_string(),
        _ => validated_provider.to_string(),
    }
}

/// Applies a freshly saved API key to the running provider so the change
/// takes effect without restarting the app.
///
/// Without this, `save_api_key` only writes to the keystore while the
/// `ProviderManager` keeps its boot-time state, so a key entered during
/// onboarding (or edited in settings) is ignored until the next launch
/// (the agent loop reports the provider as "not configured"). Best-effort:
/// failures are logged, never surfaced as a save error, because the key is
/// already persisted and will be picked up on the next boot.
async fn apply_key_to_runtime(manager: &ProviderManager, provider: &str, api_key: &str) {
    match provider.to_lowercase().as_str() {
        "mistral" => {
            if let Err(e) = manager.configure_mistral(api_key).await {
                warn!(error = %e, "Failed to apply Mistral key to runtime after save");
            } else {
                info!("Mistral provider reconfigured from newly saved key");
            }
        }
        // Ollama is keyless (configured by URL); nothing to apply here.
        "ollama" => {}
        // Custom providers reconfigure in place if already registered.
        _ => {
            if let Some(p) = manager.get_custom_provider(provider).await {
                let base_url = p.get_base_url().await.unwrap_or_default();
                if let Err(e) = p.configure(api_key, &base_url).await {
                    warn!(
                        provider = %provider,
                        error = %e,
                        "Failed to reconfigure custom provider after key save"
                    );
                }
            }
        }
    }
}

/// Saves an API key for a provider.
///
/// The key is validated, encrypted with AES-256-GCM, and stored in the OS keychain.
#[tauri::command]
#[instrument(name = "save_api_key", skip(api_key, state, keystore), fields(provider = %provider))]
pub async fn save_api_key(
    provider: String,
    api_key: String,
    state: State<'_, AppState>,
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

    // Save to keystore under the canonical key so the read paths find it.
    let keystore_key = canonical_keystore_key(&validated_provider);
    keystore.inner.save(&keystore_key, &api_key).map_err(|e| {
        error!(error = %e, "Failed to save API key");
        format!("Failed to save API key: {}", e)
    })?;

    // Apply the key to the running provider so it works without a restart.
    apply_key_to_runtime(&state.llm_manager, &validated_provider, &api_key).await;

    info!("API key saved successfully");
    Ok(())
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

    // Delete from keystore under the same canonical key used to save.
    let keystore_key = canonical_keystore_key(&validated_provider);
    keystore.inner.delete(&keystore_key).map_err(|e| {
        // Normalized error message to prevent provider enumeration
        warn!("API key operation failed for provider");
        match &e {
            KeyStoreError::NotFound(_) => "API key not found".to_string(),
            _ => "API key operation failed".to_string(),
        }
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

    let keystore_key = canonical_keystore_key(&validated_provider);
    let exists = keystore.inner.exists(&keystore_key);
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
    use crate::llm::ProviderType;

    #[test]
    fn canonical_keystore_key_normalizes_builtins_to_display_form() {
        // The frontend sends lowercase ids; built-ins must resolve to the
        // capitalized form used by every backend read site.
        assert_eq!(canonical_keystore_key("mistral"), "Mistral");
        assert_eq!(canonical_keystore_key("ollama"), "Ollama");
    }

    #[test]
    fn canonical_keystore_key_is_idempotent_on_display_form() {
        // Keys already stored under the display form (pre-existing installs)
        // must keep resolving to the same slot — backward compatibility.
        assert_eq!(canonical_keystore_key("Mistral"), "Mistral");
        assert_eq!(canonical_keystore_key("Ollama"), "Ollama");
    }

    #[test]
    fn canonical_keystore_key_keeps_custom_providers_verbatim() {
        // Custom providers store/read under their exact id; never rewrite.
        assert_eq!(canonical_keystore_key("routerlab"), "routerlab");
        assert_eq!(canonical_keystore_key("openrouter"), "openrouter");
    }

    #[tokio::test]
    async fn apply_key_to_runtime_configures_mistral_without_restart() {
        let manager = ProviderManager::new().expect("test provider manager");
        assert!(!manager.is_provider_configured(ProviderType::Mistral));

        apply_key_to_runtime(&manager, "mistral", "test-api-key-1234567890").await;

        assert!(manager.is_provider_configured(ProviderType::Mistral));
    }

    #[tokio::test]
    async fn apply_key_to_runtime_is_noop_for_keyless_ollama() {
        // Ollama is configured by URL, not key: applying a key must not error
        // nor accidentally configure another provider.
        let manager = ProviderManager::new().expect("test provider manager");
        apply_key_to_runtime(&manager, "ollama", "irrelevant-key-1234567890").await;
        assert!(!manager.is_provider_configured(ProviderType::Mistral));
    }
}
