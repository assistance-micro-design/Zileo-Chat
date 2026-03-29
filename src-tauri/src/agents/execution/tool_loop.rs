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
//! Contains the main execution logic for both simple (no tools) and
//! tool-augmented (local + MCP) agent execution paths.

use crate::agents::core::agent::{
    ReasoningSource, ReasoningStepData, Report, ReportMetrics, ReportStatus, Task,
    ToolExecutionData,
};
use crate::agents::execution::tools;
use crate::agents::prompt::{self, REPORT_ENFORCEMENT_PROMPT};
use crate::llm::adapters::{MistralToolAdapter, OllamaToolAdapter, OpenAiToolAdapter};
use crate::llm::tool_adapter::{ProviderToolAdapter, TokenUsage};
use crate::llm::{CompletionParams, LLMError, ProviderManager, ProviderType, ToolCompletionParams};
use crate::mcp::MCPManager;
use crate::models::agent::ReasoningEffort;
use crate::models::function_calling::ToolChoiceMode;
use crate::models::streaming::{events, StreamChunk};
use crate::models::workflow::IterationMetrics;
use crate::models::AgentConfig;
use crate::tools::{context::AgentToolContext, validation_helper::ValidationHelper, ToolFactory};
use std::sync::Arc;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Tracks cumulative and per-iteration token usage across the tool loop.
struct TokenTracker {
    total_input: usize,
    total_output: usize,
    /// Last call's input tokens (context window size)
    context: usize,
    total_cached: Option<usize>,
    total_cache_write: Option<usize>,
    total_thinking: Option<usize>,
    // Per-iteration values (overwritten each iteration, read for IterationMetrics)
    iter_input: usize,
    iter_output: usize,
    iter_cached: Option<usize>,
    iter_cache_write: Option<usize>,
    iter_thinking: Option<usize>,
}

impl TokenTracker {
    fn new() -> Self {
        Self {
            total_input: 0,
            total_output: 0,
            context: 0,
            total_cached: None,
            total_cache_write: None,
            total_thinking: None,
            iter_input: 0,
            iter_output: 0,
            iter_cached: None,
            iter_cache_write: None,
            iter_thinking: None,
        }
    }

    /// Records token usage from an LLM response, updating both per-iteration and cumulative values.
    fn record(&mut self, usage: &TokenUsage) {
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
    }

    /// Adds estimated thinking tokens (fallback when provider doesn't report them).
    fn add_estimated_thinking(&mut self, estimated: usize) {
        self.iter_thinking = Some(estimated);
        Self::accumulate(&mut self.total_thinking, Some(estimated));
    }

    fn accumulate(total: &mut Option<usize>, value: Option<usize>) {
        if let Some(val) = value {
            *total = Some(total.unwrap_or(0) + val);
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
            tools_used,
            mcp_calls,
            tool_executions,
            reasoning_steps,
            iteration_metrics,
        }
    }
}

/// Formats an LLMError into a user-friendly error message.
fn format_llm_error(error: &LLMError) -> String {
    match error {
        LLMError::ConnectionError(msg) => {
            format!(
                "Connection error: {}\n\nMake sure the LLM service is running and accessible.",
                msg
            )
        }
        LLMError::ModelNotFound(msg) => format!("Model not found: {}", msg),
        LLMError::MissingApiKey(provider) => {
            format!(
                "API key missing for {}. Please configure it in Settings.",
                provider
            )
        }
        LLMError::RequestFailed(msg) => format!("Request failed: {}", msg),
        _ => error.to_string(),
    }
}

/// Returns the effective reasoning effort based on model capability.
///
/// When `is_reasoning` is true but no explicit effort is set, defaults to Medium.
/// This ensures reasoning models always use the thinking path (important for Ollama
/// where the `think` parameter controls separate thinking extraction).
fn effective_reasoning_effort(config: &AgentConfig) -> Option<ReasoningEffort> {
    if config.llm.is_reasoning {
        Some(
            config
                .reasoning_effort
                .clone()
                .unwrap_or(ReasoningEffort::Medium),
        )
    } else {
        None
    }
}

/// Emits a streaming event to the frontend via Tauri.
fn emit_progress(agent_context: Option<&AgentToolContext>, chunk: StreamChunk) {
    if let Some(context) = agent_context {
        if let Some(ref handle) = context.app_handle {
            if let Err(e) = handle.emit(events::WORKFLOW_STREAM, &chunk) {
                warn!(error = %e, "Failed to emit LLM agent progress event");
            }
        }
    }
}

