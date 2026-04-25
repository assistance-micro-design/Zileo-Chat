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
//! Tauri commands for executing and cancelling workflows with real-time
//! events. Orchestrates the validator → orchestrator_bridge → persistence
//! pipeline. Each step lives in its own sibling module.

use crate::{
    agents::execution::sequence_tracker::SequenceTracker, models::WorkflowResult,
    security::validate_uuid_field, AppState,
};
use std::sync::Arc;
use tauri::{State, Window};
use tracing::{info, instrument};
use uuid::Uuid;

use super::orchestrator_bridge::{
    build_task, load_workflow, run_orchestrator_with_cancel, BridgeOutcome,
};
use super::persistence_step::{finalize_completion, persist_initial_reasoning, CompletionContext};
use super::validator::{validate_inputs, ValidatedInputs};

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

    let ValidatedInputs {
        workflow_id,
        message,
        agent_id,
    } = validate_inputs(workflow_id, message, agent_id, &state).await?;

    let cancellation_token = state.create_cancellation_token(&workflow_id).await;

    // Confirm workflow exists (errors emitted to frontend by load_workflow).
    let _workflow = load_workflow(&window, &workflow_id, &state).await?;

    let message_id = Uuid::new_v4().to_string();
    let mut thinking_step_number: u32 = 0;

    // AtomicU32-backed sequence tracker shared with completion.
    let sequence_tracker = Arc::new(SequenceTracker::new(0));
    let initial_sequence = sequence_tracker.allocate();

    thinking_step_number = persist_initial_reasoning(
        &state,
        &window,
        &workflow_id,
        &agent_id,
        &message_id,
        initial_sequence,
        thinking_step_number,
    )
    .await;

    let (task, task_id) = build_task(&state, &workflow_id, &message, &locale).await;

    let (report, duration_ms) = match run_orchestrator_with_cancel(
        &window,
        &state,
        &workflow_id,
        &agent_id,
        task,
        cancellation_token,
    )
    .await
    {
        BridgeOutcome::Completed {
            report,
            duration_ms,
        } => (*report, duration_ms),
        BridgeOutcome::Failed(msg) => return Err(msg),
        BridgeOutcome::Cancelled => return Err("Workflow cancelled by user".to_string()),
    };

    info!(task_id = %task_id, "Finalizing streaming workflow completion");

    let result = finalize_completion(
        &state,
        CompletionContext {
            window: &window,
            workflow_id: &workflow_id,
            agent_id: &agent_id,
            message_id: &message_id,
            report,
            duration_ms,
            thinking_step_number,
            initial_sequence,
            sequence_tracker,
        },
    )
    .await;

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
