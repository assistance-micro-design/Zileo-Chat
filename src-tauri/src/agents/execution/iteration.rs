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

//! Single-iteration logic of the agent tool loop.
//!
//! [`run_single_iteration`] performs one pass of: LLM call (cancellable),
//! thinking extraction, tool-call parsing, per-iteration metrics, and tool
//! execution. The outer orchestrator in [`tool_loop`] decides whether to
//! loop again or finish based on the returned [`IterationOutcome`].

use crate::agents::core::agent::{ReasoningSource, ReasoningStepData, ToolExecutionData};
use crate::agents::execution::reasoning::{
    effective_reasoning_effort, emit_progress, emit_reasoning, format_llm_error,
};
use crate::agents::execution::tool_loop::{TokenTracker, ToolLoopContext};
use crate::agents::execution::tools;
use crate::llm::tool_adapter::ProviderToolAdapter;
use crate::llm::{ProviderType, ToolCompletionParams};
use crate::models::function_calling::ToolChoiceMode;
use crate::models::streaming::StreamChunk;
use crate::models::workflow::IterationMetrics;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

/// Outcome of a single iteration of the tool loop.
pub(crate) enum IterationOutcome {
    /// LLM responded without tool calls — content is the final assistant text.
    Finished(String),
    /// LLM emitted tool calls — caller continues to the next iteration.
    Continue,
    /// LLM call failed — caller should bail out with this error message.
    Failed(String),
}

/// All mutable state mutated by a single iteration.
pub(crate) struct IterationMutState<'a> {
    pub messages: &'a mut Vec<serde_json::Value>,
    pub tokens: &'a mut TokenTracker,
    pub tools_used: &'a mut Vec<String>,
    pub mcp_calls_made: &'a mut Vec<String>,
    pub iteration_metrics_data: &'a mut Vec<IterationMetrics>,
    pub tool_executions_data: &'a mut Vec<ToolExecutionData>,
    pub reasoning_steps_data: &'a mut Vec<ReasoningStepData>,
    pub global_sequence: &'a mut u32,
}

/// Read-only inputs that don't change between iterations.
pub(crate) struct IterationInputs<'a> {
    pub provider_type: &'a ProviderType,
    pub adapter: &'a dyn ProviderToolAdapter,
    pub tools_json: &'a [serde_json::Value],
    pub event_workflow_id: &'a str,
    pub call_ctx: &'a tools::FunctionCallContext<'a>,
    pub start_instant: std::time::Instant,
    pub iteration: usize,
    pub cancellation_token: Option<CancellationToken>,
}

