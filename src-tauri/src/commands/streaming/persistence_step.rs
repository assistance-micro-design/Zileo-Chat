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

//! Post-execution persistence and emission for streaming workflows.
//!
//! Encapsulates the persistence side effects that follow a successful
//! orchestrator run: thinking step bookkeeping, response block emission,
//! pricing rollup, tool execution + reasoning persistence, sub-agent
//! linking, and the final `workflow_complete` event.

use crate::{
    agents::core::agent::Report,
    agents::execution::sequence_tracker::SequenceTracker,
    models::{
        StreamChunk, ThinkingStepCreate, WorkflowComplete, WorkflowMetrics, WorkflowResult,
        WorkflowToolExecution,
    },
    AppState,
};
use std::sync::Arc;
use tauri::{State, Window};
use tracing::{info, warn};
use uuid::Uuid;

use super::helpers::{aggregate_sub_agent_metrics, emit_chunk, emit_complete};
use super::pricing::{
    load_model_pricing_info, update_workflow_cumulative_metrics, CumulativeMetricsUpdate,
};

/// Inputs needed to persist completion side effects.
pub struct CompletionContext<'a> {
    pub window: &'a Window,
    pub workflow_id: &'a str,
    pub agent_id: &'a str,
    pub message_id: &'a str,
    pub report: Report,
    pub duration_ms: u64,
    pub thinking_step_number: u32,
    pub initial_sequence: u32,
    pub sequence_tracker: Arc<SequenceTracker>,
}

/// Persist the initial "Analyzing..." reasoning step and emit it.
///
/// Returns the next thinking step number to use.
pub async fn persist_initial_reasoning(
    state: &State<'_, AppState>,
    window: &Window,
    workflow_id: &str,
    agent_id: &str,
    message_id: &str,
    initial_sequence: u32,
    thinking_step_number: u32,
) -> u32 {
    let initial_reasoning = "Analyzing request and preparing response...".to_string();
    emit_chunk(
        window,
        StreamChunk::reasoning(workflow_id.to_string(), initial_reasoning.clone()),
    );

    let initial_step = ThinkingStepCreate {
        workflow_id: workflow_id.to_string(),
        message_id: message_id.to_string(),
        agent_id: agent_id.to_string(),
        step_number: thinking_step_number,
        content: initial_reasoning,
        duration_ms: None,
        tokens: None,
        sequence: initial_sequence,
        source: "agent_flow".to_string(),
    };
    let step_id = Uuid::new_v4().to_string();
    if let Err(e) = state
        .db
        .create("thinking_step", &step_id, initial_step)
        .await
    {
        warn!(error = %e, "Failed to persist initial thinking step");
    }
    thinking_step_number + 1
}