/// Emits a reasoning step and records it.
fn emit_reasoning(
    agent_context: Option<&AgentToolContext>,
    event_workflow_id: &str,
    content: String,
    elapsed_ms: u64,
    sequence: u32,
    source: ReasoningSource,
    steps: &mut Vec<ReasoningStepData>,
) {
    emit_progress(
        agent_context,
        StreamChunk::reasoning(event_workflow_id.to_string(), content.clone()),
    );
    steps.push(ReasoningStepData {
        content,
        duration_ms: elapsed_ms,
        sequence,
        source,
    });
}

/// Builds the tools section for the final markdown report.
fn build_tools_section(tools_used: &[String], mcp_calls_made: &[String]) -> String {
    if tools_used.is_empty() && mcp_calls_made.is_empty() {
        return String::new();
    }

    let local_used = if !tools_used.is_empty() {
        format!(
            "\n### Local Tools Used\n{}",
            tools_used
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        String::new()
    };

    let mcp_used = if !mcp_calls_made.is_empty() {
        format!(
            "\n### MCP Tools Called\n{}",
            mcp_calls_made
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        String::new()
    };

    format!("\n\n## Tool Usage{}{}", local_used, mcp_used)
}

/// Mutable state passed to report enforcement to avoid too many arguments.
struct EnforcementState<'a> {
    messages: &'a mut Vec<serde_json::Value>,
    tokens: &'a mut TokenTracker,
    reasoning_steps: &'a mut Vec<ReasoningStepData>,
    iteration_metrics: &'a mut Vec<IterationMetrics>,
    global_sequence: &'a mut u32,
}

/// Makes a follow-up LLM call to get a proper report when the agent finished
/// with a generic completion message.
async fn enforce_report(
    ctx: &ToolLoopContext<'_>,
    provider_type: &ProviderType,
    adapter: &dyn ProviderToolAdapter,
    event_workflow_id: &str,
    state: &mut EnforcementState<'_>,
    elapsed_ms: u64,
    iteration: usize,
) -> Option<String> {
    *state.global_sequence += 1;
    emit_reasoning(
        ctx.agent_context,
        event_workflow_id,
        "Agent completed tools without a report. Requesting summary...".to_string(),
        elapsed_ms,
        *state.global_sequence,
        ReasoningSource::AgentFlow,
        state.reasoning_steps,
    );

    state.messages.push(serde_json::json!({
        "role": "user",
        "content": REPORT_ENFORCEMENT_PROMPT
    }));

    let empty_tools: Vec<serde_json::Value> = vec![];
    let report_iter_start = std::time::Instant::now();

    let result = ctx
        .provider_manager
        .complete_with_tools(
            provider_type.clone(),
            ToolCompletionParams {
                messages: state.messages.clone(),
                tools: empty_tools,
                tool_choice: None,
                model: ctx.config.llm.model.clone(),
                temperature: ctx.config.llm.temperature,
                max_tokens: ctx.config.llm.max_tokens,
                context_window: ctx.config.llm.context_window,
                reasoning_effort: effective_reasoning_effort(ctx.config),
            },
        )
        .await;

    let mut enforced_content = None;

    match result {
        Ok(response) => {
            let usage = adapter.extract_usage(&response);
            state.tokens.record(&usage);

            if let Some(content) = adapter.extract_content(&response) {
                if !content.trim().is_empty() {
                    info!("Report enforcement successful, received meaningful response");
                    enforced_content = Some(content);
                } else {
                    warn!("Report enforcement returned empty content, keeping generic message");
                }
            }
        }
        Err(e) => {
            warn!(error = %e, "Report enforcement LLM call failed, keeping generic message");
        }
    }

    state.iteration_metrics.push(IterationMetrics {
        iteration: (iteration + 1) as u32,
        tokens_input: state.tokens.iter_input,
        tokens_output: state.tokens.iter_output,
        cached_tokens: state.tokens.iter_cached,
        cache_write_tokens: state.tokens.iter_cache_write,
        thinking_tokens: state.tokens.iter_thinking,
        messages_count: state.messages.len(),
        tool_calls_count: 0,
        duration_ms: report_iter_start.elapsed().as_millis() as u64,
    });

    enforced_content
}

/// Executes a task without tools (simple LLM completion).
pub(crate) async fn execute_simple(
    config: &AgentConfig,
    provider_manager: &ProviderManager,
    agent_context: Option<&AgentToolContext>,
    task: Task,
) -> anyhow::Result<Report> {
    let start = std::time::Instant::now();

    debug!(
        agent_name = %config.name,
        system_prompt_len = config.system_prompt.len(),
        "LLM Agent starting simple task execution"
    );

    let user_prompt = prompt::build_prompt(&task);

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
        .complete_with_provider(
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
                        StreamChunk::thinking_block(event_workflow_id.clone(), thinking.clone()),
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
                    cached_tokens: None,
                    cache_write_tokens: None,
                    thinking_tokens: response.thinking_tokens,
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

/// Context for the tool execution loop, grouping all dependencies.
pub(crate) struct ToolLoopContext<'a> {
    pub config: &'a AgentConfig,
    pub provider_manager: &'a ProviderManager,
    pub tool_factory: Option<&'a Arc<ToolFactory>>,
    pub agent_context: Option<&'a AgentToolContext>,
}

/// Executes a task with full tool support (local + MCP) using JSON function calling.
pub(crate) async fn execute_with_tools(
    ctx: ToolLoopContext<'_>,
    task: Task,
    mcp_manager: Option<Arc<MCPManager>>,
    cancellation_token: Option<CancellationToken>,
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

    let locale = task
        .context
        .get("locale")
        .and_then(|v| v.as_str())
        .map(String::from);

    let effective_context = match (ctx.agent_context, &cancellation_token) {
        (Some(agent_ctx), Some(token)) => {
            Some(agent_ctx.clone().with_cancellation_token(token.clone()))
        }
        _ => None,
    };

    let local_tools = tools::create_local_tools(
        ctx.config,
        ctx.tool_factory,
        ctx.agent_context,
        workflow_id,
        is_primary_agent,
        effective_context.as_ref(),
    )
    .await;

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
        return execute_simple(ctx.config, ctx.provider_manager, ctx.agent_context, task).await;
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

    let existing_messages = task
        .context
        .get("conversation_messages")
        .and_then(|v| v.as_array())
        .cloned();

    let mut messages: Vec<serde_json::Value> = if let Some(existing) = existing_messages {
        let mut msgs: Vec<serde_json::Value> = existing;
        msgs.push(serde_json::json!({
            "role": "user",
            "content": task.description
        }));
        debug!(
            existing_messages_count = msgs.len() - 1,
            "Continuing conversation with existing context"
        );
        msgs
    } else {
        let system_prompt = prompt::build_system_prompt_with_tools(
            ctx.config,
            &local_tools,
            &mcp_tools,
            &mcp_server_summaries,
            locale.as_deref(),
            has_delegation_tools,
        );
        let base_prompt = prompt::build_prompt(&task);
        let msgs = vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": base_prompt}),
        ];
        debug!("First message: building new system prompt with tools");
        msgs
    };

    // Tool execution loop
    let mut final_response_content = String::new();
    let mut iteration = 0;
    let mut global_sequence: u32 = 0;
    let max_iterations = ctx.config.max_tool_iterations.clamp(1, 200);

    let call_ctx = tools::FunctionCallContext {
        local_tools: &local_tools,
        mcp_manager: mcp_manager.as_ref(),
        workflow_id: &event_workflow_id,
        validation_helper: validation_helper.as_ref(),
        require_file_confirmation: ctx.config.require_file_confirmation,
    };

    loop {
        iteration += 1;
        let iter_start = std::time::Instant::now();
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
            );
            break;
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
            );
        }

        debug!(
            iteration = iteration,
            messages_count = messages.len(),
            "Executing LLM call with JSON function calling"
        );

        // Execute LLM call with tools
        let response = match ctx
            .provider_manager
            .complete_with_tools(
                provider_type.clone(),
                ToolCompletionParams {
                    messages: messages.clone(),
                    tools: tools_json.clone(),
                    tool_choice: Some(adapter.get_tool_choice(ToolChoiceMode::Auto)),
                    model: ctx.config.llm.model.clone(),
                    temperature: ctx.config.llm.temperature,
                    max_tokens: ctx.config.llm.max_tokens,
                    context_window: ctx.config.llm.context_window,
                    reasoning_effort: effective_reasoning_effort(ctx.config),
                },
            )
            .await
        {
            Ok(r) => {
                let usage = adapter.extract_usage(&r);
                tokens.record(&usage);

                debug!(
                    iteration = iteration,
                    input_tokens = usage.input_tokens,
                    output_tokens = usage.output_tokens,
                    cached_tokens = ?usage.cached_tokens,
                    total_input = tokens.total_input,
                    total_output = tokens.total_output,
                    "Token usage - input_tokens is this call, total_input is cumulative"
                );

                r
            }
            Err(e) => {
                error!(error = %e, iteration = iteration, "LLM call with tools failed");
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
                    format_llm_error(&e),
                    metrics,
                ));
            }
        };

        // Handle thinking content
        if let Some(thinking) = adapter.extract_thinking(&response) {
            if !thinking.trim().is_empty() {
                if tokens.iter_thinking.is_none() {
                    let estimated = crate::llm::utils::estimate_tokens(&thinking);
                    tokens.add_estimated_thinking(estimated);
                }
                global_sequence += 1;
                emit_progress(
                    ctx.agent_context,
                    StreamChunk::thinking_block(event_workflow_id.clone(), thinking.clone()),
                );
                reasoning_steps_data.push(ReasoningStepData {
                    content: thinking,
                    duration_ms: start.elapsed().as_millis() as u64,
                    sequence: global_sequence,
                    source: ReasoningSource::ModelThinking,
                });
            }
        }

        // Parse tool calls from response
        let function_calls = if adapter.has_tool_calls(&response) {
            adapter.parse_tool_calls(&response)
        } else {
            Vec::new()
        };

        // Record per-iteration metrics
        iteration_metrics_data.push(IterationMetrics {
            iteration: iteration as u32,
            tokens_input: tokens.iter_input,
            tokens_output: tokens.iter_output,
            cached_tokens: tokens.iter_cached,
            cache_write_tokens: tokens.iter_cache_write,
            thinking_tokens: tokens.iter_thinking,
            messages_count: messages.len(),
            tool_calls_count: function_calls.len(),
            duration_ms: iter_start.elapsed().as_millis() as u64,
        });

        // No tool calls = finished
        if function_calls.is_empty() {
            if let Some(content) = adapter.extract_content(&response) {
                if !content.trim().is_empty() {
                    final_response_content = content;
                } else {
                    warn!(
                        iteration = iteration,
                        "LLM returned empty content, treating as task completion"
                    );
                    final_response_content = format!(
                        "Task completed after {} iteration(s). Tool executions completed successfully.",
                        iteration
                    );
                }
            } else {
                final_response_content = format!(
                    "Task completed after {} iteration(s). Tool executions completed successfully.",
                    iteration
                );
            }
            debug!(
                iteration = iteration,
                provider = adapter.provider_name(),
                finished = adapter.is_finished(&response),
                "No tool calls found, finishing"
            );
            break;
        }

        info!(
            iteration = iteration,
            tool_calls_count = function_calls.len(),
            "Found tool calls, executing"
        );

        // Emit progress about found tool calls
        let tool_names: Vec<String> = function_calls.iter().map(|c| c.name.clone()).collect();
        global_sequence += 1;
        emit_reasoning(
            ctx.agent_context,
            &event_workflow_id,
            format!(
                "Executing {} tool(s): {}",
                function_calls.len(),
                tool_names.join(", ")
            ),
            start.elapsed().as_millis() as u64,
            global_sequence,
            ReasoningSource::AgentFlow,
            &mut reasoning_steps_data,
        );

        // Add assistant message with tool calls
        messages.push(adapter.build_assistant_message(&response));

        // Execute each function call
        for call in &function_calls {
            let exec_start = std::time::Instant::now();

            emit_progress(
                ctx.agent_context,
                StreamChunk::tool_start(event_workflow_id.clone(), call.name.clone()),
            );

            let result =
                tools::execute_function_call(call, &call_ctx, &mut tools_used, &mut mcp_calls_made)
                    .await;

            // Capture detailed execution data
            let exec_duration = exec_start.elapsed().as_millis() as u64;
            let tool_type = if call.is_mcp_tool() { "mcp" } else { "local" };
            let (server_name, tool_name_for_data) =
                if let Some((server, tool)) = call.parse_mcp_name() {
                    (Some(server.to_string()), tool.to_string())
                } else {
                    (None, call.name.clone())
                };

            global_sequence += 1;
            tool_executions_data.push(ToolExecutionData {
                tool_type: tool_type.to_string(),
                tool_name: tool_name_for_data.clone(),
                server_name: server_name.clone(),
                input_params: call.arguments.clone(),
                output_result: result.result.clone(),
                success: result.success,
                error_message: result.error.clone(),
                duration_ms: exec_duration,
                iteration: iteration as u32,
                sequence: global_sequence,
            });

            let input_json =
                serde_json::to_string(&call.arguments).unwrap_or_else(|_| "{}".to_string());
            let output_json =
                serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".to_string());
            emit_progress(
                ctx.agent_context,
                StreamChunk::tool_call_complete(
                    event_workflow_id.clone(),
                    tool_name_for_data,
                    tool_type,
                    server_name,
                    exec_duration,
                    input_json,
                    output_json,
                    result.success,
                ),
            );

            messages.push(adapter.format_tool_result(&result));
        }
    }

    // Report enforcement
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

    let tools_section = build_tools_section(&tools_used, &mcp_calls_made);

    let content = format!(
        "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Response\n\n{}\n\n## Metrics\n- Provider: {}\n- Model: {}\n- Tokens (input/output): {}/{}\n- Duration: {}ms\n- Tool iterations: {}{}",
        ctx.config.id,
        task.description,
        final_response_content,
        provider_type,
        ctx.config.llm.model,
        tokens.total_input,
        tokens.total_output,
        duration_ms,
        iteration,
        tools_section
    );

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
