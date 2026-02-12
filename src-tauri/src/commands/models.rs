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

use chrono::Utc;
use std::time::Instant;
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use crate::commands::security::SecureKeyStore;
use crate::models::llm_models::{
    get_all_builtin_models, ConnectionTestResult, CreateModelRequest, LLMModel, ProviderSettings,
    ProviderType, UpdateModelRequest,
};
use crate::state::AppState;
use crate::tools::constants::{commands as cmd_const, query_limits};

// ============================================================================
// Validation Helpers
// ============================================================================

/// Validates a model ID string.
fn validate_model_id(id: &str) -> Result<(), String> {
    if id.trim().is_empty() {
        return Err("Model ID is required".into());
    }
    if id.len() > cmd_const::MAX_MODEL_ID_LEN {
        return Err(format!(
            "Model ID must be {} characters or less",
            cmd_const::MAX_MODEL_ID_LEN
        ));
    }
    Ok(())
}

/// Validates a provider string.
fn validate_provider_string(provider: &str) -> Result<ProviderType, String> {
    provider
        .parse::<ProviderType>()
        .map_err(|_| format!("Invalid provider '{}'", provider))
}

// ============================================================================
// Model CRUD Commands
// ============================================================================

/// Lists all LLM models, optionally filtered by provider.
///
/// # Arguments
///
/// * `provider` - Optional provider filter ("mistral" or "ollama")
///
/// # Returns
///
/// A list of [`LLMModel`] matching the filter criteria.
///
/// # Errors
///
/// Returns an error if:
/// - The provider filter is invalid
/// - Database query fails
#[tauri::command]
#[instrument(name = "list_models", skip(state), fields(provider))]
pub async fn list_models(
    provider: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<LLMModel>, String> {
    // Validate provider filter if provided
    let provider_filter = if let Some(ref p) = provider {
        tracing::Span::current().record("provider", p.as_str());
        Some(validate_provider_string(p)?)
    } else {
        None
    };

    info!(
        provider_filter = ?provider_filter,
        "Listing models"
    );

    // Build query based on filter
    // Use ?? (null coalescing) for pricing fields to handle existing records without these fields
    // Add LIMIT to prevent memory explosion (OPT-DB-8)
    let query = if let Some(ref pt) = provider_filter {
        format!(
            "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
             max_output_tokens, temperature_default, is_builtin, is_reasoning, \
             (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
             (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
             created_at, updated_at \
             FROM llm_model WHERE provider = '{}' LIMIT {}",
            pt,
            query_limits::DEFAULT_MODELS_LIMIT
        )
    } else {
        format!(
            "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
             max_output_tokens, temperature_default, is_builtin, is_reasoning, \
             (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
             (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
             created_at, updated_at \
             FROM llm_model LIMIT {}",
            query_limits::DEFAULT_MODELS_LIMIT
        )
    };

    let result: Vec<LLMModel> = state
        .db
        .db
        .query(&query)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to query models");
            format!("Database error: {}", e)
        })?
        .take(0)
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize models");
            format!("Failed to deserialize models: {}", e)
        })?;

    info!(count = result.len(), "Models retrieved");
    Ok(result)
}

/// Gets a single model by ID.
///
/// # Arguments
///
/// * `id` - The model ID to retrieve
///
/// # Returns
///
/// The [`LLMModel`] with the given ID.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The model is not found
/// - Database query fails
#[tauri::command]
#[instrument(name = "get_model", skip(state), fields(model_id = %id))]
pub async fn get_model(id: String, state: State<'_, AppState>) -> Result<LLMModel, String> {
    validate_model_id(&id)?;

    info!("Getting model");

    // Query by record ID directly (llm_model:uuid)
    // Use ?? (null coalescing) for pricing fields to handle existing records without these fields
    let query = format!(
        "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
         max_output_tokens, temperature_default, is_builtin, is_reasoning, \
         (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
         (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
         created_at, updated_at \
         FROM llm_model:`{}`",
        id
    );

    let mut result: Vec<LLMModel> = state
        .db
        .db
        .query(&query)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to query model");
            format!("Database error: {}", e)
        })?
        .take(0)
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize model");
            format!("Failed to deserialize model: {}", e)
        })?;

    result.pop().ok_or_else(|| {
        warn!(model_id = %id, "Model not found");
        format!("Model not found: {}", id)
    })
}

