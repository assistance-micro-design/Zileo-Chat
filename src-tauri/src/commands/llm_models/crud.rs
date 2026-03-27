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

//! Model CRUD commands: list, get, get_by_api_name, create, update, delete.

use chrono::Utc;
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use super::{validate_model_id, validate_provider_string};
use crate::constants::query_limits;
use crate::db::count_exists;
use crate::models::llm_models::{CreateModelRequest, LLMModel, UpdateModelRequest};
use crate::state::AppState;

/// SELECT column list for LLM model queries.
///
/// Uses `meta::id(id)` for clean UUIDs and `??` null coalescing for pricing
/// fields to handle existing records created before pricing was added.
const LLM_MODEL_SELECT_COLUMNS: &str =
    "meta::id(id) AS id, provider, name, api_name, context_window, \
     max_output_tokens, temperature_default, is_builtin, is_reasoning, \
     (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
     (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
     (cache_read_price_per_mtok ?? 0.0) AS cache_read_price_per_mtok, \
     (cache_write_price_per_mtok ?? 0.0) AS cache_write_price_per_mtok, \
     created_at, updated_at";

/// Lists all LLM models, optionally filtered by provider.
#[tauri::command]
#[instrument(name = "list_models", skip(state), fields(provider))]
pub async fn list_models(
    provider: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<LLMModel>, String> {
    let provider_filter = if let Some(ref p) = provider {
        tracing::Span::current().record("provider", p.as_str());
        Some(validate_provider_string(p)?)
    } else {
        None
    };

    info!(provider_filter = ?provider_filter, "Listing models");

    // Use ?? (null coalescing) for pricing fields to handle existing records without these fields
    let result: Vec<LLMModel> = if let Some(ref pt) = provider_filter {
        let query = format!(
            "SELECT {} FROM llm_model WHERE provider = $provider LIMIT {}",
            LLM_MODEL_SELECT_COLUMNS,
            query_limits::DEFAULT_MODELS_LIMIT
        );
        state
            .db
            .query_with_params(
                &query,
                vec![("provider".to_string(), serde_json::json!(pt.as_id()))],
            )
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to query models");
                format!("Failed to query models: {}", e)
            })?
    } else {
        let query = format!(
            "SELECT {} FROM llm_model LIMIT {}",
            LLM_MODEL_SELECT_COLUMNS,
            query_limits::DEFAULT_MODELS_LIMIT
        );
        state
            .db
            .db
            .query(&query)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to query models");
                format!("Failed to query models: {}", e)
            })?
            .take(0)
            .map_err(|e| {
                error!(error = %e, "Failed to deserialize models");
                format!("Failed to deserialize models: {}", e)
            })?
    };

    info!(count = result.len(), "Models retrieved");
    Ok(result)
}

/// Gets a single model by ID.
#[tauri::command]
#[instrument(name = "get_model", skip(state), fields(model_id = %id))]
pub async fn get_model(id: String, state: State<'_, AppState>) -> Result<LLMModel, String> {
    validate_model_id(&id)?;

    info!("Getting model");

    let query = format!(
        "SELECT {} FROM llm_model:`{}`",
        LLM_MODEL_SELECT_COLUMNS, id
    );

    let mut result: Vec<LLMModel> = state
        .db
        .db
        .query(&query)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to query model");
            format!("Failed to query model: {}", e)
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
#[tauri::command]
#[instrument(name = "get_model_by_api_name", skip(state), fields(api_name = %api_name, provider = %provider))]
pub async fn get_model_by_api_name(
    api_name: String,
    provider: String,
    state: State<'_, AppState>,
) -> Result<LLMModel, String> {
    info!("Getting model by api_name");

    let provider_lower = provider.to_lowercase();

    let query = format!(
        "SELECT {} FROM llm_model WHERE api_name = $api_name AND provider = $provider",
        LLM_MODEL_SELECT_COLUMNS
    );

    let mut result: Vec<LLMModel> = state
        .db
        .query_with_params(
            &query,
            vec![
                ("api_name".to_string(), serde_json::json!(api_name)),
                ("provider".to_string(), serde_json::json!(provider_lower)),
            ],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to query model by api_name");
            format!("Failed to query model by api_name: {}", e)
        })?;

    result.pop().ok_or_else(|| {
        warn!(api_name = %api_name, provider = %provider, "Model not found");
        format!("Model not found: {} (provider: {})", api_name, provider)
    })
}

/// Creates a new custom LLM model.
#[tauri::command]
#[instrument(name = "create_model", skip(state, data), fields(provider, name))]
pub async fn create_model(
    data: CreateModelRequest,
    state: State<'_, AppState>,
) -> Result<LLMModel, String> {
    tracing::Span::current().record("provider", data.provider.to_string().as_str());
    tracing::Span::current().record("name", &data.name);

    data.validate()?;

    info!(
        api_name = %data.api_name,
        context_window = data.context_window,
        "Creating custom model"
    );

    check_model_uniqueness(&state.db, &data).await?;

    let model_id = Uuid::new_v4().to_string();
    let now = Utc::now();
    let model = LLMModel::from_create_request(model_id.clone(), &data);

    insert_model_record(&state.db, &model_id, &model).await?;

    info!(model_id = %model_id, "Model created");

    Ok(LLMModel {
        created_at: now,
        updated_at: now,
        ..model
    })
}

