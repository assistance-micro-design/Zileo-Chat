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

//! Seed command for builtin LLM models.

use tauri::State;
use tracing::{error, info, instrument};

use crate::db::count_exists;
use crate::models::llm_models::get_all_builtin_models;
use crate::state::AppState;

/// Seeds the database with builtin models.
///
/// This command inserts all builtin models if they don't already exist.
/// Safe to call multiple times (uses INSERT IGNORE pattern).
#[tauri::command]
#[instrument(name = "seed_builtin_models", skip(state))]
pub async fn seed_builtin_models(state: State<'_, AppState>) -> Result<usize, String> {
    info!("Seeding builtin models");

    let models = get_all_builtin_models();
    let mut inserted = 0;

    for model in &models {
        let check_query = format!(
            "SELECT count() FROM llm_model WHERE id = '{}' GROUP ALL",
            model.id
        );
        let count_result: Vec<serde_json::Value> = state
            .db
            .db
            .query(&check_query)
            .await
            .map_err(|e| format!("Failed to check builtin model existence: {}", e))?
            .take(0)
            .unwrap_or_default();

        let exists = count_exists(&count_result);

        if !exists {
            let insert_query = format!(
                "CREATE llm_model:`{}` CONTENT $data ; \
                 UPDATE llm_model:`{}` SET created_at = time::now(), updated_at = time::now()",
                model.id, model.id
            );
            let insert_data = serde_json::json!({
                "id": model.id,
                "provider": model.provider.as_id(),
                "name": model.name,
                "api_name": model.api_name,
                "context_window": model.context_window,
                "max_output_tokens": model.max_output_tokens,
                "temperature_default": model.temperature_default,
                "is_builtin": true,
                "is_reasoning": model.is_reasoning,
                "input_price_per_mtok": model.input_price_per_mtok,
                "output_price_per_mtok": model.output_price_per_mtok,
                "cache_read_price_per_mtok": model.cache_read_price_per_mtok,
                "cache_write_price_per_mtok": model.cache_write_price_per_mtok,
            });

            state
                .db
                .execute_with_params(&insert_query, vec![("data".to_string(), insert_data)])
                .await
                .map_err(|e| {
                    error!(error = %e, model_id = %model.id, "Failed to insert builtin model");
                    format!("Failed to insert model {}: {}", model.id, e)
                })?;

            inserted += 1;
        }
    }

    // Normalize existing provider values to lowercase (fixes case mismatch from Display trait)
    state
        .db
        .db
        .query("UPDATE llm_model SET provider = string::lowercase(provider)")
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to normalize provider values");
            format!("Failed to normalize provider values: {}", e)
        })?;

    info!(
        total = models.len(),
        inserted = inserted,
        "Builtin models seeded (provider values normalized)"
    );
    Ok(inserted)
}
