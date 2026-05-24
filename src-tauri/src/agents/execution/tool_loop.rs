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

//! Tool execution loop for LLM agents.
//!
//! This module is the orchestrator only. The hot logic lives in sibling
//! modules:
//! - [`super::reasoning`]: emit_progress / emit_reasoning + small format helpers
//! - [`super::completion`]: report enforcement + report content building
//! - [`super::iteration`]: a single pass of the LLM-call → tool-execute loop

use crate::agents::core::agent::{
    ReasoningSource, ReasoningStepData, Report, ReportMetrics, ReportStatus, Task,
    ToolExecutionData,
};
use crate::agents::execution::completion::{
    build_report_content, enforce_report, EnforcementState, ReportContentInputs,
};
use crate::agents::execution::iteration::{
    run_single_iteration, IterationInputs, IterationMutState, IterationOutcome,
};
use crate::agents::execution::reasoning::{
    effective_reasoning_effort, emit_progress, emit_reasoning, format_llm_error,
};
use crate::agents::execution::tools;
use crate::agents::prompt;
use crate::db::DBClient;
use crate::llm::adapters::{MistralToolAdapter, OllamaToolAdapter, OpenAiToolAdapter};
use crate::llm::pricing::{load_pricing_row, ModelPricingRow};
use crate::llm::tool_adapter::{ProviderToolAdapter, TokenUsage};
use crate::llm::{CompletionParams, ProviderManager, ProviderType};
use crate::mcp::MCPManager;
use crate::models::function_calling::ToolChoiceMode;
use crate::models::streaming::StreamChunk;
use crate::models::workflow::IterationMetrics;
use crate::models::AgentConfig;
use crate::tools::{
    context::AgentToolContext, validation_helper::ValidationHelper, Tool, ToolFactory,
};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

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

    fn to_report_metrics(
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

/// Context for the tool execution loop, grouping all dependencies.
pub(crate) struct ToolLoopContext<'a> {
    pub config: &'a AgentConfig,
    pub provider_manager: &'a ProviderManager,
    pub tool_factory: Option<&'a Arc<ToolFactory>>,
    pub agent_context: Option<&'a AgentToolContext>,
}

/// Builds the initial message vector sent to the LLM at the start of a tool loop.
///
/// Two branches:
/// - **Continuation**: `task.context["conversation_messages"]` contains a non-empty
///   array of `{role, content}` entries persisted in the DB. The current user
///   message has already been saved by the frontend before the streaming call,
///   so the array already ends with the latest user turn — we replay it as-is
///   under a freshly regenerated system prompt and do NOT re-append
///   `task.description` (that would duplicate the last user turn).
/// - **First call** (or empty history fallback): build a `[system, user]` pair
///   from the regenerated system prompt and the formatted user prompt
///   (`prompt::build_prompt` may wrap the description with extra context).
///
/// The system prompt is rebuilt every turn because it depends on live agent
/// configuration (tools, MCP servers, locale, current date) that can change
/// between turns. It is therefore never persisted in the DB.
fn build_initial_messages(task: &Task, system_prompt: String) -> Vec<serde_json::Value> {
    let existing = task
        .context
        .get("conversation_messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if !existing.is_empty() {
        let history_count = existing.len();
        let mut msgs = Vec::with_capacity(history_count + 1);
        msgs.push(serde_json::json!({
            "role": "system",
            "content": system_prompt,
        }));
        msgs.extend(existing);
        debug!(
            history_count = history_count,
            "Continuing conversation: regenerated system prompt + replayed history"
        );
        msgs
    } else {
        let base_prompt = prompt::build_prompt(task);
        debug!("First message: building new system prompt with tools");

        // Promote a `pending_attachments` payload (set by `build_task` for
        // streaming workflows) into a multipart user turn. Emits the DEFAULT
        // OpenAI shape; per-provider adapters re-normalize at body-build time.
        let attachments: Vec<crate::models::MessageAttachment> = task
            .context
            .get("pending_attachments")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let user_content = if attachments.is_empty() {
            serde_json::Value::String(base_prompt)
        } else {
            let mut parts = vec![serde_json::json!({
                "type": "text",
                "text": base_prompt,
            })];
            for att in &attachments {
                parts.push(crate::llm::image_format::build_image_content_part_openai(
                    att,
                ));
            }
            serde_json::Value::Array(parts)
        };

        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_content}),
        ]
    }
}