/// Checks that no model with the same api_name exists for the provider.
async fn check_model_uniqueness(
    db: &crate::db::DBClient,
    data: &CreateModelRequest,
) -> Result<(), String> {
    let check_query =
        "SELECT count() FROM llm_model WHERE provider = $provider AND api_name = $api_name GROUP ALL";

    let check_results: Vec<serde_json::Value> = db
        .query_json_with_params(
            check_query,
            vec![
                (
                    "provider".to_string(),
                    serde_json::json!(data.provider.as_id()),
                ),
                ("api_name".to_string(), serde_json::json!(&data.api_name)),
            ],
        )
        .await
        .map_err(|e| format!("Failed to check model uniqueness: {}", e))?;

    if count_exists(&check_results) {
        return Err(format!(
            "Model with api_name '{}' already exists for provider {}",
            data.api_name, data.provider
        ));
    }

    Ok(())
}

/// Inserts a model record into the database with timestamps.
async fn insert_model_record(
    db: &crate::db::DBClient,
    model_id: &str,
    model: &LLMModel,
) -> Result<(), String> {
    let insert_query = format!("CREATE llm_model:`{}` CONTENT $data", model_id);
    let insert_data = serde_json::json!({
        "id": model_id,
        "provider": model.provider.as_id(),
        "name": model.name,
        "api_name": model.api_name,
        "context_window": model.context_window,
        "max_output_tokens": model.max_output_tokens,
        "temperature_default": model.temperature_default,
        "is_builtin": false,
        "is_reasoning": model.is_reasoning,
        "input_price_per_mtok": model.input_price_per_mtok,
        "output_price_per_mtok": model.output_price_per_mtok,
        "cache_read_price_per_mtok": model.cache_read_price_per_mtok,
        "cache_write_price_per_mtok": model.cache_write_price_per_mtok,
    });

    db.execute_with_params(
        &format!(
            "{} ; UPDATE llm_model:`{}` SET created_at = time::now(), updated_at = time::now()",
            insert_query, model_id
        ),
        vec![("data".to_string(), insert_data)],
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Failed to create model");
        format!("Failed to create model: {}", e)
    })
}

/// Updates an existing LLM model.
///
/// For builtin models, only `temperature_default` can be modified.
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

    let existing = get_model(id.clone(), state.clone()).await?;
    data.validate(existing.is_builtin)?;

    info!(is_builtin = existing.is_builtin, "Updating model");

    let mut set_parts: Vec<String> = vec!["updated_at = time::now()".to_string()];
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    if let Some(ref name) = data.name {
        set_parts.push("name = $p_name".to_string());
        params.push(("p_name".to_string(), serde_json::json!(name)));
    }
    if let Some(ref api_name) = data.api_name {
        let check_query = "SELECT count() FROM llm_model \
            WHERE provider = $provider AND api_name = $api_name AND meta::id(id) != $exclude_id \
            GROUP ALL";
        let check_results: Vec<serde_json::Value> = state
            .db
            .query_json_with_params(
                check_query,
                vec![
                    (
                        "provider".to_string(),
                        serde_json::json!(existing.provider.as_id()),
                    ),
                    ("api_name".to_string(), serde_json::json!(api_name)),
                    ("exclude_id".to_string(), serde_json::json!(&id)),
                ],
            )
            .await
            .map_err(|e| format!("Failed to check model uniqueness: {}", e))?;

        if count_exists(&check_results) {
            return Err(format!(
                "Model with api_name '{}' already exists for provider {}",
                api_name, existing.provider
            ));
        }
        set_parts.push("api_name = $p_api_name".to_string());
        params.push(("p_api_name".to_string(), serde_json::json!(api_name)));
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
    if let Some(cached_price) = data.cache_read_price_per_mtok {
        set_parts.push(format!("cache_read_price_per_mtok = {}", cached_price));
    }
    if let Some(cache_write_price) = data.cache_write_price_per_mtok {
        set_parts.push(format!(
            "cache_write_price_per_mtok = {}",
            cache_write_price
        ));
    }

    let update_query = format!("UPDATE llm_model:`{}` SET {}", id, set_parts.join(", "));

    state
        .db
        .execute_with_params(&update_query, params)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update model");
            format!("Failed to update model: {}", e)
        })?;

    info!("Model updated");

    get_model(id, state).await
}

/// Deletes a custom LLM model.
///
/// Builtin models cannot be deleted.
#[tauri::command]
#[instrument(name = "delete_model", skip(state), fields(model_id = %id))]
pub async fn delete_model(id: String, state: State<'_, AppState>) -> Result<bool, String> {
    validate_model_id(&id)?;

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