/// Runs a single iteration of the tool loop.
pub(crate) async fn run_single_iteration(
    ctx: &ToolLoopContext<'_>,
    inputs: &IterationInputs<'_>,
    mstate: &mut IterationMutState<'_>,
) -> IterationOutcome {
    let iter_start = std::time::Instant::now();

    debug!(
        iteration = inputs.iteration,
        messages_count = mstate.messages.len(),
        "Executing LLM call with JSON function calling"
    );

    // cancellable LLM call — drops the future on cancel, which
    // tears down the in-flight HTTP request at the TCP level.
    let response = match ctx
        .provider_manager
        .complete_with_tools_cancellable(
            inputs.provider_type.clone(),
            ToolCompletionParams {
                messages: mstate.messages.clone(),
                tools: inputs.tools_json.to_vec(),
                tool_choice: Some(inputs.adapter.get_tool_choice(ToolChoiceMode::Auto)),
                model: ctx.config.llm.model.clone(),
                temperature: ctx.config.llm.temperature,
                max_tokens: ctx.config.llm.max_tokens,
                context_window: ctx.config.llm.context_window,
                reasoning_effort: effective_reasoning_effort(ctx.config),
            },
            inputs.cancellation_token.clone(),
        )
        .await
    {
        Ok(r) => {
            let usage = inputs.adapter.extract_usage(&r);
            mstate.tokens.record(&usage);

            debug!(
                iteration = inputs.iteration,
                input_tokens = usage.input_tokens,
                output_tokens = usage.output_tokens,
                cached_tokens = ?usage.cached_tokens,
                total_input = mstate.tokens.total_input,
                total_output = mstate.tokens.total_output,
                "Token usage - input_tokens is this call, total_input is cumulative"
            );

            r
        }
        Err(e) => {
            error!(error = %e, iteration = inputs.iteration, "LLM call with tools failed");
            return IterationOutcome::Failed(format_llm_error(&e));
        }
    };

    // Thinking content extraction.
    // Diagnostic: when the model is flagged is_reasoning but no thinking surfaces,
    // log enough context to tell apart the two failure modes:
    //   (a) reasoning_effort never sent (model card not flagged is_reasoning in DB)
    //   (b) reasoning_effort sent but provider returned no thinking field.
    let extracted_thinking = inputs.adapter.extract_thinking(&response);
    if extracted_thinking.is_none() && ctx.config.llm.is_reasoning {
        // Surface the message-level keys + content shape so the missing variant
        // can be identified without a full JSON dump (which may include user data).
        let message_keys: Vec<String> = response
            .pointer("/choices/0/message")
            .and_then(|m| m.as_object())
            .map(|obj| obj.keys().cloned().collect())
            .unwrap_or_default();
        let content_shape = match response.pointer("/choices/0/message/content") {
            Some(serde_json::Value::String(_)) => "string".to_string(),
            Some(serde_json::Value::Array(arr)) => format!(
                "array(types={:?})",
                arr.iter()
                    .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
            ),
            Some(serde_json::Value::Null) => "null".to_string(),
            Some(_) => "other".to_string(),
            None => "missing".to_string(),
        };
        debug!(
            iteration = inputs.iteration,
            provider = inputs.adapter.provider_name(),
            model = %ctx.config.llm.model,
            reasoning_effort_sent = ?effective_reasoning_effort(ctx.config),
            message_keys = ?message_keys,
            content_shape = %content_shape,
            "Model is flagged is_reasoning but extract_thinking returned None"
        );
    }
    if let Some(thinking) = extracted_thinking {
        if !thinking.trim().is_empty() {
            if mstate.tokens.iter_thinking.is_none() {
                let estimated = crate::llm::utils::estimate_tokens(&thinking);
                mstate.tokens.add_estimated_thinking(estimated);
            }
            *mstate.global_sequence += 1;
            emit_progress(
                ctx.agent_context,
                StreamChunk::thinking_block(inputs.event_workflow_id.to_string(), thinking.clone()),
            );
            mstate.reasoning_steps_data.push(ReasoningStepData {
                content: thinking,
                duration_ms: inputs.start_instant.elapsed().as_millis() as u64,
                sequence: *mstate.global_sequence,
                source: ReasoningSource::ModelThinking,
            });
        }
    }

    // Parse tool calls (if any).
    let function_calls = if inputs.adapter.has_tool_calls(&response) {
        inputs.adapter.parse_tool_calls(&response)
    } else {
        Vec::new()
    };

    mstate.iteration_metrics_data.push(IterationMetrics {
        iteration: inputs.iteration as u32,
        tokens_input: mstate.tokens.iter_input,
        tokens_output: mstate.tokens.iter_output,
        cached_tokens: mstate.tokens.iter_cached,
        cache_write_tokens: mstate.tokens.iter_cache_write,
        thinking_tokens: mstate.tokens.iter_thinking,
        messages_count: mstate.messages.len(),
        tool_calls_count: function_calls.len(),
        duration_ms: iter_start.elapsed().as_millis() as u64,
    });

    // Emit a live progress chunk so the frontend metrics bar reflects each
    // tool-loop iteration in real time. Without this, ENTREE/SORTIE,
    // contexte and t/s stay at 0 until the workflow completes (the final
    // `response_block` only fires once at the very end). `cost_usd` is left
    // None here — the per-iteration cost would require caching pricing
    // rates upfront; the final `response_block` carries the resolved cost.
    emit_progress(
        ctx.agent_context,
        StreamChunk::iteration_progress(
            inputs.event_workflow_id.to_string(),
            inputs.iteration as u32,
            mstate.tokens.total_input,
            mstate.tokens.total_output,
            mstate.tokens.total_cached,
            mstate.tokens.total_cache_write,
            None,
        ),
    );

    if function_calls.is_empty() {
        let final_text = match inputs.adapter.extract_content(&response) {
            Some(content) if !content.trim().is_empty() => content,
            _ => {
                warn!(
                    iteration = inputs.iteration,
                    "LLM returned empty content, treating as task completion"
                );
                format!(
                    "Task completed after {} iteration(s). Tool executions completed successfully.",
                    inputs.iteration
                )
            }
        };
        debug!(
            iteration = inputs.iteration,
            provider = inputs.adapter.provider_name(),
            finished = inputs.adapter.is_finished(&response),
            "No tool calls found, finishing"
        );
        return IterationOutcome::Finished(final_text);
    }

    info!(
        iteration = inputs.iteration,
        tool_calls_count = function_calls.len(),
        "Found tool calls, executing"
    );

    let tool_names: Vec<String> = function_calls.iter().map(|c| c.name.clone()).collect();
    *mstate.global_sequence += 1;
    emit_reasoning(
        ctx.agent_context,
        inputs.event_workflow_id,
        format!(
            "Executing {} tool(s): {}",
            function_calls.len(),
            tool_names.join(", ")
        ),
        inputs.start_instant.elapsed().as_millis() as u64,
        *mstate.global_sequence,
        ReasoningSource::AgentFlow,
        mstate.reasoning_steps_data,
    );

    // Add assistant message with tool calls.
    mstate
        .messages
        .push(inputs.adapter.build_assistant_message(&response));

    // Execute each function call.
    for call in &function_calls {
        let exec_start = std::time::Instant::now();

        emit_progress(
            ctx.agent_context,
            StreamChunk::tool_start(inputs.event_workflow_id.to_string(), call.name.clone()),
        );

        let result = tools::execute_function_call(
            call,
            inputs.call_ctx,
            mstate.tools_used,
            mstate.mcp_calls_made,
        )
        .await;

        let exec_duration = exec_start.elapsed().as_millis() as u64;
        let tool_type = if call.is_mcp_tool() { "mcp" } else { "local" };
        let (server_name, tool_name_for_data) = if let Some((server, tool)) = call.parse_mcp_name()
        {
            (Some(server.to_string()), tool.to_string())
        } else {
            (None, call.name.clone())
        };

        *mstate.global_sequence += 1;
        mstate.tool_executions_data.push(ToolExecutionData {
            tool_type: tool_type.to_string(),
            tool_name: tool_name_for_data.clone(),
            server_name: server_name.clone(),
            input_params: call.arguments.clone(),
            output_result: result.result.clone(),
            success: result.success,
            error_message: result.error.clone(),
            duration_ms: exec_duration,
            iteration: inputs.iteration as u32,
            sequence: *mstate.global_sequence,
        });

        let input_json =
            serde_json::to_string(&call.arguments).unwrap_or_else(|_| "{}".to_string());
        let output_json =
            serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".to_string());
        emit_progress(
            ctx.agent_context,
            StreamChunk::tool_call_complete(
                inputs.event_workflow_id.to_string(),
                tool_name_for_data,
                tool_type,
                server_name,
                exec_duration,
                input_json,
                output_json,
                result.success,
            ),
        );

        mstate
            .messages
            .push(inputs.adapter.format_tool_result(&result));
    }

    IterationOutcome::Continue
}
