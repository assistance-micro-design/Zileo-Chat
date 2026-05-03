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

//! # Token Pricing Module
//!
//! This module provides cost calculation functionality based on token counts
//! and model pricing configuration.
//!
//! ## Pricing Model
//!
//! LLM providers typically charge per million tokens (MTok) with different
//! rates for input (prompt), output (completion), cache-read, and cache-write tokens.
//!
//! ## Cache Pricing (OpenRouter)
//!
//! - **Cache read**: tokens served from cache, typically 0.25x-0.50x of input price
//! - **Cache write**: tokens written to cache on first request, typically 1.0x-1.25x of input price
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::llm::pricing::{calculate_cost_with_cache, CostParams};
//!
//! let cost = calculate_cost_with_cache(&CostParams {
//!     tokens_input: 10000, tokens_output: 2000,
//!     cached_tokens: None, cache_write_tokens: None,
//!     input_price_per_mtok: 2.0, output_price_per_mtok: 6.0,
//!     cache_read_price_per_mtok: 0.0, cache_write_price_per_mtok: 0.0,
//! });
//! // Input: 10000 tokens * $2/MTok = $0.02
//! // Output: 2000 tokens * $6/MTok = $0.012
//! // Total: $0.032
//! ```

/// Parameters for cost calculation with cache-aware pricing.
pub struct CostParams {
    /// Total input tokens (includes cached read + write)
    pub tokens_input: usize,
    /// Output tokens
    pub tokens_output: usize,
    /// Number of input tokens served from cache (cache reads)
    pub cached_tokens: Option<usize>,
    /// Number of input tokens written to cache (cache writes)
    pub cache_write_tokens: Option<usize>,
    /// Price per million regular (non-cached) input tokens
    pub input_price_per_mtok: f64,
    /// Price per million output tokens
    pub output_price_per_mtok: f64,
    /// Price per million cache-read tokens
    pub cache_read_price_per_mtok: f64,
    /// Price per million cache-write tokens
    pub cache_write_price_per_mtok: f64,
}

/// Calculates cost with separate pricing for cache-read and cache-write tokens.
///
/// Returns total cost in USD, rounded to 6 decimal places.
pub fn calculate_cost_with_cache(p: &CostParams) -> f64 {
    let cache_read = p.cached_tokens.unwrap_or(0).min(p.tokens_input);
    let cache_write = p
        .cache_write_tokens
        .unwrap_or(0)
        .min(p.tokens_input.saturating_sub(cache_read));
    let regular_input = p.tokens_input.saturating_sub(cache_read + cache_write);

    let regular_cost = (regular_input as f64 / 1_000_000.0) * p.input_price_per_mtok;
    let read_cost = (cache_read as f64 / 1_000_000.0) * p.cache_read_price_per_mtok;
    let write_cost = (cache_write as f64 / 1_000_000.0) * p.cache_write_price_per_mtok;
    let output_cost = (p.tokens_output as f64 / 1_000_000.0) * p.output_price_per_mtok;

    let total = regular_cost + read_cost + write_cost + output_cost;
    (total * 1_000_000.0).round() / 1_000_000.0
}

/// Returns the authoritative cost in USD given a provider-reported cost and a
/// locally-computed cost.
///
/// When the provider reports a strictly positive cost (e.g. OpenRouter
/// `usage.cost`), it is used as the source of truth — providers know their
/// real billing including discounts, BYOK pricing, etc. Otherwise the local
/// calculation is used as fallback.
///
/// `provider_cost == Some(0.0)` is treated as "no signal" rather than "free":
/// a real free request would also produce `local_cost == 0.0`, so falling back
/// to local is safe and consistent.
pub fn resolve_cost(provider_cost: Option<f64>, local_cost: f64) -> f64 {
    match provider_cost {
        Some(c) if c > 0.0 => c,
        _ => local_cost,
    }
}

/// Pricing query result for a given (provider, api_name).
#[derive(Debug, Clone, PartialEq)]
pub struct ModelPricingRow {
    pub model_id: String,
    pub input_price_per_mtok: f64,
    pub output_price_per_mtok: f64,
    pub cache_read_price_per_mtok: f64,
    pub cache_write_price_per_mtok: f64,
}

