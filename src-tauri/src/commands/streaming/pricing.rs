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

//! Model pricing and workflow metrics updates.

use crate::{
    llm::pricing::{calculate_cost_with_cache, CostParams},
    models::llm_models::LLMModel,
    AppState,
};
use tracing::{error, info, warn};

/// Pricing information for a model, loaded from agent config and database.
pub struct ModelPricingInfo {
    pub provider: String,
    pub model: String,
    pub model_id: String,
    pub cost_usd: f64,
}

/// Loads agent configuration and model pricing info, then calculates cost.
///
/// Supports cached token pricing: when `cached_tokens` or `cache_write_tokens`
/// is provided, the cost splits input tokens between regular, cache-read, and cache-write rates.
pub async fn load_model_pricing_info(
    state: &AppState,
    agent_id: &str,
    tokens_input: usize,
    tokens_output: usize,
    cached_tokens: Option<usize>,
    cache_write_tokens: Option<usize>,
) -> ModelPricingInfo {
    let (provider, model) = match state.registry.get(agent_id).await {
        Some(agent) => {
            let config = agent.config();
            (config.llm.provider.clone(), config.llm.model.clone())
        }
        None => ("Unknown".to_string(), agent_id.to_string()),
    };

    let (input_price, output_price, cache_read_price, cache_write_price, model_id) = {
        let provider_lower = provider.to_lowercase();
        let model_query = "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
             max_output_tokens, temperature_default, is_builtin, is_reasoning, \
             (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
             (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
             (cache_read_price_per_mtok ?? 0.0) AS cache_read_price_per_mtok, \
             (cache_write_price_per_mtok ?? 0.0) AS cache_write_price_per_mtok, \
             created_at, updated_at \
             FROM llm_model WHERE api_name = $model_name AND provider = $provider_name";

        match state
            .db
            .db
            .query(model_query)
            .bind(("model_name", model.clone()))
            .bind(("provider_name", provider_lower.clone()))
            .await
        {
            Ok(mut response) => {
                let models: Result<Vec<LLMModel>, _> = response.take(0);
                match models {
                    Ok(mut m) if !m.is_empty() => {
                        let loaded_model = m.remove(0);
                        info!(
                            model_api_name = %model,
                            model_id = %loaded_model.id,
                            input_price = loaded_model.input_price_per_mtok,
                            output_price = loaded_model.output_price_per_mtok,
                            cache_read_price = loaded_model.cache_read_price_per_mtok,
                            cache_write_price = loaded_model.cache_write_price_per_mtok,
                            "Loaded model for pricing"
                        );
                        (
                            loaded_model.input_price_per_mtok,
                            loaded_model.output_price_per_mtok,
                            loaded_model.cache_read_price_per_mtok,
                            loaded_model.cache_write_price_per_mtok,
                            loaded_model.id,
                        )
                    }
                    _ => {
                        warn!(model_api_name = %model, provider = %provider, "Model not found for pricing, using defaults");
                        (0.0, 0.0, 0.0, 0.0, model.clone())
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load model for pricing, using defaults");
                (0.0, 0.0, 0.0, 0.0, model.clone())
            }
        }
    };

    let cost_usd = calculate_cost_with_cache(&CostParams {
        tokens_input,
        tokens_output,
        cached_tokens,
        cache_write_tokens,
        input_price_per_mtok: input_price,
        output_price_per_mtok: output_price,
        cache_read_price_per_mtok: cache_read_price,
        cache_write_price_per_mtok: cache_write_price,
    });

    info!(
        tokens_input = tokens_input,
        tokens_output = tokens_output,
        cached_tokens = ?cached_tokens,
        cache_write_tokens = ?cache_write_tokens,
        input_price = input_price,
        output_price = output_price,
        cache_read_price = cache_read_price,
        cache_write_price = cache_write_price,
        cost_usd = cost_usd,
        "Calculated token cost"
    );

    ModelPricingInfo {
        provider,
        model,
        model_id,
        cost_usd,
    }
}

/// Parameters for updating workflow cumulative metrics.
pub struct CumulativeMetricsUpdate<'a> {
    pub workflow_id: &'a str,
    pub tokens_input: usize,
    pub tokens_output: usize,
    pub cached_tokens: Option<usize>,
    pub cache_write_tokens: Option<usize>,
    pub cost_usd: f64,
    pub model_id: &'a str,
    pub context_tokens: usize,
}

/// Updates workflow cumulative token counts, cost, model, and context size.
pub async fn update_workflow_cumulative_metrics(
    state: &AppState,
    params: &CumulativeMetricsUpdate<'_>,
) {
    let CumulativeMetricsUpdate {
        workflow_id,
        tokens_input,
        tokens_output,
        cached_tokens,
        cache_write_tokens,
        cost_usd,
        model_id,
        context_tokens,
    } = params;
    let cached = cached_tokens.unwrap_or(0);
    let cache_write = cache_write_tokens.unwrap_or(0);
    let update_query = format!(
        "UPDATE workflow:`{}` SET \
            total_tokens_input = (total_tokens_input ?? 0) + $tokens_in, \
            total_tokens_output = (total_tokens_output ?? 0) + $tokens_out, \
            total_cached_tokens = (total_cached_tokens ?? 0) + $cached, \
            total_cache_write_tokens = (total_cache_write_tokens ?? 0) + $cache_write, \
            total_cost_usd = (total_cost_usd ?? 0.0) + $cost, \
            model_id = $model_id, \
            current_context_tokens = $context_tokens, \
            updated_at = time::now()",
        workflow_id
    );

    info!(
        tokens_in = *tokens_input,
        tokens_out = *tokens_output,
        cached = cached,
        cache_write = cache_write,
        cost = *cost_usd,
        model_id = %model_id,
        "Executing workflow token update"
    );

    if let Err(e) = state
        .db
        .db
        .query(&update_query)
        .bind(("tokens_in", *tokens_input))
        .bind(("tokens_out", *tokens_output))
        .bind(("cached", cached))
        .bind(("cache_write", cache_write))
        .bind(("cost", *cost_usd))
        .bind(("model_id", model_id.to_string()))
        .bind(("context_tokens", *context_tokens))
        .await
    {
        error!(error = %e, "Failed to update workflow cumulative tokens");
    } else {
        info!(
            workflow_id = %workflow_id,
            tokens_input = *tokens_input,
            tokens_output = *tokens_output,
            cached_tokens = cached,
            cache_write_tokens = cache_write,
            current_context = *context_tokens,
            cost_usd = *cost_usd,
            model_id = %model_id,
            "Updated workflow cumulative tokens and context"
        );
    }
}
