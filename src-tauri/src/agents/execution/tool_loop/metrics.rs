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
//! Pricing cache, token tracking and report-metric projection.
use crate::agents::core::agent::{ReasoningStepData, ReportMetrics, ToolExecutionData};
use crate::db::DBClient;
use crate::llm::pricing::{load_pricing_row, ModelPricingRow};
use crate::llm::tool_adapter::TokenUsage;
use crate::models::workflow::IterationMetrics;
use crate::models::AgentConfig;

/// Cache of the pricing row for the agent's `(provider, model)` pair, loaded
/// once at the start of the tool loop so each iteration can compute a per-call
/// cost without firing an extra DB query (the final cumulative cost still
/// reaches the wire via `response_block` from `persistence_step.rs` —
/// backend-as-source-of-truth invariant).
///
/// Holds `pricing = None` when the model is absent from `llm_model`; callers
/// then skip the live cost emission entirely (the chunk's `cost_usd` stays
/// `None` and the frontend gracefully falls back to the final cost).
pub(crate) struct PricingCache {
    /// Loaded pricing row, or `None` when the `(provider, model)` pair was
    /// not found in the `llm_model` table.
    pub pricing: Option<ModelPricingRow>,
}

impl PricingCache {
    /// Loads the pricing row for the agent's model. Always returns a
    /// `PricingCache`: the inner `pricing` field is `None` when the lookup
    /// fails or no matching row exists, so callers don't have to handle
    /// errors at every iteration.
    pub(crate) async fn load(db: &DBClient, config: &AgentConfig) -> Self {
        let pricing = load_pricing_row(db, &config.llm.model, &config.llm.provider).await;
        Self { pricing }
    }

    /// Computes the local cost for a single iteration given its per-call
    /// token counts. Returns `None` when no pricing row is cached so the
    /// caller can pass `cost_usd: None` straight to the wire (the frontend
    /// then waits for the final `response_block` cost).
    ///
    /// Extracted as a pure function so the live-cost path can be unit-tested
    /// without instantiating a full tool loop. Called by `iteration.rs` after
    /// each LLM call to project the chunk's `cost_usd`.
    pub(crate) fn compute_iteration_local_cost(
        &self,
        iter_input: usize,
        iter_output: usize,
        iter_cached: Option<usize>,
        iter_cache_write: Option<usize>,
    ) -> Option<f64> {
        self.pricing.as_ref().map(|row| {
            crate::llm::pricing::calculate_cost_with_cache(&crate::llm::pricing::CostParams {
                tokens_input: iter_input,
                tokens_output: iter_output,
                cached_tokens: iter_cached,
                cache_write_tokens: iter_cache_write,
                input_price_per_mtok: row.input_price_per_mtok,
                output_price_per_mtok: row.output_price_per_mtok,
                cache_read_price_per_mtok: row.cache_read_price_per_mtok,
                cache_write_price_per_mtok: row.cache_write_price_per_mtok,
            })
        })
    }
}

/// Tracks cumulative and per-iteration token usage across the tool loop.
pub(crate) struct TokenTracker {
    pub total_input: usize,
    pub total_output: usize,
    /// Last call's input tokens (context window size)
    pub context: usize,
    pub total_cached: Option<usize>,
    pub total_cache_write: Option<usize>,
    pub total_thinking: Option<usize>,
    /// Cumulative provider-reported cost (e.g. OpenRouter) summed across iterations.
    /// Stays `None` if no iteration reported a cost.
    pub total_provider_cost_usd: Option<f64>,
    // Per-iteration values (overwritten each iteration, read for IterationMetrics)
    pub iter_input: usize,
    pub iter_output: usize,
    pub iter_cached: Option<usize>,
    pub iter_cache_write: Option<usize>,
    pub iter_thinking: Option<usize>,
}

impl TokenTracker {
    pub(crate) fn new() -> Self {
        Self {
            total_input: 0,
            total_output: 0,
            context: 0,
            total_cached: None,
            total_cache_write: None,
            total_thinking: None,
            total_provider_cost_usd: None,
            iter_input: 0,
            iter_output: 0,
            iter_cached: None,
            iter_cache_write: None,
            iter_thinking: None,
        }
    }

    /// Records token usage from an LLM response, updating both per-iteration and cumulative values.
    pub(crate) fn record(&mut self, usage: &TokenUsage) {
        self.iter_input = usage.input_tokens;
        self.iter_output = usage.output_tokens;
        self.iter_cached = usage.cached_tokens;
        self.iter_cache_write = usage.cache_write_tokens;
        self.iter_thinking = usage.thinking_tokens;

        self.total_input += usage.input_tokens;
        self.context = usage.input_tokens;
        self.total_output += usage.output_tokens;

        Self::accumulate(&mut self.total_cached, usage.cached_tokens);
        Self::accumulate(&mut self.total_cache_write, usage.cache_write_tokens);
        Self::accumulate(&mut self.total_thinking, usage.thinking_tokens);
        Self::accumulate_f64(&mut self.total_provider_cost_usd, usage.provider_cost_usd);
    }

    /// Adds estimated thinking tokens (fallback when provider doesn't report them).
    pub(crate) fn add_estimated_thinking(&mut self, estimated: usize) {
        self.iter_thinking = Some(estimated);
        Self::accumulate(&mut self.total_thinking, Some(estimated));
    }

    fn accumulate(total: &mut Option<usize>, value: Option<usize>) {
        if let Some(val) = value {
            *total = Some(total.unwrap_or(0) + val);
        }
    }

    fn accumulate_f64(total: &mut Option<f64>, value: Option<f64>) {
        if let Some(val) = value {
            *total = Some(total.unwrap_or(0.0) + val);
        }
    }

    pub(crate) fn to_report_metrics(
        &self,
        tools_used: Vec<String>,
        mcp_calls: Vec<String>,
        tool_executions: Vec<ToolExecutionData>,
        reasoning_steps: Vec<ReasoningStepData>,
        iteration_metrics: Vec<IterationMetrics>,
    ) -> ReportMetrics {
        ReportMetrics {
            duration_ms: 0, // caller sets this
            tokens_input: self.total_input,
            tokens_output: self.total_output,
            context_tokens: self.context,
            cached_tokens: self.total_cached,
            cache_write_tokens: self.total_cache_write,
            thinking_tokens: self.total_thinking,
            provider_cost_usd: self.total_provider_cost_usd,
            tools_used,
            mcp_calls,
            tool_executions,
            reasoning_steps,
            iteration_metrics,
        }
    }
}