/// Apply all post-execution side effects and build the final [`WorkflowResult`].
///
/// Sequence (mirrors the original monolithic `execute_workflow_streaming`):
/// 1. Reserve the completion sequence so it lands strictly after every block
///    emitted during execution.
/// 2. Emit and persist the completion reasoning step.
/// 3. Emit the response block with token counts.
/// 4. Resolve pricing, surface a warning when cost == 0 but tokens > 0.
/// 5. Update the workflow row with cumulative metrics.
/// 6. Aggregate sub-agent tokens.
/// 7. Persist tool executions and reasoning steps (with DB flush).
/// 8. Link orphan sub-agent executions to this message (parameterized).
/// 9. Emit `workflow_complete`.
/// 10. Clear the cancellation token from app state.
pub async fn finalize_completion(
    state: &State<'_, AppState>,
    ctx: CompletionContext<'_>,
) -> WorkflowResult {
    let CompletionContext {
        window,
        workflow_id,
        agent_id,
        message_id,
        report,
        duration_ms,
        mut thinking_step_number,
        initial_sequence,
        sequence_tracker,
    } = ctx;

    // 1. Compute completion sequence — must land after every block from the agent.
    //
    // Two sources of sequence numbers race here:
    //   - blocks already attached to the report (tool executions, reasoning steps),
    //   - blocks allocated concurrently via `sequence_tracker` (e.g. orchestrator
    //     bridge events). `allocate()` advances the tracker so its current peek
    //     reflects the latest reservation; we then take the max of both sources
    //     and add 1 to land strictly after every emitted block.
    let max_report_sequence = report
        .metrics
        .tool_executions
        .iter()
        .map(|te| te.sequence)
        .chain(report.metrics.reasoning_steps.iter().map(|rs| rs.sequence))
        .max()
        .unwrap_or(initial_sequence);
    let allocated = sequence_tracker.allocate();
    let completion_sequence = max_report_sequence.max(allocated) + 1;

    // 2. Emit + persist completion reasoning.
    let completion_reasoning = format!(
        "Execution completed in {}ms. Processing {} tool call(s).",
        duration_ms,
        report.metrics.tool_executions.len()
    );
    emit_chunk(
        window,
        StreamChunk::reasoning(workflow_id.to_string(), completion_reasoning.clone()),
    );

    let completion_step = ThinkingStepCreate {
        workflow_id: workflow_id.to_string(),
        message_id: message_id.to_string(),
        agent_id: agent_id.to_string(),
        step_number: thinking_step_number,
        content: completion_reasoning,
        duration_ms: Some(duration_ms),
        tokens: None,
        sequence: completion_sequence,
        source: "agent_flow".to_string(),
    };
    let completion_step_id = Uuid::new_v4().to_string();
    if let Err(e) = state
        .db
        .create("thinking_step", &completion_step_id, completion_step)
        .await
    {
        warn!(error = %e, "Failed to persist completion thinking step");
    }
    thinking_step_number += 1;

    // 3. Resolve pricing BEFORE emitting the response_block so the chunk can
    //    carry the per-iteration cost. This lets a backgrounded workflow
    //    accumulate `partialCostUsd` on its bg execution without the frontend
    //    inventing any number (backend-as-source-of-truth invariant).
    let pricing = load_model_pricing_info(
        state,
        agent_id,
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        report.metrics.cached_tokens,
        report.metrics.cache_write_tokens,
        report.metrics.provider_cost_usd,
    )
    .await;

    let total_tokens = report.metrics.tokens_input + report.metrics.tokens_output;
    if pricing.cost_usd == 0.0 && total_tokens > 0 {
        warn!(
            workflow_id = %workflow_id,
            model_id = %pricing.model_id,
            tokens = total_tokens,
            "Cost is $0.00 with non-zero token usage — model pricing likely missing"
        );
    }

    // 4. Emit response block with the resolved cost embedded.
    emit_chunk(
        window,
        StreamChunk::response_block(
            workflow_id.to_string(),
            report.response.clone(),
            report.metrics.tokens_input,
            report.metrics.tokens_output,
            report.metrics.cached_tokens,
            report.metrics.cache_write_tokens,
            report.metrics.thinking_tokens,
            Some(pricing.cost_usd),
        ),
    );

    // 5. Workflow row cumulative update.
    update_workflow_cumulative_metrics(
        state,
        &CumulativeMetricsUpdate {
            workflow_id,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cached_tokens: report.metrics.cached_tokens,
            cache_write_tokens: report.metrics.cache_write_tokens,
            cost_usd: pricing.cost_usd,
            model_id: &pricing.model_id,
            context_tokens: report.metrics.context_tokens,
        },
    )
    .await;

    // 6. Sub-agent token rollup.
    aggregate_sub_agent_metrics(state, workflow_id).await;

    // 7. Persist tool executions and reasoning steps.
    let tool_executions: Vec<WorkflowToolExecution> = report
        .metrics
        .tool_executions
        .iter()
        .map(|te| WorkflowToolExecution {
            tool_type: te.tool_type.clone(),
            tool_name: te.tool_name.clone(),
            server_name: te.server_name.clone(),
            input_params: te.input_params.clone(),
            output_result: te.output_result.clone(),
            success: te.success,
            error_message: te.error_message.clone(),
            duration_ms: te.duration_ms,
            iteration: te.iteration,
        })
        .collect();

    crate::db::persist_tool_executions(
        &state.db,
        &report.metrics.tool_executions,
        workflow_id,
        message_id,
        agent_id,
    )
    .await;

    thinking_step_number = crate::db::persist_reasoning_steps(
        &state.db,
        &report.metrics.reasoning_steps,
        workflow_id,
        message_id,
        agent_id,
        thinking_step_number,
    )
    .await;

    // 8. Sub-agent executions get `parent_message_id` at CREATE time
    //    (see SubAgentExecutor::create_execution_record_with_parent and
    //    SubAgentExecutionCreate::parent_message_id) — H2 audit 2026-05-02.
    //    The legacy bulk UPDATE was removed because it over-attributed
    //    nested sub-agents (A→B→C) all to the same primary message.

    info!(
        tool_executions_count = tool_executions.len(),
        thinking_steps_count = thinking_step_number,
        "Persisted tool executions and thinking steps to database"
    );

    let pricing_status = match pricing.status {
        crate::commands::streaming::pricing::PricingStatus::Ok => Some("ok".to_string()),
        crate::commands::streaming::pricing::PricingStatus::ModelNotFound => {
            Some("model_not_found".to_string())
        }
        crate::commands::streaming::pricing::PricingStatus::NoPricingSet => {
            Some("no_pricing_set".to_string())
        }
    };

    let result = WorkflowResult {
        report: report.content,
        response: report.response,
        metrics: WorkflowMetrics {
            duration_ms: report.metrics.duration_ms,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cost_usd: pricing.cost_usd,
            provider: pricing.provider,
            model: pricing.model,
            cached_tokens: report.metrics.cached_tokens,
            cache_write_tokens: report.metrics.cache_write_tokens,
            thinking_tokens: report.metrics.thinking_tokens,
            provider_cost_usd: report.metrics.provider_cost_usd,
            model_id_used: Some(pricing.model_id.clone()),
            pricing_status,
            iteration_metrics: report.metrics.iteration_metrics.clone(),
        },
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
        tool_executions,
        message_id: message_id.to_string(),
    };

    // 9. Emit completion event.
    emit_complete(window, WorkflowComplete::success(workflow_id.to_string()));

    info!(
        duration_ms = result.metrics.duration_ms,
        tokens_input = result.metrics.tokens_input,
        tokens_output = result.metrics.tokens_output,
        tool_executions_count = result.tool_executions.len(),
        "Streaming workflow execution completed"
    );

    // 10. Clear cancellation token.
    state.clear_cancellation(workflow_id).await;

    result
}