/// Gets a single model by api_name and provider.
///
/// This is used when the model ID is not known, but the api_name
/// (e.g., "mistral-large-latest") and provider are available.
///
/// # Arguments
///
/// * `api_name` - The model API name (e.g., "mistral-large-latest")
/// * `provider` - The provider name (e.g., "mistral", "ollama")
///
/// # Returns
///
/// The [`LLMModel`] matching the api_name and provider.
///
/// # Errors
///
/// Returns an error if:
/// - The model is not found
/// - Database query fails
#[tauri::command]
#[instrument(name = "get_model_by_api_name", skip(state), fields(api_name = %api_name, provider = %provider))]
pub async fn get_model_by_api_name(
    api_name: String,
    provider: String,
    state: State<'_, AppState>,
) -> Result<LLMModel, String> {
    info!("Getting model by api_name");

    // Convert provider to lowercase for matching (DB stores lowercase)
    let provider_lower = provider.to_lowercase();

    let query = format!(
        "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
         max_output_tokens, temperature_default, is_builtin, is_reasoning, \
         (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
         (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
         created_at, updated_at \
         FROM llm_model WHERE api_name = '{}' AND provider = '{}'",
        api_name, provider_lower
    );

    let mut result: Vec<LLMModel> = state
        .db
        .db
        .query(&query)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to query model by api_name");
            format!("Database error: {}", e)
        })?
        .take(0)
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize model");
            format!("Failed to deserialize model: {}", e)
        })?;

    result.pop().ok_or_else(|| {
        warn!(api_name = %api_name, provider = %provider, "Model not found");
        format!("Model not found: {} (provider: {})", api_name, provider)
    })
}

/// Creates a new custom LLM model.
///
/// # Arguments
///
/// * `data` - The model creation data
///
/// # Returns
///
/// The created [`LLMModel`].
///
/// # Errors
///
/// Returns an error if:
/// - The data validation fails
/// - A model with the same api_name already exists for the provider
/// - Database operation fails
#[tauri::command]
#[instrument(name = "create_model", skip(state, data), fields(provider, name))]
pub async fn create_model(
    data: CreateModelRequest,
    state: State<'_, AppState>,
) -> Result<LLMModel, String> {
    tracing::Span::current().record("provider", data.provider.to_string().as_str());
    tracing::Span::current().record("name", &data.name);

    // Validate request
    data.validate()?;

    info!(
        api_name = %data.api_name,
        context_window = data.context_window,
        "Creating custom model"
    );

    // Check for duplicate api_name
    let check_query = format!(
        "SELECT count() FROM llm_model WHERE provider = '{}' AND api_name = '{}' GROUP ALL",
        data.provider, data.api_name
    );

    let count_result: Option<serde_json::Value> = state
        .db
        .db
        .query(&check_query)
        .await
        .map_err(|e| format!("Database error: {}", e))?
        .take(0)
        .map_err(|e| format!("Failed to check duplicate: {}", e))?;

    if let Some(val) = count_result {
        if let Some(count) = val.get("count").and_then(|v| v.as_i64()) {
            if count > 0 {
                return Err(format!(
                    "Model with api_name '{}' already exists for provider {}",
                    data.api_name, data.provider
                ));
            }
        }
    }

    // Generate ID and timestamps
    let model_id = Uuid::new_v4().to_string();
    let now = Utc::now();

    // Create the model from request
    let model = LLMModel::from_create_request(model_id.clone(), &data);

    // Insert into database using raw query (SurrealDB SDK 2.x workaround)
    let insert_query = format!(
        "CREATE llm_model:`{}` CONTENT {{ \
            id: '{}', \
            provider: '{}', \
            name: '{}', \
            api_name: '{}', \
            context_window: {}, \
            max_output_tokens: {}, \
            temperature_default: {}, \
            is_builtin: false, \
            is_reasoning: {}, \
            input_price_per_mtok: {}, \
            output_price_per_mtok: {}, \
            created_at: time::now(), \
            updated_at: time::now() \
        }}",
        model_id,
        model_id,
        model.provider,
        model.name.replace('\'', "''"),
        model.api_name.replace('\'', "''"),
        model.context_window,
        model.max_output_tokens,
        model.temperature_default,
        model.is_reasoning,
        model.input_price_per_mtok,
        model.output_price_per_mtok
    );

    state.db.execute(&insert_query).await.map_err(|e| {
        error!(error = %e, "Failed to create model");
        format!("Failed to create model: {}", e)
    })?;

    info!(model_id = %model_id, "Model created");

    // Return the model with proper timestamps
    Ok(LLMModel {
        id: model_id,
        provider: model.provider,
        name: model.name,
        api_name: model.api_name,
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        temperature_default: model.temperature_default,
        is_builtin: false,
        is_reasoning: model.is_reasoning,
        input_price_per_mtok: model.input_price_per_mtok,
        output_price_per_mtok: model.output_price_per_mtok,
        created_at: now,
        updated_at: now,
    })
}