/// Executes a task without tools (simple LLM completion).
///
/// `cancellation_token` (when present) races the LLM call so a workflow
/// cancellation tears down the in-flight HTTP request.
pub(crate) async fn execute_simple(
    config: &AgentConfig,
    provider_manager: &ProviderManager,
    agent_context: Option<&AgentToolContext>,
    task: Task,
    cancellation_token: Option<CancellationToken>,
) -> anyhow::Result<Report> {
    let start = std::time::Instant::now();

    debug!(
        agent_name = %config.name,
        system_prompt_len = config.system_prompt.len(),
        "LLM Agent starting simple task execution"
    );

    let user_prompt = prompt::build_prompt(&task);

    // Same defense-in-depth as execute_with_tools: a task carrying any of
    // the three delegation flags is treated as a sub-agent run so chunks
    // emitted from here are attributed correctly to the delegated agent.
    let is_sub_agent = ["is_sub_agent", "is_delegation", "is_parallel_task"]
        .iter()
        .any(|key| {
            task.context
                .get(*key)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        });

    let provider_type = match config.llm.provider.parse::<ProviderType>() {
        Ok(pt) => pt,
        Err(e) => {
            error!(error = %e, "Invalid provider type in config");
            return Ok(Report::failed(
                &config.id,
                &task.description,
                format!("Invalid provider configuration: {}", e),
                start.elapsed().as_millis() as u64,
            ));
        }
    };

    if !provider_manager.is_provider_configured(provider_type.clone()) {
        warn!(
            ?provider_type,
            "Provider not configured, returning configuration error"
        );
        return Ok(Report::failed(
            &config.id,
            &task.description,
            format!(
                "LLM provider '{}' is not configured. Please configure it in Settings.",
                provider_type
            ),
            start.elapsed().as_millis() as u64,
        ));
    }

    let llm_result = provider_manager
        .complete_with_provider_cancellable(
            provider_type.clone(),
            CompletionParams {
                prompt: user_prompt.clone(),
                system_prompt: Some(config.system_prompt.clone()),
                model: Some(config.llm.model.clone()),
                temperature: config.llm.temperature,
                max_tokens: config.llm.max_tokens,
                reasoning_effort: effective_reasoning_effort(config),
                context_window: config.llm.context_window,
            },
            cancellation_token,
        )
        .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    let event_workflow_id = task
        .context
        .get("workflow_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| task.id.clone());

    match llm_result {
        Ok(response) => {
            info!(
                tokens_input = response.tokens_input,
                tokens_output = response.tokens_output,
                model = %response.model,
                duration_ms = duration_ms,
                "LLM Agent task execution completed successfully"
            );

            let mut reasoning_steps = vec![];
            if let Some(ref thinking) = response.thinking_content {
                if !thinking.trim().is_empty() {
                    emit_progress(
                        agent_context,
                        StreamChunk::thinking_block(
                            event_workflow_id.clone(),
                            thinking.clone(),
                            Some(config.id.clone()),
                            Some(config.name.clone()),
                            is_sub_agent,
                        ),
                    );
                    reasoning_steps.push(ReasoningStepData {
                        content: thinking.clone(),
                        duration_ms,
                        sequence: 1,
                        source: ReasoningSource::ModelThinking,
                    });
                }
            }

            let content = format!(
                "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Response\n\n{}\n\n## Metrics\n- Provider: {}\n- Model: {}\n- Tokens (input/output): {}/{}\n- Duration: {}ms",
                config.id,
                task.description,
                response.content,
                response.provider,
                response.model,
                response.tokens_input,
                response.tokens_output,
                duration_ms
            );

            Ok(Report {
                status: ReportStatus::Success,
                content,
                response: response.content.clone(),
                metrics: ReportMetrics {
                    duration_ms,
                    tokens_input: response.tokens_input,
                    tokens_output: response.tokens_output,
                    context_tokens: response.tokens_input,
                    cached_tokens: response.cached_tokens,
                    cache_write_tokens: response.cache_write_tokens,
                    thinking_tokens: response.thinking_tokens,
                    provider_cost_usd: response.provider_cost_usd,
                    tools_used: vec![],
                    mcp_calls: vec![],
                    tool_executions: vec![],
                    reasoning_steps,
                    iteration_metrics: vec![],
                },
            })
        }
        Err(e) => {
            error!(error = %e, "LLM call failed");
            Ok(Report::failed(
                &config.id,
                &task.description,
                format_llm_error(&e),
                duration_ms,
            ))
        }
    }
}

/// Resolves the `tool_choice` to apply for a given iteration of the tool loop.
///
/// `opening_tool_choice` is what the caller requested for the *first* turn.
/// We honour it only on iteration 1 and fall back to [`ToolChoiceMode::Auto`]
/// afterwards. This is what lets a flow force the model to engage its tools on
/// the opening turn (e.g. Kanban analyze / compose, where the model otherwise
/// writes prose and finishes without ever calling its submit tool) while still
/// allowing it to *finish* naturally on a later turn — a plain `Auto` analyze
/// could end with no tool call, and a blanket `Required` would never let the
/// loop terminate (no turn could be tool-free), spinning until max_iterations.
fn tool_choice_for_iteration(
    iteration: usize,
    opening_tool_choice: ToolChoiceMode,
) -> ToolChoiceMode {
    if iteration <= 1 {
        opening_tool_choice
    } else {
        ToolChoiceMode::Auto
    }
}

/// Executes a task with full tool support (local + MCP) using JSON function calling.
///
/// `extra_tools` lets callers inject privately-instantiated tools (carrying
/// captured state via `Arc<Mutex<_>>`, etc.) alongside factory-resolved ones.
/// These are concatenated after the factory's `create_local_tools` output and
/// participate normally in tool definition collection, system-prompt injection
/// and JSON function-call dispatch. Pass `vec![]` when no injection is needed
/// (the standard workflow case).
///
/// `opening_tool_choice` is the `tool_choice` applied to the *first* iteration
/// only (see [`tool_choice_for_iteration`]). Pass [`ToolChoiceMode::Auto`] for
/// the standard workflow path; pass [`ToolChoiceMode::Required`] for flows that
/// must obtain a single mandatory tool call (Kanban analyze / compose).
pub(crate) async fn execute_with_tools(
    ctx: ToolLoopContext<'_>,
    task: Task,
    mcp_manager: Option<Arc<MCPManager>>,
    cancellation_token: Option<CancellationToken>,
    extra_tools: Vec<Arc<dyn Tool>>,
    opening_tool_choice: ToolChoiceMode,
) -> anyhow::Result<Report> {
    let start = std::time::Instant::now();
    let mut tools_used: Vec<String> = Vec::new();
    let mut mcp_calls_made: Vec<String> = Vec::new();
    let mut tokens = TokenTracker::new();
    let mut iteration_metrics_data: Vec<IterationMetrics> = Vec::new();
    let mut tool_executions_data: Vec<ToolExecutionData> = Vec::new();
    let mut reasoning_steps_data: Vec<ReasoningStepData> = Vec::new();

    // Get provider type early to fail fast
    let provider_type = match ctx.config.llm.provider.parse::<ProviderType>() {
        Ok(pt) => pt,
        Err(e) => {
            error!(error = %e, "Invalid provider type in config");
            return Ok(Report::failed(
                &ctx.config.id,
                &task.description,
                format!("Invalid provider configuration: {}", e),
                start.elapsed().as_millis() as u64,
            ));
        }
    };

    if !ctx
        .provider_manager
        .is_provider_configured(provider_type.clone())
    {
        warn!(
            ?provider_type,
            "Provider not configured, returning configuration error"
        );
        return Ok(Report::failed(
            &ctx.config.id,
            &task.description,
            format!(
                "LLM provider '{}' is not configured. Please configure it in Settings.",
                provider_type
            ),
            start.elapsed().as_millis() as u64,
        ));
    }

    let adapter: Box<dyn ProviderToolAdapter> = match provider_type {
        ProviderType::Mistral => Box::new(MistralToolAdapter::new()),
        ProviderType::Ollama => Box::new(OllamaToolAdapter::new()),
        ProviderType::Custom(_) => Box::new(OpenAiToolAdapter::new()),
    };

    let workflow_id = task
        .context
        .get("workflow_id")
        .and_then(|v| v.as_str())
        .map(String::from);

    let event_workflow_id = workflow_id.clone().unwrap_or_else(|| task.id.clone());

    let validation_helper = if let Some(factory) = ctx.tool_factory {
        let db = factory.get_db();
        let app_handle = match ctx.agent_context.and_then(|c| c.app_handle.clone()) {
            Some(handle) => Some(handle),
            None => factory.get_app_handle().await,
        };
        Some(ValidationHelper::new(db, app_handle))
    } else {
        None
    };

    let is_primary_agent = task
        .context
        .get("is_primary_agent")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // True for any agent invoked through the orchestrator's delegation
    // tools (SpawnAgent / DelegateTask / ParallelTasks). Each one sets a
    // distinct flag on the task context — read all three so future tools
    // and renames can't silently dodge the filter and leak chunks back
    // onto the orchestrator's metrics bar.
    let is_sub_agent = ["is_sub_agent", "is_delegation", "is_parallel_task"]
        .iter()
        .any(|key| {
            task.context
                .get(*key)
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        });

    // Defense-in-depth: a sub-agent must NEVER carry is_primary_agent: true.
    // The combination would let a delegated agent pass the
    // `check_primary_permission` gate inside SpawnAgent / DelegateTask /
    // ParallelTasks, opening a recursion-amplification path. The orchestrator
    // never sets both at the same time, but a future caller might forget
    // — in production downgrade to sub-agent privileges with a warn rather
    // than panic (debug builds assert).
    if is_sub_agent && is_primary_agent {
        warn!(
            agent_id = %ctx.config.id,
            "is_primary_agent=true on a sub-agent task — downgrading to sub-agent privileges",
        );
        debug_assert!(
            !(is_sub_agent && is_primary_agent),
            "is_primary_agent must be false for sub-agent tasks"
        );
    }
    let is_primary_agent = is_primary_agent && !is_sub_agent;

    let locale = task
        .context
        .get("locale")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Pre-allocated assistant message_id propagated from execution.rs via
    // build_task. Sub-agent tools persist it as `parent_message_id` on
    // sub_agent_execution at CREATE time (H2 audit 2026-05-02).
    let current_message_id = task
        .context
        .get("message_id")
        .and_then(|v| v.as_str())
        .map(String::from);

    let effective_context = match (ctx.agent_context, &cancellation_token) {
        (Some(agent_ctx), Some(token)) => {
            let mut ctx = agent_ctx.clone().with_cancellation_token(token.clone());
            if let Some(ref msg_id) = current_message_id {
                ctx = ctx.with_current_message_id(msg_id.clone());
            }
            Some(ctx)
        }
        (Some(agent_ctx), None) => {
            let ctx = if let Some(ref msg_id) = current_message_id {
                agent_ctx.clone().with_current_message_id(msg_id.clone())
            } else {
                agent_ctx.clone()
            };
            Some(ctx)
        }
        _ => None,
    };

    let mut local_tools = tools::create_local_tools(
        ctx.config,
        ctx.tool_factory,
        ctx.agent_context,
        workflow_id,
        is_primary_agent,
        effective_context.as_ref(),
    )
    .await;
    // Caller-injected tools (e.g. Submit*/ListAgents during compose/analyze)
    // join the local set so they appear in tool definitions, system prompt
    // and dispatch exactly like factory-resolved tools.
    local_tools.extend(extra_tools);

    let has_delegation_tools = ctx
        .config
        .tools
        .iter()
        .any(|t| t == "SpawnAgentTool" || t == "DelegateTaskTool" || t == "ParallelTasksTool");

    let (mcp_tools, mcp_server_summaries) = if let Some(ref mcp) = mcp_manager {
        let mcp_tool_defs = if !ctx.config.mcp_servers.is_empty() {
            tools::get_mcp_tool_definitions(ctx.config, mcp).await
        } else {
            Vec::new()
        };
        let summaries = if has_delegation_tools {
            tools::get_mcp_server_summaries(ctx.config, mcp).await
        } else {
            Vec::new()
        };
        (mcp_tool_defs, summaries)
    } else {
        (Vec::new(), Vec::new())
    };

    if local_tools.is_empty() && mcp_tools.is_empty() {
        debug!("No tools available, using basic execute");
        return execute_simple(
            ctx.config,
            ctx.provider_manager,
            ctx.agent_context,
            task,
            cancellation_token,
        )
        .await;
    }

    debug!(
        agent_name = %ctx.config.name,
        provider = adapter.provider_name(),
        local_tools_count = local_tools.len(),
        mcp_tools_count = mcp_tools.len(),
        mcp_servers_count = mcp_server_summaries.len(),
        "LLM Agent starting task execution with JSON function calling"
    );

    let tool_definitions = tools::collect_tool_definitions(&local_tools, &mcp_tools);
    let tools_json = adapter.format_tools(&tool_definitions);

    let system_prompt = prompt::build_system_prompt_with_tools(
        ctx.config,
        &local_tools,
        &mcp_tools,
        &mcp_server_summaries,
        locale.as_deref(),
        has_delegation_tools,
    );

    // In continuation mode, `task.description` mirrors the last user turn —
    // already persisted by the frontend and replayed via `conversation_messages`.
    // `build_initial_messages` deliberately does not re-append it (see its docstring).
    let mut messages = build_initial_messages(&task, system_prompt);

    // Tool execution loop
    let mut final_response_content = String::new();
    let mut iteration: usize = 0;
    let mut global_sequence: u32 = 0;
    let max_iterations = ctx.config.max_tool_iterations.clamp(1, 200);

    let call_ctx = tools::FunctionCallContext {
        local_tools: &local_tools,
        mcp_manager: mcp_manager.as_ref(),
        workflow_id: &event_workflow_id,
        validation_helper: validation_helper.as_ref(),
        require_file_confirmation: ctx.config.require_file_confirmation,
    };

    // Load the model pricing once so each iteration_progress chunk can carry
    // a per-call `cost_usd` that grows live alongside ENTREE/SORTIE. Avoids
    // N queries (1 per iteration). When no `tool_factory` is available (rare
    // test path) we skip the cache: the frontend gracefully falls back to the
    // final `response_block` cost.
    let pricing_cache = if let Some(factory) = ctx.tool_factory {
        Some(PricingCache::load(&factory.get_db(), ctx.config).await)
    } else {
        None
    };

    loop {
        iteration += 1;
        if iteration > max_iterations {
            warn!(
                iterations = max_iterations,
                "Max tool iterations reached, stopping execution"
            );
            global_sequence += 1;
            emit_reasoning(
                ctx.agent_context,
                &event_workflow_id,
                format!(
                    "Max tool iterations ({}) reached, stopping execution",
                    max_iterations
                ),
                start.elapsed().as_millis() as u64,
                global_sequence,
                ReasoningSource::AgentFlow,
                &mut reasoning_steps_data,
                Some(ctx.config.id.clone()),
                Some(ctx.config.name.clone()),
                is_sub_agent,
            );
            break;
        }

        // Cancellation gate between iterations: align with the existing check
        // around enforce_report. The in-flight LLM call inside run_single_iteration
        // is already cancellable, but a Continue outcome that lands here while
        // the user has just cancelled would otherwise spin one more iteration.
        if cancellation_token
            .as_ref()
            .is_some_and(|t| t.is_cancelled())
        {
            info!(
                iteration = iteration,
                "Cancellation detected between iterations, stopping tool loop"
            );
            let mut metrics = tokens.to_report_metrics(
                tools_used,
                mcp_calls_made,
                tool_executions_data,
                reasoning_steps_data,
                iteration_metrics_data,
            );
            metrics.duration_ms = start.elapsed().as_millis() as u64;
            return Ok(Report::failed_with_metrics(
                &ctx.config.id,
                &task.description,
                "cancelled".to_string(),
                metrics,
            ));
        }

        if iteration > 1 {
            global_sequence += 1;
            emit_reasoning(
                ctx.agent_context,
                &event_workflow_id,
                format!("Tool iteration {} - Processing tool results...", iteration),
                start.elapsed().as_millis() as u64,
                global_sequence,
                ReasoningSource::AgentFlow,
                &mut reasoning_steps_data,
                Some(ctx.config.id.clone()),
                Some(ctx.config.name.clone()),
                is_sub_agent,
            );
        }

        let inputs = IterationInputs {
            provider_type: &provider_type,
            adapter: adapter.as_ref(),
            tools_json: tools_json.as_slice(),
            event_workflow_id: &event_workflow_id,
            call_ctx: &call_ctx,
            start_instant: start,
            iteration,
            cancellation_token: cancellation_token.clone(),
            is_sub_agent,
            pricing_cache: pricing_cache.as_ref(),
            tool_choice: tool_choice_for_iteration(iteration, opening_tool_choice),
        };

        let mut mstate = IterationMutState {
            messages: &mut messages,
            tokens: &mut tokens,
            tools_used: &mut tools_used,
            mcp_calls_made: &mut mcp_calls_made,
            iteration_metrics_data: &mut iteration_metrics_data,
            tool_executions_data: &mut tool_executions_data,
            reasoning_steps_data: &mut reasoning_steps_data,
            global_sequence: &mut global_sequence,
        };

        match run_single_iteration(&ctx, &inputs, &mut mstate).await {
            IterationOutcome::Continue => {}
            IterationOutcome::Finished(content) => {
                final_response_content = content;
                break;
            }
            IterationOutcome::Failed(message) => {
                let mut metrics = tokens.to_report_metrics(
                    tools_used,
                    mcp_calls_made,
                    tool_executions_data,
                    reasoning_steps_data,
                    iteration_metrics_data,
                );
                metrics.duration_ms = start.elapsed().as_millis() as u64;
                return Ok(Report::failed_with_metrics(
                    &ctx.config.id,
                    &task.description,
                    message,
                    metrics,
                ));
            }
        }
    }

    // Report enforcement.
    if prompt::is_generic_completion_message(&final_response_content) && iteration > 1 {
        info!(
            original_response = %final_response_content,
            "Generic completion detected, requesting report from LLM"
        );

        let cancelled = cancellation_token
            .as_ref()
            .is_some_and(|t| t.is_cancelled());

        if !cancelled {
            let mut enforcement_state = EnforcementState {
                messages: &mut messages,
                tokens: &mut tokens,
                reasoning_steps: &mut reasoning_steps_data,
                iteration_metrics: &mut iteration_metrics_data,
                global_sequence: &mut global_sequence,
            };
            if let Some(enforced) = enforce_report(
                &ctx,
                &provider_type,
                adapter.as_ref(),
                &event_workflow_id,
                &mut enforcement_state,
                start.elapsed().as_millis() as u64,
                iteration,
                cancellation_token.clone(),
                is_sub_agent,
            )
            .await
            {
                final_response_content = enforced;
            }
        } else {
            debug!("Skipping report enforcement: workflow cancelled");
        }
    }

    let duration_ms = start.elapsed().as_millis() as u64;

    info!(
        iterations = iteration,
        provider = adapter.provider_name(),
        tools_used_count = tools_used.len(),
        mcp_calls_count = mcp_calls_made.len(),
        total_tokens_input = tokens.total_input,
        total_tokens_output = tokens.total_output,
        total_cached_tokens = ?tokens.total_cached,
        duration_ms = duration_ms,
        "LLM Agent task execution with tools completed"
    );

    let content = build_report_content(&ReportContentInputs {
        agent_id: &ctx.config.id,
        task_description: &task.description,
        final_response_content: &final_response_content,
        provider_type: &provider_type,
        model: &ctx.config.llm.model,
        total_tokens_input: tokens.total_input,
        total_tokens_output: tokens.total_output,
        duration_ms,
        iteration,
        tools_used: &tools_used,
        mcp_calls_made: &mcp_calls_made,
    });

    let mut metrics = tokens.to_report_metrics(
        tools_used,
        mcp_calls_made,
        tool_executions_data,
        reasoning_steps_data,
        iteration_metrics_data,
    );
    metrics.duration_ms = duration_ms;

    Ok(Report {
        status: ReportStatus::Success,
        content,
        response: final_response_content,
        metrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_tool_choice_only_applies_to_first_iteration() {
        // Required is honoured on the opening turn so the model must emit a
        // tool call (Kanban analyze / compose root cause: model writes prose
        // and finishes without ever submitting).
        assert_eq!(
            tool_choice_for_iteration(1, ToolChoiceMode::Required),
            ToolChoiceMode::Required
        );
        // Subsequent turns fall back to Auto so the loop can terminate once
        // the model has submitted — a blanket Required would never let any
        // turn be tool-free, spinning until max_iterations.
        assert_eq!(
            tool_choice_for_iteration(2, ToolChoiceMode::Required),
            ToolChoiceMode::Auto
        );
        assert_eq!(
            tool_choice_for_iteration(50, ToolChoiceMode::Required),
            ToolChoiceMode::Auto
        );
    }

    #[test]
    fn opening_tool_choice_auto_stays_auto_every_iteration() {
        // The standard workflow path passes Auto and must never be forced.
        for iteration in [1usize, 2, 10] {
            assert_eq!(
                tool_choice_for_iteration(iteration, ToolChoiceMode::Auto),
                ToolChoiceMode::Auto
            );
        }
    }

    fn make_task(description: &str, context: serde_json::Value) -> Task {
        Task {
            id: "test-task".to_string(),
            description: description.to_string(),
            context,
        }
    }

    #[test]
    fn test_build_initial_messages_first_call() {
        let task = make_task(
            "Mon nom est Bob",
            serde_json::json!({
                "is_primary_agent": true,
                "workflow_id": "wf-1",
                "locale": "fr",
            }),
        );

        let msgs = build_initial_messages(&task, "SYSTEM PROMPT".to_string());

        assert_eq!(msgs.len(), 2, "First call must produce [system, user]");
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "SYSTEM PROMPT");
        assert_eq!(msgs[1]["role"], "user");
        // build_prompt wraps task.description with optional context, but our
        // context contains no `conversation_history` and only is_primary_agent/
        // workflow_id/locale -> they appear as "Context: ```json{...}```".
        // We just assert the user content contains the original description.
        let user_content = msgs[1]["content"].as_str().unwrap();
        assert!(
            user_content.contains("Mon nom est Bob"),
            "User content must contain task.description, got: {}",
            user_content
        );
    }

    #[test]
    fn test_build_initial_messages_continuation_no_duplication() {
        let history = serde_json::json!([
            {"role": "user", "content": "Mon nom est Bob"},
            {"role": "assistant", "content": "Enchante Bob"},
            {"role": "user", "content": "Comment je m'appelle?"},
        ]);
        let task = make_task(
            "Comment je m'appelle?",
            serde_json::json!({
                "conversation_messages": history,
                "is_primary_agent": true,
                "workflow_id": "wf-1",
            }),
        );

        let msgs = build_initial_messages(&task, "REGEN SYSTEM".to_string());

        assert_eq!(
            msgs.len(),
            4,
            "Continuation must produce [system, ...history] (no extra user append)"
        );
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "REGEN SYSTEM");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[1]["content"], "Mon nom est Bob");
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[2]["content"], "Enchante Bob");
        assert_eq!(msgs[3]["role"], "user");
        assert_eq!(msgs[3]["content"], "Comment je m'appelle?");

        // Defense-in-depth: the last user content must appear exactly once.
        let occurrences = msgs
            .iter()
            .filter(|m| m["content"].as_str() == Some("Comment je m'appelle?"))
            .count();
        assert_eq!(
            occurrences, 1,
            "Current user message must NOT be duplicated"
        );
    }

    #[test]
    fn test_build_initial_messages_empty_history_fallback() {
        // Empty array -> fall back to first-call behavior.
        let task = make_task(
            "Premier tour",
            serde_json::json!({
                "conversation_messages": [],
                "workflow_id": "wf-1",
            }),
        );

        let msgs = build_initial_messages(&task, "SYSTEM PROMPT".to_string());

        assert_eq!(
            msgs.len(),
            2,
            "Empty conversation_messages must trigger first-call fallback"
        );
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
        let user_content = msgs[1]["content"].as_str().unwrap();
        assert!(user_content.contains("Premier tour"));
    }

    #[test]
    fn test_build_initial_messages_missing_context_key() {
        // No conversation_messages key at all -> first-call.
        let task = make_task("Hello", serde_json::json!({"workflow_id": "wf-1"}));

        let msgs = build_initial_messages(&task, "SP".to_string());
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0]["content"], "SP");
    }

    #[test]
    fn test_build_initial_messages_continuation_preserves_order() {
        let history = serde_json::json!([
            {"role": "user", "content": "1"},
            {"role": "assistant", "content": "2"},
            {"role": "user", "content": "3"},
            {"role": "assistant", "content": "4"},
            {"role": "user", "content": "5"},
        ]);
        let task = make_task("5", serde_json::json!({"conversation_messages": history}));

        let msgs = build_initial_messages(&task, "S".to_string());

        let contents: Vec<&str> = msgs[1..]
            .iter()
            .map(|m| m["content"].as_str().unwrap())
            .collect();
        assert_eq!(contents, vec!["1", "2", "3", "4", "5"]);
    }

    // =========================================================================
    // PricingCache — live cost during streaming.
    //
    // Cover the three call paths driving the live-cost feature added so the
    // TokenDisplay metrics bar grows progressively (`~ X$`) instead of
    // jumping from 0 to the final value at the very end:
    //   1. model seeded -> load() returns Some(row)
    //   2. model absent  -> load() returns None
    //   3. compute_iteration_local_cost honours the cached pricing
    // =========================================================================

    use crate::models::LLMConfig;
    use crate::test_utils::{seed_llm_model, setup_test_state};

    fn make_agent_config(provider: &str, model: &str) -> AgentConfig {
        AgentConfig {
            id: "test-agent".to_string(),
            name: "Test".to_string(),
            lifecycle: crate::models::Lifecycle::Permanent,
            llm: LLMConfig {
                provider: provider.to_string(),
                model: model.to_string(),
                temperature: 0.7,
                max_tokens: 1024,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: false,
            system_prompt: String::new(),
            max_tool_iterations: 10,
            reasoning_effort: None,
            kind: None,
            auto_analyze_reports: false,
        }
    }

    #[tokio::test]
    async fn pricing_cache_loads_row_when_model_seeded() {
        let (state, _guard) = setup_test_state().await;
        seed_llm_model(&state.db, "Mistral", "mistral-medium", 2.0, 6.0).await;

        let cache =
            PricingCache::load(&state.db, &make_agent_config("Mistral", "mistral-medium")).await;

        let row = cache.pricing.expect("seeded llm_model row must be cached");
        assert!((row.input_price_per_mtok - 2.0).abs() < 1e-9);
        assert!((row.output_price_per_mtok - 6.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn pricing_cache_load_returns_none_when_model_absent() {
        // Defensive: tool loop must keep working when the agent's model is
        // not yet registered in `llm_model` — the chunk's `cost_usd` simply
        // stays `None` and the frontend falls back to the final cost.
        let (state, _guard) = setup_test_state().await;

        let cache =
            PricingCache::load(&state.db, &make_agent_config("Custom", "unknown-model")).await;

        assert!(cache.pricing.is_none());
    }

    #[test]
    fn compute_iteration_local_cost_returns_none_when_pricing_absent() {
        let cache = PricingCache { pricing: None };

        let cost = cache.compute_iteration_local_cost(1_000, 500, None, None);

        assert!(
            cost.is_none(),
            "absent pricing -> None so the wire chunk omits cost_usd"
        );
    }

    #[test]
    fn compute_iteration_local_cost_uses_cached_pricing() {
        // 1k input * $2/MTok + 500 output * $6/MTok = 0.002 + 0.003 = $0.005
        let cache = PricingCache {
            pricing: Some(ModelPricingRow {
                model_id: "mid".to_string(),
                input_price_per_mtok: 2.0,
                output_price_per_mtok: 6.0,
                cache_read_price_per_mtok: 0.0,
                cache_write_price_per_mtok: 0.0,
            }),
        };

        let cost = cache
            .compute_iteration_local_cost(1_000, 500, None, None)
            .expect("Some pricing -> Some(cost)");

        assert!(
            (cost - 0.005).abs() < 1e-9,
            "expected $0.005 for 1k in / 500 out at $2/$6 MTok, got ${}",
            cost
        );
    }

    #[test]
    fn compute_iteration_local_cost_propagates_cache_savings() {
        // 80% of input served from cache @ 50% of input price ->
        // 200 regular * $2/M + 800 cache-read * $1/M + 100 output * $6/M
        // = 0.0004 + 0.0008 + 0.0006 = $0.0018
        let cache = PricingCache {
            pricing: Some(ModelPricingRow {
                model_id: "mid".to_string(),
                input_price_per_mtok: 2.0,
                output_price_per_mtok: 6.0,
                cache_read_price_per_mtok: 1.0,
                cache_write_price_per_mtok: 0.0,
            }),
        };

        let cost = cache
            .compute_iteration_local_cost(1_000, 100, Some(800), None)
            .expect("Some pricing -> Some(cost)");

        assert!(
            (cost - 0.0018).abs() < 1e-9,
            "expected $0.0018 with cache savings, got ${}",
            cost
        );
    }
}
