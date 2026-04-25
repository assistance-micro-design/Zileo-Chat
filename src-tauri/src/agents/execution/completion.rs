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

//! Final report enforcement and report content building.
//!
//! Two responsibilities:
//! 1. When the LLM finishes the tool loop with a generic completion message
//!    (e.g. "Task complete."), make a follow-up call to obtain a proper report.
//! 2. Format the final markdown report content emitted at the end of the loop.

use crate::agents::core::agent::{ReasoningSource, ReasoningStepData};
use crate::agents::execution::reasoning::{effective_reasoning_effort, emit_reasoning};
use crate::agents::execution::tool_loop::{TokenTracker, ToolLoopContext};
use crate::agents::prompt::REPORT_ENFORCEMENT_PROMPT;
use crate::llm::tool_adapter::ProviderToolAdapter;
use crate::llm::{ProviderType, ToolCompletionParams};
use crate::models::workflow::IterationMetrics;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

/// Mutable state passed to report enforcement to avoid too many arguments.
pub(crate) struct EnforcementState<'a> {
    pub messages: &'a mut Vec<serde_json::Value>,
    pub tokens: &'a mut TokenTracker,
    pub reasoning_steps: &'a mut Vec<ReasoningStepData>,
    pub iteration_metrics: &'a mut Vec<IterationMetrics>,
    pub global_sequence: &'a mut u32,
}

/// Builds the tools section for the final markdown report.
pub(crate) fn build_tools_section(tools_used: &[String], mcp_calls_made: &[String]) -> String {
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

/// Inputs needed to build the final report markdown.
pub(crate) struct ReportContentInputs<'a> {
    pub agent_id: &'a str,
    pub task_description: &'a str,
    pub final_response_content: &'a str,
    pub provider_type: &'a ProviderType,
    pub model: &'a str,
    pub total_tokens_input: usize,
    pub total_tokens_output: usize,
    pub duration_ms: u64,
    pub iteration: usize,
    pub tools_used: &'a [String],
    pub mcp_calls_made: &'a [String],
}

/// Builds the markdown content of a successful agent report.
pub(crate) fn build_report_content(inputs: &ReportContentInputs<'_>) -> String {
    let tools_section = build_tools_section(inputs.tools_used, inputs.mcp_calls_made);
    format!(
        "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Response\n\n{}\n\n## Metrics\n- Provider: {}\n- Model: {}\n- Tokens (input/output): {}/{}\n- Duration: {}ms\n- Tool iterations: {}{}",
        inputs.agent_id,
        inputs.task_description,
        inputs.final_response_content,
        inputs.provider_type,
        inputs.model,
        inputs.total_tokens_input,
        inputs.total_tokens_output,
        inputs.duration_ms,
        inputs.iteration,
        tools_section
    )
}

/// Makes a follow-up LLM call to get a proper report when the agent finished
/// with a generic completion message.
///
/// `cancellation_token` (when present) races the report enforcement call so
/// a cancellation request mid-enforcement tears down the in-flight HTTP
/// request — the call can be slow on big contexts.
///
/// Returns `Some(content)` when a meaningful response was obtained, `None`
/// otherwise (kept generic message, error, empty response).
#[allow(clippy::too_many_arguments)]
pub(crate) async fn enforce_report(
    ctx: &ToolLoopContext<'_>,
    provider_type: &ProviderType,
    adapter: &dyn ProviderToolAdapter,
    event_workflow_id: &str,
    state: &mut EnforcementState<'_>,
    elapsed_ms: u64,
    iteration: usize,
    cancellation_token: Option<CancellationToken>,
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
        .complete_with_tools_cancellable(
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
            cancellation_token,
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