/// Updates an existing LLM model.
///
/// For builtin models, only `temperature_default` can be modified.
///
/// # Arguments
///
/// * `id` - The model ID to update
/// * `data` - The update data
///
/// # Returns
///
/// The updated [`LLMModel`].
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The model is not found
/// - The update data is invalid
/// - Attempting to modify protected fields on a builtin model
/// - Database operation fails
#[tauri::command]
#[instrument(name = "update_model", skip(state, data), fields(model_id = %id))]
pub async fn update_model(
    id: String,
    data: UpdateModelRequest,
    state: State<'_, AppState>,
) -> Result<LLMModel, String> {
    validate_model_id(&id)?;

    if data.is_empty() {
        return Err("No fields to update".into());
    }

    // Fetch existing model
    let existing = get_model(id.clone(), state.clone()).await?;

    // Validate update (respects builtin restrictions)
    data.validate(existing.is_builtin)?;

    info!(is_builtin = existing.is_builtin, "Updating model");

    // Build SET clause dynamically
    let mut set_parts: Vec<String> = vec!["updated_at = time::now()".to_string()];

    if let Some(ref name) = data.name {
        set_parts.push(format!("name = '{}'", name.replace('\'', "''")));
    }
    if let Some(ref api_name) = data.api_name {
        // Check for duplicate api_name
        let check_query = format!(
            "SELECT count() FROM llm_model WHERE provider = '{}' AND api_name = '{}' AND id != '{}' GROUP ALL",
            existing.provider, api_name, id
        );
        let count_result: Option<serde_json::Value> = state
            .db
            .db
            .query(&check_query)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .take(0)
            .map_err(|e| format!("Failed to check duplicate: {}", e))?;

        if let Some(val) = count_result {
            if let Some(count) = val.get("count").and_then(|v| v.as_i64()) {
                if count > 0 {
                    return Err(format!(
                        "Model with api_name '{}' already exists for provider {}",
                        api_name, existing.provider
                    ));
                }
            }
        }
        set_parts.push(format!("api_name = '{}'", api_name.replace('\'', "''")));
    }
    if let Some(ctx) = data.context_window {
        set_parts.push(format!("context_window = {}", ctx));
    }
    if let Some(max_out) = data.max_output_tokens {
        set_parts.push(format!("max_output_tokens = {}", max_out));
    }
    if let Some(temp) = data.temperature_default {
        set_parts.push(format!("temperature_default = {}", temp));
    }
    if let Some(is_reasoning) = data.is_reasoning {
        set_parts.push(format!("is_reasoning = {}", is_reasoning));
    }
    if let Some(input_price) = data.input_price_per_mtok {
        set_parts.push(format!("input_price_per_mtok = {}", input_price));
    }
    if let Some(output_price) = data.output_price_per_mtok {
        set_parts.push(format!("output_price_per_mtok = {}", output_price));
    }

    let update_query = format!("UPDATE llm_model:`{}` SET {}", id, set_parts.join(", "));

    state.db.execute(&update_query).await.map_err(|e| {
        error!(error = %e, "Failed to update model");
        format!("Failed to update model: {}", e)
    })?;

    info!("Model updated");

    // Return updated model
    get_model(id, state).await
}

/// Deletes a custom LLM model.
///
/// Builtin models cannot be deleted.
///
/// # Arguments
///
/// * `id` - The model ID to delete
///
/// # Returns
///
/// `true` if the model was deleted.
///
/// # Errors
///
/// Returns an error if:
/// - The ID is invalid
/// - The model is not found
/// - The model is builtin (cannot be deleted)
/// - Database operation fails
#[tauri::command]
#[instrument(name = "delete_model", skip(state), fields(model_id = %id))]
pub async fn delete_model(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    validate_model_id(&id)?;

    // Fetch existing model to check if builtin
    let existing = get_model(id.clone(), state.clone()).await?;

    if existing.is_builtin {
        return Err("Cannot delete builtin models".into());
    }

    info!("Deleting model");

    let delete_query = format!("DELETE llm_model:`{}`", id);

    state.db.execute(&delete_query).await.map_err(|e| {
        error!(error = %e, "Failed to delete model");
        format!("Failed to delete model: {}", e)
    })?;

    info!("Model deleted");
    Ok(true)
}

