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
    llm::pricing::{calculate_cost_with_cache, resolve_cost, CostParams},
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
    /// Status of the pricing lookup. `Ok` only when prices > 0.
    pub status: PricingStatus,
}

/// Outcome of the pricing lookup. Phase 8 surfaces this to the frontend so
/// "free" can be distinguished from "pricing missing".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PricingStatus {
    /// Model row found and prices are non-zero.
    Ok,
    /// No matching model row in the `llm_model` table.
    ModelNotFound,
    /// Model found but `input_price` and `output_price` are both 0.
    NoPricingSet,
}

/// Loads agent configuration and model pricing info, then calculates cost.
///
/// Supports cached token pricing: when `cached_tokens` or `cache_write_tokens`
/// is provided, the cost splits input tokens between regular, cache-read, and cache-write rates.
///
/// When `provider_cost_usd` is provided (e.g. OpenRouter `usage.cost`), it
/// takes precedence over the locally-computed cost — the provider's billing
/// is the source of truth.
pub async fn load_model_pricing_info(
    state: &AppState,
    agent_id: &str,
    tokens_input: usize,
    tokens_output: usize,
    cached_tokens: Option<usize>,
    cache_write_tokens: Option<usize>,
    provider_cost_usd: Option<f64>,
) -> ModelPricingInfo {
    let (provider, model) = match state.registry.get(agent_id).await {
        Some(agent) => {
            let config = agent.config();
            (config.llm.provider.clone(), config.llm.model.clone())
        }
        None => ("Unknown".to_string(), agent_id.to_string()),
    };

    let (input_price, output_price, cache_read_price, cache_write_price, model_id, status) = {
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
                        let status = if loaded_model.input_price_per_mtok == 0.0
                            && loaded_model.output_price_per_mtok == 0.0
                        {
                            PricingStatus::NoPricingSet
                        } else {
                            PricingStatus::Ok
                        };
                        (
                            loaded_model.input_price_per_mtok,
                            loaded_model.output_price_per_mtok,
                            loaded_model.cache_read_price_per_mtok,
                            loaded_model.cache_write_price_per_mtok,
                            loaded_model.id,
                            status,
                        )
                    }
                    _ => {
                        warn!(model_api_name = %model, provider = %provider, "Model not found for pricing, using defaults");
                        (
                            0.0,
                            0.0,
                            0.0,
                            0.0,
                            model.clone(),
                            PricingStatus::ModelNotFound,
                        )
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load model for pricing, using defaults");
                (
                    0.0,
                    0.0,
                    0.0,
                    0.0,
                    model.clone(),
                    PricingStatus::ModelNotFound,
                )
            }
        }
    };

    let local_cost = calculate_cost_with_cache(&CostParams {
        tokens_input,
        tokens_output,
        cached_tokens,
        cache_write_tokens,
        input_price_per_mtok: input_price,
        output_price_per_mtok: output_price,
        cache_read_price_per_mtok: cache_read_price,
        cache_write_price_per_mtok: cache_write_price,
    });

    let cost_usd = resolve_cost(provider_cost_usd, local_cost);

    info!(
        tokens_input = tokens_input,
        tokens_output = tokens_output,
        cached_tokens = ?cached_tokens,
        cache_write_tokens = ?cache_write_tokens,
        input_price = input_price,
        output_price = output_price,
        cache_read_price = cache_read_price,
        cache_write_price = cache_write_price,
        local_cost = local_cost,
        provider_cost_usd = ?provider_cost_usd,
        cost_usd = cost_usd,
        pricing_status = ?status,
        cost_source = if matches!(provider_cost_usd, Some(c) if c > 0.0) { "provider" } else { "local" },
        "Resolved token cost"
    );

    ModelPricingInfo {
        provider,
        model,
        model_id,
        cost_usd,
        status,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_cost_prefers_positive_provider_value() {
        // Provider value wins, even if locally we would compute something else.
        assert_eq!(resolve_cost(Some(0.42), 0.10), 0.42);
        assert_eq!(resolve_cost(Some(0.001), 999.0), 0.001);
    }

    #[test]
    fn resolve_cost_falls_back_to_local_when_none() {
        assert_eq!(resolve_cost(None, 0.05), 0.05);
        assert_eq!(resolve_cost(None, 0.0), 0.0);
    }

    #[test]
    fn resolve_cost_falls_back_to_local_when_provider_is_zero() {
        // `Some(0.0)` is treated as "no signal" — a real free request also
        // yields `local_cost == 0.0` so falling back is consistent.
        assert_eq!(resolve_cost(Some(0.0), 0.05), 0.05);
        assert_eq!(resolve_cost(Some(0.0), 0.0), 0.0);
    }

    #[test]
    fn resolve_cost_falls_back_when_provider_negative() {
        // Defensive: providers shouldn't return negatives, but if they do, we
        // ignore the bad signal rather than persist absurd values.
        assert_eq!(resolve_cost(Some(-1.0), 0.10), 0.10);
    }
}
