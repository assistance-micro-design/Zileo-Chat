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

//! Embedding configuration commands.
//!
//! Tauri commands for managing embedding settings, provider initialization,
//! and testing embedding generation.

use crate::{
    commands::SecureKeyStore,
    db::DBClient,
    llm::embedding::{EmbeddingProvider, EmbeddingService},
    llm::DEFAULT_OLLAMA_URL,
    models::{EmbeddingConfigSettings, EmbeddingTestResult},
    security::serialize_for_query,
    AppState,
};
use std::sync::Arc;
use std::time::Instant;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Storage key for embedding configuration in the database
pub const EMBEDDING_CONFIG_KEY: &str = "settings:embedding_config";

/// Loads the Ollama base URL from `provider_settings:ollama`, falling back to
/// `DEFAULT_OLLAMA_URL` when the row is missing, the field is `NONE`, or the
/// query itself fails (the embedding service can still work on default URL).
///
/// Centralized here so both [`update_embedding_service_internal`] and
/// `AppState::initialize_embedding_from_config` resolve the URL the same way
/// and the previous hardcoded `"http://localhost:11434"` is replaced.
pub async fn load_ollama_base_url(db: &DBClient) -> String {
    let query = "SELECT base_url FROM provider_settings:ollama";
    match db.query_json(query).await {
        Ok(rows) => rows
            .first()
            .and_then(|r| r.get("base_url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| DEFAULT_OLLAMA_URL.to_string()),
        Err(e) => {
            warn!(
                error = %e,
                "Failed to load Ollama base_url from provider_settings, falling back to default"
            );
            DEFAULT_OLLAMA_URL.to_string()
        }
    }
}

/// Gets the current embedding configuration.
///
/// Returns the stored configuration, or `None` when the row is absent. The
/// previous behavior of silently returning defaults masked the "no config"
/// state from the frontend (`configExists` was always true). Returning
/// `Option` lets the UI distinguish unconfigured from configured.
#[tauri::command]
#[instrument(name = "get_embedding_config", skip(state))]
pub async fn get_embedding_config(
    state: State<'_, AppState>,
) -> Result<Option<EmbeddingConfigSettings>, String> {
    info!("Getting embedding configuration");

    let query = format!("SELECT config FROM settings:`{}`", EMBEDDING_CONFIG_KEY);

    let results: Vec<serde_json::Value> = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to query embedding config");
        format!("Failed to load embedding config: {}", e)
    })?;

    if let Some(row) = results.first() {
        if let Some(config_value) = row.get("config") {
            if let Ok(config) =
                serde_json::from_value::<EmbeddingConfigSettings>(config_value.clone())
            {
                info!("Loaded embedding config from database");
                return Ok(Some(config));
            }
        }
    }

    info!("No stored config found");
    Ok(None)
}

/// Saves the embedding configuration.
///
/// # Arguments
/// * `config` - The embedding configuration to save
#[tauri::command]
#[instrument(name = "save_embedding_config", skip(state, keystore, config))]
pub async fn save_embedding_config(
    config: EmbeddingConfigSettings,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    info!(
        provider = %config.provider,
        model = %config.model,
        "Saving embedding configuration"
    );

    // Validate configuration
    if config.provider.is_empty() {
        return Err("Provider cannot be empty".to_string());
    }
    if config.model.is_empty() {
        return Err("Model cannot be empty".to_string());
    }

    let config_json_str = serialize_for_query(&config, "config")?;

    let upsert_query = format!(
        "UPSERT settings:`{}` CONTENT {{ id: '{}', config: {} }}",
        EMBEDDING_CONFIG_KEY, EMBEDDING_CONFIG_KEY, config_json_str
    );

    state.db.execute(&upsert_query).await.map_err(|e| {
        error!(error = %e, "Failed to save embedding config");
        format!("Failed to save config: {}", e)
    })?;

    // Update the EmbeddingService in AppState
    update_embedding_service_internal(&config, &state, &keystore).await;

    info!("Embedding configuration saved successfully");
    Ok(())
}

/// Deletes the embedding configuration row and clears the in-memory service.
///
/// After this call, [`get_embedding_config`] returns `None` and any future
/// search falls back to text mode. Idempotent: deleting an absent row is a
/// no-op. The Mistral API key in the OS keychain is left untouched — it is
/// owned by the provider settings UI, not the memory settings.
#[tauri::command]
#[instrument(name = "delete_embedding_config", skip(state))]
pub async fn delete_embedding_config(state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting embedding configuration");

    let delete_query = format!("DELETE settings:`{}`", EMBEDDING_CONFIG_KEY);
    state.db.execute(&delete_query).await.map_err(|e| {
        error!(error = %e, "Failed to delete embedding config row");
        format!("Failed to delete config: {}", e)
    })?;

    // Reset the in-memory service so subsequent searches fall back to text.
    *state.embedding_service.write().await = None;

    info!("Embedding configuration deleted successfully");
    Ok(())
}