// ============================================================================
// Provider Settings Commands
// ============================================================================

/// Gets settings for a provider.
///
/// If no settings exist, returns default settings for the provider.
///
/// # Arguments
///
/// * `provider` - The provider name ("mistral" or "ollama")
///
/// # Returns
///
/// The [`ProviderSettings`] for the provider.
///
/// # Errors
///
/// Returns an error if:
/// - The provider is invalid
/// - Database query fails
#[tauri::command]
#[instrument(name = "get_provider_settings", skip(state, keystore), fields(provider = %provider))]
pub async fn get_provider_settings(
    provider: String,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ProviderSettings, String> {
    let provider_type = validate_provider_string(&provider)?;

    info!("Getting provider settings");

    // Query by record ID (provider_settings:mistral or provider_settings:ollama)
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
            format!("Database error: {}", e)
        })?
        .take(0)
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize settings");
            format!("Failed to deserialize settings: {}", e)
        })?;

    info!(found = result.is_some(), "Provider settings query result");

    // Check if API key is configured (using the secure keystore)
    let api_key_configured = match &provider_type {
        ProviderType::Mistral => keystore.has_key("Mistral"),
        ProviderType::Custom(name) => keystore.has_key(name),
        ProviderType::Ollama => false, // Ollama doesn't need API key
    };

    match result {
        Some(mut settings) => {
            settings.api_key_configured = api_key_configured;
            Ok(settings)
        }
        None => {
            // Return default settings
            let mut default = ProviderSettings::default_for(provider_type);
            default.api_key_configured = api_key_configured;
            Ok(default)
        }
    }
}

/// Updates settings for a provider.
///
/// Creates settings if they don't exist (upsert behavior).
///
/// # Arguments
///
/// * `provider` - The provider name
/// * `enabled` - Whether to enable/disable the provider
/// * `default_model_id` - The default model ID for this provider
/// * `base_url` - Custom base URL (mainly for Ollama)
///
/// # Returns
///
/// The updated [`ProviderSettings`].
///
/// # Errors
///
/// Returns an error if:
/// - The provider is invalid
/// - The default_model_id doesn't exist
/// - Database operation fails
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

    // Build SET clause with null coalescing (??) to preserve existing values
    // or use defaults for new records
    let mut set_parts: Vec<String> = vec![
        format!("provider = '{}'", provider_type),
        "updated_at = time::now()".to_string(),
    ];

    // For enabled: use provided value, keep existing, or default to true
    if let Some(en) = enabled {
        set_parts.push(format!("enabled = {}", en));
    } else {
        set_parts.push("enabled = enabled ?? true".to_string());
    }

    // For default_model_id: use provided value or keep existing
    if let Some(ref model_id) = default_model_id {
        set_parts.push(format!("default_model_id = '{}'", model_id));
    } else {
        set_parts.push("default_model_id = default_model_id".to_string());
    }

    // For base_url: use provided value or keep existing
    if let Some(ref url) = base_url {
        set_parts.push(format!("base_url = '{}'", url.replace('\'', "''")));
    } else {
        set_parts.push("base_url = base_url".to_string());
    }

    // Upsert: create if not exists, update if exists
    let upsert_query = format!(
        "UPSERT provider_settings:`{}` SET {}",
        provider_type,
        set_parts.join(", ")
    );

    info!(query = %upsert_query, "Executing UPSERT query");

    state.db.execute(&upsert_query).await.map_err(|e| {
        error!(error = %e, "Failed to update provider settings");
        format!("Failed to update settings: {}", e)
    })?;

    info!("Provider settings updated successfully");

    get_provider_settings(provider, state, keystore).await
}

// ============================================================================
// Connection Test Commands
// ============================================================================