/// Loads the pricing row for a given `(api_name, provider)` pair from the
/// `llm_model` table. Returns `None` when no matching model exists.
///
/// The function lives in `llm/pricing.rs` (not `commands/streaming/pricing.rs`)
/// so it can be reused by the sub-agent executor without depending on the
/// `commands` layer.
pub async fn load_pricing_row(
    db: &crate::db::DBClient,
    api_name: &str,
    provider: &str,
) -> Option<ModelPricingRow> {
    let provider_lower = provider.to_lowercase();
    let query = "SELECT meta::id(id) AS id, \
         (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
         (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
         (cache_read_price_per_mtok ?? 0.0) AS cache_read_price_per_mtok, \
         (cache_write_price_per_mtok ?? 0.0) AS cache_write_price_per_mtok \
         FROM llm_model WHERE api_name = $api AND provider = $prov LIMIT 1";

    let rows = db
        .query_json_with_params(
            query,
            vec![
                ("api".to_string(), serde_json::json!(api_name)),
                ("prov".to_string(), serde_json::json!(provider_lower)),
            ],
        )
        .await
        .ok()?;

    let row = rows.into_iter().next()?;
    Some(ModelPricingRow {
        model_id: row.get("id")?.as_str()?.to_string(),
        input_price_per_mtok: row.get("input_price_per_mtok")?.as_f64()?,
        output_price_per_mtok: row.get("output_price_per_mtok")?.as_f64()?,
        cache_read_price_per_mtok: row.get("cache_read_price_per_mtok")?.as_f64()?,
        cache_write_price_per_mtok: row.get("cache_write_price_per_mtok")?.as_f64()?,
    })
}

/// Outcome of a sub-agent cost computation.
///
/// Currently exposes only the resolved cost. The `llm_model.id` is not
/// returned: sub-agent records reference the agent (whose model is in the
/// registry), so the lookup can be performed downstream when needed.
#[derive(Debug, Clone)]
pub struct SubAgentCost {
    /// Resolved cost in USD (provider-reported when available, else local calc).
    pub cost_usd: f64,
}

/// Token usage + provider-reported cost for a single sub-agent execution.
///
/// Bundling these reduces the argument count of `compute_sub_agent_cost`
/// (clippy::too_many_arguments).
#[derive(Debug, Clone, Default)]
pub struct SubAgentCostInput {
    pub tokens_input: usize,
    pub tokens_output: usize,
    pub cached_tokens: Option<usize>,
    pub cache_write_tokens: Option<usize>,
    pub provider_cost_usd: Option<f64>,
}

/// Computes the cost of a sub-agent execution using the SUB-AGENT's own pricing
/// (looked up from its registered AgentConfig), not the parent's.
///
/// Returns `None` when the sub-agent is not registered. Returns
/// `Some(cost_usd = 0.0)` when the model is not in the pricing table — the
/// caller can decide whether to treat that as "free" or surface a warning.
pub async fn compute_sub_agent_cost(
    db: &crate::db::DBClient,
    registry: &crate::agents::core::AgentRegistry,
    sub_agent_id: &str,
    usage: SubAgentCostInput,
) -> Option<SubAgentCost> {
    let agent = registry.get(sub_agent_id).await?;
    let cfg = agent.config();
    let api_name = cfg.llm.model.clone();
    let provider = cfg.llm.provider.clone();

    let pricing = load_pricing_row(db, &api_name, &provider).await;

    let local_cost = if let Some(ref row) = pricing {
        calculate_cost_with_cache(&CostParams {
            tokens_input: usage.tokens_input,
            tokens_output: usage.tokens_output,
            cached_tokens: usage.cached_tokens,
            cache_write_tokens: usage.cache_write_tokens,
            input_price_per_mtok: row.input_price_per_mtok,
            output_price_per_mtok: row.output_price_per_mtok,
            cache_read_price_per_mtok: row.cache_read_price_per_mtok,
            cache_write_price_per_mtok: row.cache_write_price_per_mtok,
        })
    } else {
        0.0
    };

    Some(SubAgentCost {
        cost_usd: resolve_cost(usage.provider_cost_usd, local_cost),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::too_many_arguments)]
    fn cost(
        ti: usize,
        to: usize,
        cr: Option<usize>,
        cw: Option<usize>,
        ip: f64,
        op: f64,
        crp: f64,
        cwp: f64,
    ) -> f64 {
        calculate_cost_with_cache(&CostParams {
            tokens_input: ti,
            tokens_output: to,
            cached_tokens: cr,
            cache_write_tokens: cw,
            input_price_per_mtok: ip,
            output_price_per_mtok: op,
            cache_read_price_per_mtok: crp,
            cache_write_price_per_mtok: cwp,
        })
    }

    #[test]
    fn test_calculate_cost_no_cache() {
        assert!((cost(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0) - 0.032).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_small_model() {
        assert!((cost(50000, 10000, None, None, 0.2, 0.6, 0.0, 0.0) - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_calculate_cost_zero_pricing() {
        assert_eq!(cost(100000, 50000, None, None, 0.0, 0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn test_calculate_cost_zero_tokens() {
        assert_eq!(cost(0, 0, None, None, 2.0, 6.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn test_calculate_cost_large_token_count() {
        assert!(
            (cost(1_000_000, 1_000_000, None, None, 2.0, 6.0, 0.0, 0.0) - 8.0).abs() < 0.000001
        );
    }

    #[test]
    fn test_calculate_cost_precision() {
        assert!((cost(100, 50, None, None, 2.0, 6.0, 0.0, 0.0) - 0.0005).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_50_percent_price() {
        assert!((cost(10000, 2000, Some(8000), None, 2.0, 6.0, 1.0, 0.0) - 0.024).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_free() {
        assert!((cost(10000, 2000, Some(8000), None, 2.0, 6.0, 0.0, 0.0) - 0.016).abs() < 0.000001);
    }

    #[test]
    fn test_cache_write_anthropic_125x() {
        assert!((cost(10000, 2000, None, Some(8000), 2.0, 6.0, 0.0, 2.5) - 0.036).abs() < 0.000001);
    }

    #[test]
    fn test_cache_read_and_write_combined() {
        assert!(
            (cost(10000, 1000, Some(5000), Some(3000), 2.0, 6.0, 0.5, 2.5) - 0.02).abs() < 0.000001
        );
    }

    #[test]
    fn test_cache_exceeding_input_clamped() {
        assert!((cost(1000, 500, Some(2000), None, 2.0, 6.0, 1.0, 0.0) - 0.004).abs() < 0.000001);
    }

    #[test]
    fn test_no_cache_info_same_as_regular() {
        let c1 = cost(10000, 2000, None, None, 2.0, 6.0, 1.0, 2.5);
        let c2 = cost(10000, 2000, None, None, 2.0, 6.0, 0.0, 0.0);
        assert!((c1 - c2).abs() < 0.000001);
    }

    // =========================================================================
    // compute_sub_agent_cost — integration tests against a real DB + registry.
    //
    // These cover the four call paths that drive sub-agent billing accuracy:
    //   1. registered + pricing row present  -> local cost via SUB-agent prices
    //   2. registered + provider_cost > 0    -> provider cost wins
    //   3. registered + no pricing row       -> Some(0.0) (caller decides)
    //   4. not registered                    -> None (no agent => no cost)
    // =========================================================================

    use crate::test_utils::{seed_llm_model, setup_test_state, TestRegistryAgent};
    use std::sync::Arc;

    #[tokio::test]
    async fn compute_sub_agent_cost_uses_sub_agents_own_pricing() {
        let (state, _db_guard) = setup_test_state().await;

        // Sub-agent uses an EXPENSIVE model ($10/$30 per MTok input/output).
        let sub_id = "sub-agent-expensive";
        seed_llm_model(&state.db, "Mistral", "premium-model", 10.0, 30.0).await;
        state
            .registry
            .register(
                sub_id.to_string(),
                Arc::new(TestRegistryAgent::new(sub_id, "Mistral", "premium-model")),
            )
            .await;

        let result = compute_sub_agent_cost(
            &state.db,
            &state.registry,
            sub_id,
            SubAgentCostInput {
                tokens_input: 1_000_000,
                tokens_output: 1_000_000,
                cached_tokens: None,
                cache_write_tokens: None,
                provider_cost_usd: None,
            },
        )
        .await;

        let cost = result.expect("registered agent + pricing row -> Some");
        // 1M input * $10 + 1M output * $30 = $10 + $30 = $40
        assert!(
            (cost.cost_usd - 40.0).abs() < 0.000001,
            "Expected $40 from SUB-agent pricing, got ${}",
            cost.cost_usd
        );
    }

    #[tokio::test]
    async fn compute_sub_agent_cost_prefers_provider_cost_when_present() {
        let (state, _db_guard) = setup_test_state().await;

        // Local pricing would compute $40, but provider reports $0.99 (e.g.
        // OpenRouter discount or BYOK). Provider value MUST win.
        let sub_id = "sub-agent-openrouter";
        seed_llm_model(&state.db, "Custom", "model-x", 10.0, 30.0).await;
        state
            .registry
            .register(
                sub_id.to_string(),
                Arc::new(TestRegistryAgent::new(sub_id, "Custom", "model-x")),
            )
            .await;

        let result = compute_sub_agent_cost(
            &state.db,
            &state.registry,
            sub_id,
            SubAgentCostInput {
                tokens_input: 1_000_000,
                tokens_output: 1_000_000,
                cached_tokens: None,
                cache_write_tokens: None,
                provider_cost_usd: Some(0.99),
            },
        )
        .await
        .expect("registered agent -> Some");

        assert!(
            (result.cost_usd - 0.99).abs() < 0.000001,
            "Expected provider-reported $0.99 to win over local $40, got ${}",
            result.cost_usd
        );
    }

    #[tokio::test]
    async fn compute_sub_agent_cost_returns_zero_when_pricing_missing() {
        let (state, _db_guard) = setup_test_state().await;

        // Agent registered, but NO llm_model row seeded for its (provider, model).
        let sub_id = "sub-agent-no-pricing";
        state
            .registry
            .register(
                sub_id.to_string(),
                Arc::new(TestRegistryAgent::new(sub_id, "Custom", "ghost-model")),
            )
            .await;

        let result = compute_sub_agent_cost(
            &state.db,
            &state.registry,
            sub_id,
            SubAgentCostInput {
                tokens_input: 100_000,
                tokens_output: 50_000,
                cached_tokens: None,
                cache_write_tokens: None,
                provider_cost_usd: None,
            },
        )
        .await
        .expect("registered agent -> Some even without pricing");

        // Returns Some(0.0) so the caller can distinguish "registered but
        // no pricing data" from "agent unknown" (which returns None).
        assert_eq!(
            result.cost_usd, 0.0,
            "Missing pricing row should yield $0.0 (caller decides how to surface)"
        );
    }

    #[tokio::test]
    async fn compute_sub_agent_cost_returns_none_for_unregistered_agent() {
        let (state, _db_guard) = setup_test_state().await;

        // Nothing registered. Even if pricing row existed, we have no
        // (provider, model) mapping for this id.
        let result = compute_sub_agent_cost(
            &state.db,
            &state.registry,
            "ghost-agent-id",
            SubAgentCostInput {
                tokens_input: 1000,
                tokens_output: 500,
                cached_tokens: None,
                cache_write_tokens: None,
                provider_cost_usd: Some(1.23),
            },
        )
        .await;

        assert!(
            result.is_none(),
            "Unregistered agent must yield None (no pricing context to use)"
        );
    }

    #[tokio::test]
    async fn compute_sub_agent_cost_propagates_cache_savings() {
        let (state, _db_guard) = setup_test_state().await;

        // 80% of input is served from cache at 0.5x the input price -> the
        // wired path through calculate_cost_with_cache must preserve that.
        let sub_id = "sub-agent-cached";
        crate::test_utils::seed_llm_model_with_cache(
            &state.db,
            "Custom",
            "cached-model",
            2.0,
            6.0,
            1.0,
            0.0,
        )
        .await;
        state
            .registry
            .register(
                sub_id.to_string(),
                Arc::new(TestRegistryAgent::new(sub_id, "Custom", "cached-model")),
            )
            .await;

        let result = compute_sub_agent_cost(
            &state.db,
            &state.registry,
            sub_id,
            SubAgentCostInput {
                tokens_input: 10_000,
                tokens_output: 2_000,
                cached_tokens: Some(8_000),
                cache_write_tokens: None,
                provider_cost_usd: None,
            },
        )
        .await
        .expect("registered agent -> Some");

        // 2k regular input * $2/M + 8k cache-read * $1/M + 2k output * $6/M
        // = 0.004 + 0.008 + 0.012 = $0.024
        assert!(
            (result.cost_usd - 0.024).abs() < 0.000001,
            "Expected $0.024 with cache savings, got ${}",
            result.cost_usd
        );
    }
}
