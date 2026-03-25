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

//! Provider settings commands: get and update provider configuration.

use tauri::State;
use tracing::{error, info, instrument};

use super::crud::get_model;
use super::validate_provider_string;
use crate::commands::security::SecureKeyStore;
use crate::llm::ProviderType;
use crate::models::llm_models::ProviderSettings;
use crate::state::AppState;

/// Gets settings for a provider.
///
/// If no settings exist, returns default settings for the provider.
#[tauri::command]
#[instrument(name = "get_provider_settings", skip(state, keystore), fields(provider = %provider))]
pub async fn get_provider_settings(
    provider: String,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ProviderSettings, String> {
    let provider_type = validate_provider_string(&provider)?;

    info!("Getting provider settings");

    let query = format!(
        "SELECT provider, enabled, default_model_id, base_url, updated_at \
         FROM provider_settings:`{}`",
        provider_type
    );

    let result: Option<ProviderSettings> = state
        .db
        .db
        .query(&query)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to query provider settings");
            format!("Failed to query provider settings: {}", e)
        })?
        .take(0)
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize settings");
            format!("Failed to deserialize settings: {}", e)
        })?;

    info!(found = result.is_some(), "Provider settings query result");

    let api_key_configured = match &provider_type {
        ProviderType::Mistral => keystore.has_key("Mistral"),
        ProviderType::Custom(name) => keystore.has_key(name),
        ProviderType::Ollama => false,
    };

    match result {
        Some(mut settings) => {
            settings.api_key_configured = api_key_configured;
            Ok(settings)
        }
        None => {
            let mut default = ProviderSettings::default_for(provider_type);
            default.api_key_configured = api_key_configured;
            Ok(default)
        }
    }
}

/// Updates settings for a provider.
///
/// Creates settings if they don't exist (upsert behavior).
#[tauri::command]
#[instrument(name = "update_provider_settings", skip(state, keystore), fields(provider = %provider))]
pub async fn update_provider_settings(
    provider: String,
    enabled: Option<bool>,
    default_model_id: Option<String>,
    base_url: Option<String>,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ProviderSettings, String> {
    let provider_type = validate_provider_string(&provider)?;

    info!(
        enabled = ?enabled,
        default_model_id = ?default_model_id,
        base_url = ?base_url,
        "Updating provider settings - received params"
    );

    // Validate default_model_id exists if provided
    if let Some(ref model_id) = default_model_id {
        let model = get_model(model_id.clone(), state.clone()).await?;
        if model.provider != provider_type {
            return Err(format!(
                "Model {} belongs to provider {}, not {}",
                model_id, model.provider, provider_type
            ));
        }
    }

    let mut set_parts: Vec<String> = vec![
        "provider = $p_provider".to_string(),
        "updated_at = time::now()".to_string(),
    ];
    let mut params: Vec<(String, serde_json::Value)> = vec![(
        "p_provider".to_string(),
        serde_json::json!(provider_type.as_id()),
    )];

    if let Some(en) = enabled {
        set_parts.push(format!("enabled = {}", en));
    } else {
        set_parts.push("enabled = enabled ?? true".to_string());
    }

    if let Some(ref model_id) = default_model_id {
        set_parts.push("default_model_id = $p_model_id".to_string());
        params.push(("p_model_id".to_string(), serde_json::json!(model_id)));
    } else {
        set_parts.push("default_model_id = default_model_id".to_string());
    }

    if let Some(ref url) = base_url {
        set_parts.push("base_url = $p_base_url".to_string());
        params.push(("p_base_url".to_string(), serde_json::json!(url)));
    } else {
        set_parts.push("base_url = base_url".to_string());
    }

    let upsert_query = format!(
        "UPSERT provider_settings:`{}` SET {}",
        provider_type,
        set_parts.join(", ")
    );

    info!(query = %upsert_query, "Executing UPSERT query");

    state
        .db
        .execute_with_params(&upsert_query, params)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update provider settings");
            format!("Failed to update settings: {}", e)
        })?;

    info!("Provider settings updated successfully");

    get_provider_settings(provider, state, keystore).await
}