/// Tests connection to an LLM provider.
///
/// For Ollama: Tests the `/api/version` endpoint.
/// For Mistral: Tests by making a simple API request.
///
/// # Arguments
///
/// * `provider` - The provider to test ("mistral" or "ollama")
///
/// # Returns
///
/// A [`ConnectionTestResult`] with success status, latency, and error details.
///
/// # Notes
///
/// - Timeout is 10 seconds
/// - Returns success=false with error message on failure
#[tauri::command]
#[instrument(name = "test_provider_connection", skip(state, keystore), fields(provider = %provider))]
pub async fn test_provider_connection(
    provider: String,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ConnectionTestResult, String> {
    let provider_type = validate_provider_string(&provider)?;

    info!("Testing provider connection");

    let start = Instant::now();

    match provider_type {
        ProviderType::Ollama => match state.llm_manager.ollama().test_connection().await {
            Ok(success) => {
                let latency = start.elapsed().as_millis() as u64;
                if success {
                    info!(latency_ms = latency, "Ollama connection successful");
                    Ok(ConnectionTestResult::success(provider_type, latency, None))
                } else {
                    warn!("Ollama connection returned false");
                    Ok(ConnectionTestResult::failure(
                        provider_type,
                        "Connection test returned false".into(),
                    ))
                }
            }
            Err(e) => {
                warn!(error = %e, "Ollama connection failed");
                Ok(ConnectionTestResult::failure(
                    provider_type,
                    format!("Connection failed: {}", e),
                ))
            }
        },
        ProviderType::Mistral => {
            // Check if API key is configured (from OS keychain)
            let api_key = match keystore.get_key("Mistral") {
                Some(key) => key,
                None => {
                    return Ok(ConnectionTestResult::failure(
                        provider_type,
                        "API key not configured".into(),
                    ));
                }
            };

            // Test Mistral by making a models list request
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

            let response = client
                .get("https://api.mistral.ai/v1/models")
                .header("Authorization", format!("Bearer {}", api_key))
                .send()
                .await;

            let latency = start.elapsed().as_millis() as u64;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        info!(latency_ms = latency, "Mistral connection successful");
                        Ok(ConnectionTestResult::success(provider_type, latency, None))
                    } else {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        warn!(status = %status, body = %body, "Mistral API error");
                        Ok(ConnectionTestResult::failure(
                            provider_type,
                            format!("API error ({}): {}", status, body),
                        ))
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Mistral connection failed");
                    Ok(ConnectionTestResult::failure(
                        provider_type,
                        format!("Connection failed: {}", e),
                    ))
                }
            }
        }
        ProviderType::Custom(ref name) => {
            let name = name.clone();
            match state.llm_manager.get_custom_provider(&name).await {
                Some(custom_provider) => match custom_provider.test_connection().await {
                    Ok(success) => {
                        let latency = start.elapsed().as_millis() as u64;
                        if success {
                            info!(provider = %name, latency_ms = latency, "Custom provider connection successful");
                            Ok(ConnectionTestResult::success(provider_type, latency, None))
                        } else {
                            warn!(provider = %name, "Custom provider connection returned false");
                            Ok(ConnectionTestResult::failure(
                                provider_type,
                                "Connection test returned false".into(),
                            ))
                        }
                    }
                    Err(e) => {
                        warn!(provider = %name, error = %e, "Custom provider connection failed");
                        Ok(ConnectionTestResult::failure(
                            provider_type,
                            format!("Connection failed: {}", e),
                        ))
                    }
                },
                None => Ok(ConnectionTestResult::failure(
                    provider_type,
                    format!("Custom provider '{}' not found", name),
                )),
            }
        }
    }
}

// ============================================================================
// Seed Commands
// ============================================================================