/// Updates the EmbeddingService in AppState based on config.
/// Note: For Mistral, requires API key to be pre-configured in Provider settings (OS keychain).
async fn update_embedding_service_internal(
    config: &EmbeddingConfigSettings,
    state: &State<'_, AppState>,
    keystore: &State<'_, SecureKeyStore>,
) {
    let provider = match config.provider.as_str() {
        "ollama" => {
            let base_url = load_ollama_base_url(&state.db).await;
            Some(EmbeddingProvider::ollama_with_config(
                &base_url,
                &config.model,
            ))
        }
        "mistral" => {
            if let Some(api_key) = keystore.get_key("Mistral") {
                Some(EmbeddingProvider::mistral_with_model(
                    &api_key,
                    &config.model,
                ))
            } else {
                warn!(
                    "Mistral API key not available - please configure in Provider settings first"
                );
                None
            }
        }
        _ => {
            warn!(provider = %config.provider, "Unknown embedding provider");
            None
        }
    };

    if let Some(provider) = provider {
        match EmbeddingService::with_provider(provider) {
            Ok(service) => {
                let mut guard = state.embedding_service.write().await;
                *guard = Some(Arc::new(service));
                info!("Embedding service updated successfully");
            }
            Err(e) => {
                error!("Failed to create embedding service: {}", e);
            }
        }
    }
}

/// Helper function to get config internally.
///
/// Returns `Ok(None)` when no row exists (no implicit default) — callers
/// decide whether to fall back or surface the missing-config state.
pub async fn get_embedding_config_internal(
    state: &State<'_, AppState>,
) -> Result<Option<EmbeddingConfigSettings>, String> {
    let query = format!("SELECT config FROM settings:`{}`", EMBEDDING_CONFIG_KEY);

    let results: Vec<serde_json::Value> = state
        .db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?;

    if let Some(row) = results.first() {
        if let Some(config) = row.get("config") {
            let parsed = serde_json::from_value(config.clone())
                .map_err(|e| format!("Failed to parse config: {}", e))?;
            return Ok(Some(parsed));
        }
    }

    Ok(None)
}

/// Reinitializes the embedding service with current config
#[tauri::command]
#[instrument(name = "reinit_embedding_service", skip(state, keystore))]
pub async fn reinit_embedding_service(
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    info!("Reinitializing embedding service");
    match get_embedding_config_internal(&state).await? {
        Some(config) => {
            update_embedding_service_internal(&config, &state, &keystore).await;
        }
        None => {
            *state.embedding_service.write().await = None;
            info!("No embedding config stored, embedding service cleared");
        }
    }
    Ok(())
}

/// Tests embedding generation with current configuration
#[tauri::command]
#[instrument(name = "test_embedding", skip(state))]
pub async fn test_embedding(
    state: State<'_, AppState>,
    text: String,
) -> Result<EmbeddingTestResult, String> {
    info!(text_len = text.len(), "Testing embedding generation");

    if text.is_empty() {
        return Err("Test text cannot be empty".to_string());
    }

    if text.len() > 10000 {
        return Err("Test text too long (max 10000 chars)".to_string());
    }

    let embed_service = state.embedding_service.read().await;
    let service = embed_service.as_ref().ok_or_else(|| {
        "Embedding service not configured. Please save embedding settings first.".to_string()
    })?;

    let start = Instant::now();

    match service.embed(&text).await {
        Ok(embedding) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let dimension = embedding.len();
            let preview: Vec<f32> = embedding.iter().take(5).cloned().collect();

            let (provider, model) = config_or_unknown(&state).await;

            info!(
                dimension = dimension,
                duration_ms = duration_ms,
                "Embedding test successful"
            );

            Ok(EmbeddingTestResult {
                success: true,
                dimension,
                preview,
                duration_ms,
                provider,
                model,
                error: None,
            })
        }
        Err(e) => {
            let (provider, model) = config_or_unknown(&state).await;

            warn!(error = %e, "Embedding test failed");

            Ok(EmbeddingTestResult {
                success: false,
                dimension: 0,
                preview: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
                provider,
                model,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Returns the current provider/model pair from the stored config or
/// `("unknown", "unknown")` when no row exists or it fails to parse.
/// Used only to enrich [`EmbeddingTestResult`] for the UI.
async fn config_or_unknown(state: &State<'_, AppState>) -> (String, String) {
    match get_embedding_config_internal(state).await {
        Ok(Some(c)) => (c.provider, c.model),
        _ => ("unknown".to_string(), "unknown".to_string()),
    }
}
