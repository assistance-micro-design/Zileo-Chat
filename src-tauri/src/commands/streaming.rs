// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Streaming workflow execution with real-time events.
//!
//! Provides Tauri commands for executing workflows with streaming
//! responses via Tauri events.

use crate::{
    agents::core::agent::Task,
    models::{
        streaming::events, StreamChunk, Workflow, WorkflowComplete, WorkflowMetrics, WorkflowResult,
    },
    security::Validator,
    AppState,
};
use tauri::{Emitter, State, Window};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

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
        message_len = message.len()
    )
)]
pub async fn execute_workflow_streaming(
    window: Window,
    workflow_id: String,
    message: String,
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowResult, String> {
    info!("Starting streaming workflow execution");

    // Validate inputs
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    let validated_message = Validator::validate_message(&message).map_err(|e| {
        warn!(error = %e, "Invalid message");
        format!("Invalid message: {}", e)
    })?;

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent ID");
        format!("Invalid agent ID: {}", e)
    })?;

    // Load workflow
    let workflows: Vec<Workflow> = state
        .db
        .query(&format!(
            "SELECT * FROM workflow WHERE id = '{}'",
            validated_workflow_id
        ))
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

    let _workflow = workflows.first().ok_or_else(|| {
        warn!(workflow_id = %validated_workflow_id, "Workflow not found");
        emit_error(&window, &validated_workflow_id, "Workflow not found");
        "Workflow not found".to_string()
    })?;

    // Emit initial reasoning step
    emit_chunk(
        &window,
        StreamChunk::reasoning(
            validated_workflow_id.clone(),
            "Analyzing request and preparing response...".to_string(),
        ),
    );

    // Create task
    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for streaming workflow");

    let task = Task {
        id: task_id.clone(),
        description: validated_message,
        context: serde_json::json!({}),
    };

    // Emit tool start (agent execution)
    emit_chunk(
        &window,
        StreamChunk::tool_start(validated_workflow_id.clone(), validated_agent_id.clone()),
    );

    let start_time = std::time::Instant::now();

    // Execute via orchestrator
    let report = match state.orchestrator.execute(&validated_agent_id, task).await {
        Ok(report) => {
            let duration = start_time.elapsed().as_millis() as u64;

            // Emit tool end
            emit_chunk(
                &window,
                StreamChunk::tool_end(
                    validated_workflow_id.clone(),
                    validated_agent_id.clone(),
                    duration,
                ),
            );

            // Stream the response content in chunks
            let content = &report.content;
            let chunk_size = 50; // Characters per chunk for simulated streaming
            let mut cancelled = false;

            for (i, chunk) in content
                .chars()
                .collect::<Vec<_>>()
                .chunks(chunk_size)
                .enumerate()
            {
                // Check for cancellation before each chunk
                if state.is_cancelled(&validated_workflow_id).await {
                    warn!(workflow_id = %validated_workflow_id, "Streaming cancelled by user");
                    cancelled = true;
                    emit_chunk(
                        &window,
                        StreamChunk::error(
                            validated_workflow_id.clone(),
                            "Cancelled by user".to_string(),
                        ),
                    );
                    emit_complete(
                        &window,
                        WorkflowComplete::cancelled(validated_workflow_id.clone()),
                    );
                    // Clear the cancellation flag
                    state.clear_cancellation(&validated_workflow_id).await;
                    break;
                }

                let chunk_text: String = chunk.iter().collect();
                emit_chunk(
                    &window,
                    StreamChunk::token(validated_workflow_id.clone(), chunk_text),
                );

                // Small delay between chunks to simulate streaming
                if i < content.len() / chunk_size {
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
            }

            if cancelled {
                return Err("Workflow cancelled by user".to_string());
            }

            report
        }
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
            return Err(format!("Execution failed: {}", e));
        }
    };

    // Get agent config for accurate provider/model info
    let (provider, model) = match state.registry.get(&validated_agent_id).await {
        Some(agent) => {
            let config = agent.config();
            (config.llm.provider.clone(), config.llm.model.clone())
        }
        None => {
            // Fallback if agent not found (shouldn't happen after successful execution)
            ("Unknown".to_string(), validated_agent_id.clone())
        }
    };

    // Build result
    // Note: cost_usd calculation requires provider-specific pricing APIs (future enhancement)
    let result = WorkflowResult {
        report: report.content,
        metrics: WorkflowMetrics {
            duration_ms: report.metrics.duration_ms,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cost_usd: 0.0,
            provider,
            model,
        },
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
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
        "Streaming workflow execution completed"
    );

    Ok(result)
}

/// Helper function to emit a stream chunk event.
fn emit_chunk(window: &Window, chunk: StreamChunk) {
    if let Err(e) = window.emit(events::WORKFLOW_STREAM, &chunk) {
        warn!(error = %e, "Failed to emit stream chunk");
    }
}

/// Helper function to emit a completion event.
fn emit_complete(window: &Window, complete: WorkflowComplete) {
    if let Err(e) = window.emit(events::WORKFLOW_COMPLETE, &complete) {
        warn!(error = %e, "Failed to emit completion event");
    }
}

/// Helper function to emit an error and completion.
fn emit_error(window: &Window, workflow_id: &str, error: &str) {
    emit_chunk(
        window,
        StreamChunk::error(workflow_id.to_string(), error.to_string()),
    );
    emit_complete(
        window,
        WorkflowComplete::failed(workflow_id.to_string(), error.to_string()),
    );
}

/// Cancels a streaming workflow execution.
///
/// Marks the workflow for cooperative cancellation. The streaming execution
/// checks this flag between chunk emissions and will abort if set.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to cancel
/// * `state` - Application state containing the cancellation tracker
#[tauri::command]
#[instrument(name = "cancel_workflow_streaming", skip(state), fields(workflow_id = %workflow_id))]
pub async fn cancel_workflow_streaming(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Cancelling streaming workflow");

    // Validate workflow ID
    let validated_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // Request cancellation
    state.request_cancellation(&validated_id).await;
    info!(workflow_id = %validated_id, "Workflow cancellation requested");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::streaming::CompletionStatus;

    #[test]
    fn test_stream_chunk_creation() {
        let chunk = StreamChunk::token("wf_001".to_string(), "Hello".to_string());
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