/// Seeds the database with builtin models.
///
/// This command inserts all builtin models if they don't already exist.
/// Safe to call multiple times (uses INSERT IGNORE pattern).
///
/// # Returns
///
/// The number of models inserted.
///
/// # Errors
///
/// Returns an error if database operations fail.
#[tauri::command]
#[instrument(name = "seed_builtin_models", skip(state))]
pub async fn seed_builtin_models(state: State<'_, AppState>) -> Result<usize, String> {
    info!("Seeding builtin models");

    let models = get_all_builtin_models();
    let mut inserted = 0;

    for model in &models {
        // Check if model already exists
        let check_query = format!(
            "SELECT count() FROM llm_model WHERE id = '{}' GROUP ALL",
            model.id
        );
        let count_result: Vec<serde_json::Value> = state
            .db
            .db
            .query(&check_query)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .take(0)
            .unwrap_or_default();

        let exists = count_result
            .first()
            .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
            .unwrap_or(0)
            > 0;

        if !exists {
            let insert_query = format!(
                "CREATE llm_model:`{}` CONTENT {{ \
                    id: '{}', \
                    provider: '{}', \
                    name: '{}', \
                    api_name: '{}', \
                    context_window: {}, \
                    max_output_tokens: {}, \
                    temperature_default: {}, \
                    is_builtin: true, \
                    is_reasoning: {}, \
                    input_price_per_mtok: {}, \
                    output_price_per_mtok: {}, \
                    created_at: time::now(), \
                    updated_at: time::now() \
                }}",
                model.id,
                model.id,
                model.provider,
                model.name.replace('\'', "''"),
                model.api_name.replace('\'', "''"),
                model.context_window,
                model.max_output_tokens,
                model.temperature_default,
                model.is_reasoning,
                model.input_price_per_mtok,
                model.output_price_per_mtok
            );

            state.db.execute(&insert_query).await.map_err(|e| {
                error!(error = %e, model_id = %model.id, "Failed to insert builtin model");
                format!("Failed to insert model {}: {}", model.id, e)
            })?;

            inserted += 1;
        }
    }

    info!(
        total = models.len(),
        inserted = inserted,
        "Builtin models seeded"
    );
    Ok(inserted)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Validation Helper Tests
    // ========================================================================

    #[test]
    fn test_validate_model_id_valid() {
        // Standard UUIDs
        assert!(validate_model_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
        // Short IDs
        assert!(validate_model_id("valid-id").is_ok());
        // API names as IDs (for builtin models)
        assert!(validate_model_id("mistral-large-latest").is_ok());
        // Single character
        assert!(validate_model_id("a").is_ok());
        // Max length (128 chars)
        assert!(validate_model_id(&"a".repeat(128)).is_ok());
    }

    #[test]
    fn test_validate_model_id_invalid() {
        // Empty string
        assert!(validate_model_id("").is_err());
        // Whitespace only
        assert!(validate_model_id("   ").is_err());
        assert!(validate_model_id("\t\n").is_err());
        // Too long (>128 chars)
        assert!(validate_model_id(&"a".repeat(129)).is_err());
        assert!(validate_model_id(&"x".repeat(200)).is_err());
    }

    #[test]
    fn test_validate_provider_string_valid() {
        // Lowercase
        assert!(validate_provider_string("mistral").is_ok());
        assert!(validate_provider_string("ollama").is_ok());
        // Uppercase
        assert!(validate_provider_string("MISTRAL").is_ok());
        assert!(validate_provider_string("OLLAMA").is_ok());
        // Mixed case
        assert!(validate_provider_string("Mistral").is_ok());
        assert!(validate_provider_string("Ollama").is_ok());
        assert!(validate_provider_string("MiStRaL").is_ok());
    }

    #[test]
    fn test_validate_provider_string_returns_correct_type() {
        let mistral = validate_provider_string("mistral").unwrap();
        assert_eq!(mistral, ProviderType::Mistral);

        let ollama = validate_provider_string("OLLAMA").unwrap();
        assert_eq!(ollama, ProviderType::Ollama);
    }

    #[test]
    fn test_validate_provider_string_invalid() {
        // Empty string rejected
        assert!(validate_provider_string("").is_err());
    }

    #[test]
    fn test_validate_provider_string_custom_providers() {
        // Unknown names now parse as Custom providers
        let custom = validate_provider_string("routerlab").unwrap();
        assert_eq!(custom, ProviderType::Custom("routerlab".to_string()));

        let custom2 = validate_provider_string("openai").unwrap();
        assert_eq!(custom2, ProviderType::Custom("openai".to_string()));
    }

    #[test]
    fn test_validate_provider_string_error_message() {
        let err = validate_provider_string("").unwrap_err();
        assert!(err.contains("Invalid provider"));
    }

    // ========================================================================
    // Constants Tests
    // ========================================================================

    #[test]
    fn test_max_model_id_len_constant() {
        assert_eq!(cmd_const::MAX_MODEL_ID_LEN, 128);
    }

    #[test]
    fn test_validate_provider_string() {
        assert!(validate_provider_string("mistral").is_ok());
        assert!(validate_provider_string("ollama").is_ok());
        assert!(validate_provider_string("routerlab").is_ok()); // Custom providers accepted
        assert!(validate_provider_string("").is_err()); // Empty rejected
    }
}
