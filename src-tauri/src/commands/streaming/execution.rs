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

//! Streaming workflow execution commands.
//!
//! Tauri commands for executing and cancelling workflows with real-time events.

use crate::{
    agents::core::agent::Task,
    db::queries::workflow as wf_queries,
    models::{
        Prompt, StreamChunk, ThinkingStepCreate, Workflow, WorkflowComplete, WorkflowMetrics,
        WorkflowResult, WorkflowToolExecution,
    },
    security::{validate_uuid_field, Validator},
    AppState,
};
use tauri::{State, Window};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

use super::helpers::{
    aggregate_sub_agent_tokens, emit_chunk, emit_complete, emit_error, load_conversation_history,
};
use super::pricing::{
    load_model_pricing_info, update_workflow_cumulative_metrics, CumulativeMetricsUpdate,
};

/// Executes a workflow with streaming events.
///
/// Emits events during execution:
/// - `workflow_stream`: For each token/tool/reasoning chunk
/// - `workflow_complete`: When execution finishes
///
/// # Arguments
/// * `window` - Tauri window for event emission
/// * `workflow_id` - Associated workflow ID
/// * `message` - User message to process
/// * `agent_id` - Agent to execute with
/// * `locale` - User's selected language (e.g., "en", "fr")
///
/// # Returns
/// Final workflow result after streaming completes
#[tauri::command]
#[instrument(
    name = "execute_workflow_streaming",
    skip(window, state, message),
    fields(
        workflow_id = %workflow_id,
        agent_id = %agent_id,
        message_len = message.len(),
        locale = %locale
    )
)]
pub async fn execute_workflow_streaming(
    window: Window,
    workflow_id: String,
    message: String,
    agent_id: String,
    locale: String,
    state: State<'_, AppState>,
) -> Result<WorkflowResult, String> {
    info!("Starting streaming workflow execution");

    // Validate inputs
    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    let validated_message = Validator::validate_message(&message).map_err(|e| {
        warn!(error = %e, "Invalid message");
        format!("Invalid message: {}", e)
    })?;

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    // Safety net: enforce concurrent workflow limit
    // Frontend enforces this too, but backend provides race condition protection
    let running_count = state.streaming_cancellations.lock().await.len();
    let max_concurrent: usize = 3; // Maximum concurrent workflows (frontend also enforces per-mode limits)
    if running_count >= max_concurrent {
        return Err(format!(
            "Maximum concurrent workflows ({}) reached. Please wait for a workflow to complete.",
            max_concurrent
        ));
    }

    // Create cancellation token for this workflow (enables real cancel functionality)
    let cancellation_token = state
        .create_cancellation_token(&validated_workflow_id)
        .await;

    // Use centralized query constant with bind param
    let query = format!("{} WHERE meta::id(id) = $wf_id", wf_queries::SELECT_BASIC);

    let json_results = state
        .db
        .query_json_with_params(
            &query,
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflow");
            emit_error(
                &window,
                &validated_workflow_id,
                &format!("Failed to load workflow: {}", e),
            );
            format!("Failed to load workflow: {}", e)
        })?;

    let workflows: Vec<Workflow> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Workflow>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize workflow");
            emit_error(
                &window,
                &validated_workflow_id,
                &format!("Failed to deserialize workflow: {}", e),
            );
            format!("Failed to deserialize workflow: {}", e)
        })?;

    let _workflow = workflows.first().ok_or_else(|| {
        warn!(workflow_id = %validated_workflow_id, "Workflow not found");
        emit_error(&window, &validated_workflow_id, "Workflow not found");
        "Workflow not found".to_string()
    })?;

    // Generate a message ID for this execution (the assistant response)
    // This is generated early so thinking steps can reference it
    let message_id = Uuid::new_v4().to_string();

    // Counter for thinking steps
    let mut thinking_step_number: u32 = 0;

    // Global sequence counter for block ordering (shared with report blocks)
    // Initial reasoning = sequence 0, report blocks start at 1+
    let initial_sequence: u32 = 0;

    // Emit and persist initial reasoning step
    let initial_reasoning = "Analyzing request and preparing response...".to_string();
    emit_chunk(
        &window,
        StreamChunk::reasoning(validated_workflow_id.clone(), initial_reasoning.clone()),
    );

    // Persist the initial thinking step
    let initial_step = ThinkingStepCreate {
        workflow_id: validated_workflow_id.clone(),
        message_id: message_id.clone(),
        agent_id: validated_agent_id.clone(),
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
    thinking_step_number += 1;

    // Load conversation history and build context for the LLM
    let (history_context, _history_count) =
        load_conversation_history(&state, &validated_workflow_id, &locale).await;

    // Create task with conversation history
    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for streaming workflow");

    // Resolve skill references: {{skill:name}} -> LLM instruction to read via ReadSkillTool
    let resolved_message = Prompt::interpolate_skills(&validated_message);

    let task = Task {
        id: task_id.clone(),
        description: resolved_message,
        context: history_context,
    };

    // Emit tool start (agent execution)
    emit_chunk(
        &window,
        StreamChunk::tool_start(validated_workflow_id.clone(), validated_agent_id.clone()),
    );

    let start_time = std::time::Instant::now();

    // Execute via orchestrator with MCP support, racing against cancellation token
    // Using tokio::select! allows the execution to be cancelled immediately when the user clicks Cancel
    // The cancellation token is also propagated to sub-agents so they abort when the user cancels
    let execution_future = state.orchestrator.execute_with_mcp(
        &validated_agent_id,
        task,
        Some(state.mcp_manager.clone()),
        Some(cancellation_token.clone()),
    );

    let report = tokio::select! {
        // Execution branch - runs the actual LLM call
        result = execution_future => {
            match result {
                Ok(report) => report,
                Err(e) => {
                    error!(error = %e, task_id = %task_id, "Streaming workflow execution failed");
                    emit_chunk(
                        &window,
                        StreamChunk::error(validated_workflow_id.clone(), e.to_string()),
                    );
                    emit_complete(
                        &window,
                        WorkflowComplete::failed(validated_workflow_id.clone(), e.to_string()),
                    );
                    state.clear_cancellation(&validated_workflow_id).await;
                    return Err(format!("Execution failed: {}", e));
                }
            }
        }
        // Cancellation branch - triggers when user clicks Cancel button
        _ = cancellation_token.cancelled() => {
            warn!(workflow_id = %validated_workflow_id, "Workflow cancelled by user during execution");
            emit_chunk(
                &window,
                StreamChunk::error(validated_workflow_id.clone(), "Cancelled by user".to_string()),
            );
            emit_complete(
                &window,
                WorkflowComplete::cancelled(validated_workflow_id.clone()),
            );
            state.clear_cancellation(&validated_workflow_id).await;
            return Err("Workflow cancelled by user".to_string());
        }
    };

    let duration = start_time.elapsed().as_millis() as u64;
    info!(
        duration_ms = duration,
        task_id = %task_id,
        "Streaming execution completed, processing report"
    );

    // Note: thinking blocks from report.metrics.reasoning_steps are NOT re-emitted here
    // because they were already emitted in real-time by the tool_loop via emit_reasoning()
    // and emit_progress(StreamChunk::thinking_block(...)). Re-emitting would cause duplicates.

    // Compute completion sequence: after all report blocks (tool_executions + reasoning_steps)
    let max_report_sequence = report
        .metrics
        .tool_executions
        .iter()
        .map(|te| te.sequence)
        .chain(report.metrics.reasoning_steps.iter().map(|rs| rs.sequence))
        .max()
        .unwrap_or(initial_sequence);
    let completion_sequence = max_report_sequence + 1;

    // Emit and persist reasoning step about execution completion
    let completion_reasoning = format!(
        "Execution completed in {}ms. Processing {} tool call(s).",
        duration,
        report.metrics.tool_executions.len()
    );
    emit_chunk(
        &window,
        StreamChunk::reasoning(validated_workflow_id.clone(), completion_reasoning.clone()),
    );

    // Persist the completion thinking step with correct sequence
    let completion_step = ThinkingStepCreate {
        workflow_id: validated_workflow_id.clone(),
        message_id: message_id.clone(),
        agent_id: validated_agent_id.clone(),
        step_number: thinking_step_number,
        content: completion_reasoning,
        duration_ms: Some(duration),
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

    emit_chunk(
        &window,
        StreamChunk::response_block(
            validated_workflow_id.clone(),
            report.response.clone(),
            report.metrics.tokens_input,
            report.metrics.tokens_output,
            report.metrics.cached_tokens,
            report.metrics.cache_write_tokens,
            report.metrics.thinking_tokens,
        ),
    );

    // Load agent config, model pricing, and calculate cost
    let pricing = load_model_pricing_info(
        &state,
        &validated_agent_id,
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        report.metrics.cached_tokens,
        report.metrics.cache_write_tokens,
    )
    .await;

    // Update workflow with cumulative tokens, cost, model_id, and current context size
    update_workflow_cumulative_metrics(
        &state,
        &CumulativeMetricsUpdate {
            workflow_id: &validated_workflow_id,
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

    aggregate_sub_agent_tokens(&state, &validated_workflow_id).await;

    // Convert tool executions to IPC-friendly format (clones necessary for IPC serialization)
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
    // Note: Clones here are necessary as WorkflowToolExecution needs owned data for Tauri IPC

    // Persist tool executions via shared persistence module
    crate::db::persist_tool_executions(
        &state.db,
        &report.metrics.tool_executions,
        &validated_workflow_id,
        &message_id,
        &validated_agent_id,
    )
    .await;

    // Persist intermediate reasoning steps via shared persistence module
    thinking_step_number = crate::db::persist_reasoning_steps(
        &state.db,
        &report.metrics.reasoning_steps,
        &validated_workflow_id,
        &message_id,
        &validated_agent_id,
        thinking_step_number,
    )
    .await;

    // Link sub-agent executions to this message for load_message_blocks correlation
    // Use both IS NONE and IS NULL: SurrealDB option<string> fields may be either
    // NONE (field absent) or NULL (field present but null) depending on creation path
    let link_query = format!(
        "UPDATE sub_agent_execution SET parent_message_id = '{}' \
         WHERE workflow_id = '{}' AND (parent_message_id IS NONE OR parent_message_id IS NULL)",
        message_id, validated_workflow_id
    );
    if let Err(e) = state.db.execute(&link_query).await {
        warn!(error = %e, "Failed to link sub-agent executions to message");
    }

    info!(
        tool_executions_count = tool_executions.len(),
        thinking_steps_count = thinking_step_number,
        "Persisted tool executions and thinking steps to database"
    );

    // Build result with calculated cost
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
            iteration_metrics: report.metrics.iteration_metrics.clone(),
        },
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
        tool_executions,
        message_id: message_id.clone(),
    };

    // Emit completion
    emit_complete(
        &window,
        WorkflowComplete::success(validated_workflow_id.clone()),
    );

    info!(
        duration_ms = result.metrics.duration_ms,
        tokens_input = result.metrics.tokens_input,
        tokens_output = result.metrics.tokens_output,
        tool_executions_count = result.tool_executions.len(),
        "Streaming workflow execution completed"
    );

    // Cleanup: remove cancellation token from map on successful completion
    state.clear_cancellation(&validated_workflow_id).await;

    Ok(result)
}

/// Cancels a streaming workflow execution immediately.
///
/// Triggers the cancellation token associated with the workflow, causing the
/// execute_workflow_streaming function to abort via tokio::select!.
/// This provides immediate cancellation, even during LLM execution.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to cancel
/// * `state` - Application state containing the cancellation tokens
#[tauri::command]
#[instrument(name = "cancel_workflow_streaming", skip(state), fields(workflow_id = %workflow_id))]
pub async fn cancel_workflow_streaming(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Cancelling streaming workflow");

    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Request cancellation
    state.request_cancellation(&validated_id).await;
    info!(workflow_id = %validated_id, "Workflow cancellation requested");

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::models::streaming::{events, CompletionStatus, StreamChunk, WorkflowComplete};

    #[test]
    fn test_stream_chunk_creation() {
        let chunk = StreamChunk::reasoning("wf_001".to_string(), "Analyzing...".to_string());
        assert_eq!(chunk.workflow_id, "wf_001");
        assert!(chunk.content.is_some());

        let chunk = StreamChunk::tool_start("wf_001".to_string(), "search".to_string());
        assert!(chunk.tool.is_some());
        assert!(chunk.content.is_none());
    }

    #[test]
    fn test_workflow_complete_creation() {
        let complete = WorkflowComplete::success("wf_001".to_string());
        assert_eq!(complete.status, CompletionStatus::Completed);
        assert!(complete.error.is_none());

        let complete = WorkflowComplete::failed("wf_001".to_string(), "Error".to_string());
        assert_eq!(complete.status, CompletionStatus::Error);
        assert!(complete.error.is_some());
    }

    #[test]
    fn test_event_names() {
        assert_eq!(events::WORKFLOW_STREAM, "workflow_stream");
        assert_eq!(events::WORKFLOW_COMPLETE, "workflow_complete");
    }
}
