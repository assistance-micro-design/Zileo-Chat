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
use crate::llm::adapters::{MistralToolAdapter, OllamaToolAdapter, OpenAiToolAdapter};
use crate::llm::tool_adapter::{ProviderToolAdapter, TokenUsage};
use crate::llm::{CompletionParams, ProviderManager, ProviderType};
use crate::mcp::MCPManager;
use crate::models::streaming::StreamChunk;
use crate::models::workflow::IterationMetrics;
use crate::models::AgentConfig;
use crate::tools::{context::AgentToolContext, validation_helper::ValidationHelper, ToolFactory};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Tracks cumulative and per-iteration token usage across the tool loop.
pub(crate) struct TokenTracker {
    pub total_input: usize,
    pub total_output: usize,
    /// Last call's input tokens (context window size)
    pub context: usize,
    pub total_cached: Option<usize>,
    pub total_cache_write: Option<usize>,
    pub total_thinking: Option<usize>,
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
        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": base_prompt}),
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

        let inputs = IterationInputs {
            provider_type: &provider_type,
            adapter: adapter.as_ref(),
            tools_json: tools_json.as_slice(),
            event_workflow_id: &event_workflow_id,
            call_ctx: &call_ctx,
            start_instant: start,
            iteration,
            cancellation_token: cancellation_token.clone(),
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
}
