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
//! Runners: `execute_simple` and the full `execute_with_tools` loop.
use super::context::ToolLoopContext;
use super::init::build_initial_messages;
use super::metrics::{PricingCache, TokenTracker};
use super::policy::{load_supports_forced_tool_choice, tool_choice_for_iteration};
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
use crate::llm::adapters::{MistralToolAdapter, OllamaToolAdapter, OpenAiToolAdapter};
use crate::llm::tool_adapter::ProviderToolAdapter;
use crate::llm::{CompletionParams, ProviderManager, ProviderType};
use crate::mcp::MCPManager;
use crate::models::function_calling::ToolChoiceMode;
use crate::models::streaming::StreamChunk;
use crate::models::workflow::IterationMetrics;
use crate::models::AgentConfig;
use crate::tools::{context::AgentToolContext, validation_helper::ValidationHelper, Tool};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

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
    // Cumulative serialized size of successful MCP results across the
    // whole run, gating the per-run byte budget alongside `mcp_calls_made`.
    let mut mcp_result_bytes: usize = 0;
    // Run-scoped count of *Manager content/privilege writes, gating the
    // per-run write cap in `manager_write_gate` (self-grants included).
    let mut manager_writes_made: usize = 0;
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

    // Stamp the tool loop's detached status onto the context handed to the
    // sub-agent tools (Spawn / Delegate / Parallel). This is the single source
    // of truth: every detached caller (rerun_worker, analyze, compose) already
    // sets `ToolLoopContext::is_detached`, so the sub-agent tasks those tools
    // build inherit it (transitive gate) without each caller having to
    // remember a second flag on the AgentToolContext.
    let loop_is_detached = ctx.is_detached;
    let effective_context = match (ctx.agent_context, &cancellation_token) {
        (Some(agent_ctx), Some(token)) => {
            let mut ec = agent_ctx
                .clone()
                .with_cancellation_token(token.clone())
                .with_detached(loop_is_detached);
            if let Some(ref msg_id) = current_message_id {
                ec = ec.with_current_message_id(msg_id.clone());
            }
            Some(ec)
        }
        (Some(agent_ctx), None) => {
            let mut ec = agent_ctx.clone().with_detached(loop_is_detached);
            if let Some(ref msg_id) = current_message_id {
                ec = ec.with_current_message_id(msg_id.clone());
            }
            Some(ec)
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
        is_detached: ctx.is_detached,
        is_delegated: ctx.is_delegated,
        mcp_tool_allowlist: &ctx.config.mcp_tool_allowlist,
        agent_skills: &ctx.config.skills,
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

    // Resolve the model's forced-tool-choice capability once so the opening
    // turn can downgrade `Required` to `Auto` for upstreams that reject a
    // forced `tool_choice` (deepseek-v4 via RouterLab). Defaults to true (the
    // historical behaviour) when no `tool_factory` is available (rare test
    // path) or the model card is absent.
    let model_supports_forced_tool_choice = match ctx.tool_factory {
        Some(factory) => {
            load_supports_forced_tool_choice(
                &factory.get_db(),
                &ctx.config.llm.model,
                &ctx.config.llm.provider,
            )
            .await
        }
        None => true,
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
            tool_choice: tool_choice_for_iteration(
                iteration,
                opening_tool_choice,
                model_supports_forced_tool_choice,
            ),
        };

        let mut mstate = IterationMutState {
            messages: &mut messages,
            tokens: &mut tokens,
            tools_used: &mut tools_used,
            mcp_calls_made: &mut mcp_calls_made,
            mcp_result_bytes: &mut mcp_result_bytes,
            manager_writes_made: &mut manager_writes_made,
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
