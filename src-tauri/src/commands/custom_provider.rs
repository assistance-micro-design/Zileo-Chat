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

//! Custom provider CRUD commands.
//!
//! Manages user-created OpenAI-compatible providers (RouterLab, OpenRouter, etc.).

use crate::commands::SecureKeyStore;
use crate::llm::openai_compatible::OpenAiCompatibleProvider;
use crate::models::custom_provider::{CustomProvider, ProviderInfo};
use crate::state::AppState;
use std::sync::Arc;
use tauri::State;
use tracing::{info, instrument, warn};

/// Validates a provider name (URL-safe: lowercase alphanumeric + hyphens).
fn validate_provider_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 64 {
        return Err("Provider name must be 1-64 characters".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(
            "Provider name must contain only lowercase letters, numbers, and hyphens".into(),
        );
    }
    if name == "mistral" || name == "ollama" {
        return Err(format!("'{}' is a builtin provider name", name));
    }
    Ok(())
}

/// Lists all providers (builtin + custom).
///
/// Returns a unified list of provider metadata for the frontend.
#[tauri::command]
#[instrument(name = "list_providers", skip(state))]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, String> {
    let db = &state.db;

    // Builtin providers
    let mut providers = vec![
        ProviderInfo {
            id: "mistral".to_string(),
            display_name: "Mistral".to_string(),
            is_builtin: true,
            is_cloud: true,
            requires_api_key: true,
            has_base_url: false,
            base_url: None,
            enabled: true,
        },
        ProviderInfo {
            id: "ollama".to_string(),
            display_name: "Ollama".to_string(),
            is_builtin: true,
            is_cloud: false,
            requires_api_key: false,
            has_base_url: true,
            base_url: Some("http://localhost:11434".to_string()),
            enabled: true,
        },
    ];

    // Custom providers from DB
    let query = "SELECT name, display_name, base_url, enabled, \
                 time::format(created_at, '%Y-%m-%dT%H:%M:%SZ') AS created_at \
                 FROM custom_provider ORDER BY name";
    let custom_providers: Vec<CustomProvider> = db
        .query_json(query)
        .await
        .map_err(|e| format!("Failed to list custom providers: {}", e))?
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    for cp in custom_providers {
        providers.push(ProviderInfo {
            id: cp.name.clone(),
            display_name: cp.display_name,
            is_builtin: false,
            is_cloud: true,
            requires_api_key: true,
            has_base_url: true,
            base_url: Some(cp.base_url),
            enabled: cp.enabled,
        });
    }

    info!(count = providers.len(), "Listed providers");
    Ok(providers)
}

/// Creates a new custom provider.
///
/// Stores metadata in DB, API key in SecureKeyStore, and registers
/// the provider in ProviderManager's HashMap.
#[tauri::command]
#[instrument(name = "create_custom_provider", skip(state, keystore, api_key))]
pub async fn create_custom_provider(
    name: String,
    display_name: String,
    base_url: String,
    api_key: String,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ProviderInfo, String> {
    // Validate inputs
    validate_provider_name(&name)?;

    if display_name.trim().is_empty() || display_name.len() > 128 {
        return Err("Display name must be 1-128 characters".into());
    }
    if base_url.trim().is_empty() || base_url.len() > 512 {
        return Err("Base URL must be 1-512 characters".into());
    }
    if api_key.trim().is_empty() {
        return Err("API key is required".into());
    }

    let db = &state.db;

    // Check uniqueness (parameterized to avoid injection per ERR_SURREAL_004)
    let existing: Vec<serde_json::Value> = db
        .query_json_with_params(
            "SELECT name FROM custom_provider WHERE name = $name",
            vec![("name".to_string(), serde_json::json!(name))],
        )
        .await
        .map_err(|e| format!("Failed to check provider existence: {}", e))?;
    if !existing.is_empty() {
        return Err(format!("Provider '{}' already exists", name));
    }

    // Normalize base URL
    let normalized_url = base_url.trim_end_matches('/').to_string();

    // Insert into DB
    let insert_query = format!("CREATE custom_provider:`{}` CONTENT $data", name);
    let data = serde_json::json!({
        "name": name,
        "display_name": display_name,
        "base_url": normalized_url,
        "enabled": true
    });
    db.execute_with_params(&insert_query, vec![("data".to_string(), data)])
        .await
        .map_err(|e| format!("Failed to create custom provider: {}", e))?;

    // Store API key in SecureKeyStore
    keystore
        .set_key(&name, &api_key)
        .map_err(|e| format!("Failed to store API key: {}", e))?;

    // Create and register provider in manager
    let provider = Arc::new(OpenAiCompatibleProvider::new(
        &name,
        state.llm_manager.http_client().clone(),
    ));
    if let Err(e) = provider.configure(&api_key, &normalized_url).await {
        warn!(name = %name, error = %e, "Failed to configure new custom provider");
    }
    state
        .llm_manager
        .register_custom_provider(&name, provider)
        .await;

    info!(name = %name, display_name = %display_name, "Custom provider created");

    Ok(ProviderInfo {
        id: name,
        display_name,
        is_builtin: false,
        is_cloud: true,
        requires_api_key: true,
        has_base_url: true,
        base_url: Some(normalized_url),
        enabled: true,
    })
}

/// Updates an existing custom provider.
#[tauri::command]
#[instrument(name = "update_custom_provider", skip(state, keystore, api_key))]
pub async fn update_custom_provider(
    name: String,
    display_name: Option<String>,
    base_url: Option<String>,
    api_key: Option<String>,
    enabled: Option<bool>,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ProviderInfo, String> {
    validate_provider_name(&name)?;

    let db = &state.db;

    // Build SET clauses dynamically
    let mut set_parts: Vec<String> = Vec::new();

    if let Some(ref dn) = display_name {
        if dn.trim().is_empty() || dn.len() > 128 {
            return Err("Display name must be 1-128 characters".into());
        }
        set_parts.push(format!(
            "display_name = {}",
            serde_json::to_string(dn).map_err(|e| e.to_string())?
        ));
    }
    if let Some(ref url) = base_url {
        if url.trim().is_empty() || url.len() > 512 {
            return Err("Base URL must be 1-512 characters".into());
        }
        let normalized = url.trim_end_matches('/');
        set_parts.push(format!(
            "base_url = {}",
            serde_json::to_string(normalized).map_err(|e| e.to_string())?
        ));
    }
    if let Some(en) = enabled {
        set_parts.push(format!("enabled = {}", en));
    }

    if !set_parts.is_empty() {
        set_parts.push("updated_at = time::now()".to_string());
        let update_query = format!(
            "UPDATE custom_provider:`{}` SET {}",
            name,
            set_parts.join(", ")
        );
        db.execute(&update_query)
            .await
            .map_err(|e| format!("Failed to update custom provider: {}", e))?;
    }

    // Update API key if provided
    if let Some(ref key) = api_key {
        if key.trim().is_empty() {
            return Err("API key cannot be empty".into());
        }
        keystore
            .set_key(&name, key)
            .map_err(|e| format!("Failed to update API key: {}", e))?;
    }

    // Reconfigure provider in manager if URL or key changed
    if api_key.is_some() || base_url.is_some() {
        if let Some(provider) = state.llm_manager.get_custom_provider(&name).await {
            let current_key = if let Some(ref key) = api_key {
                key.clone()
            } else {
                keystore.get_key(&name).unwrap_or_default()
            };
            let current_url = if let Some(ref url) = base_url {
                url.trim_end_matches('/').to_string()
            } else {
                provider.get_base_url().await.unwrap_or_default()
            };
            if let Err(e) = provider.configure(&current_key, &current_url).await {
                warn!(name = %name, error = %e, "Failed to reconfigure custom provider");
            }
        }
    }

    // Read back updated provider
    let read_query = format!(
        "SELECT name, display_name, base_url, enabled FROM custom_provider:`{}`",
        name
    );
    let results: Vec<serde_json::Value> = db
        .query_json(&read_query)
        .await
        .map_err(|e| format!("Failed to read updated provider: {}", e))?;
    let cp: CustomProvider = results
        .into_iter()
        .next()
        .and_then(|v| serde_json::from_value(v).ok())
        .ok_or_else(|| format!("Provider '{}' not found after update", name))?;

    info!(name = %name, "Custom provider updated");

    Ok(ProviderInfo {
        id: cp.name,
        display_name: cp.display_name,
        is_builtin: false,
        is_cloud: true,
        requires_api_key: true,
        has_base_url: true,
        base_url: Some(cp.base_url),
        enabled: cp.enabled,
    })
}

/// Deletes a custom provider.
#[tauri::command]
#[instrument(name = "delete_custom_provider", skip(state, keystore))]
pub async fn delete_custom_provider(
    name: String,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    validate_provider_name(&name)?;

    let db = &state.db;

    // Delete from DB
    let delete_query = format!("DELETE custom_provider:`{}`", name);
    db.execute(&delete_query)
        .await
        .map_err(|e| format!("Failed to delete custom provider: {}", e))?;

    // Remove API key from SecureKeyStore
    if let Err(e) = keystore.delete_key(&name) {
        warn!(name = %name, error = %e, "Failed to delete API key (may not exist)");
    }

    // Unregister from manager
    state.llm_manager.unregister_custom_provider(&name).await;

    info!(name = %name, "Custom provider deleted");
    Ok(())
}
