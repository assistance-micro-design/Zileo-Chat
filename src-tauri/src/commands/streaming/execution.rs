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
use tracing::{info, instrument, warn};
use uuid::Uuid;

use super::helpers::emit_error;
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
    let _workflow = match load_workflow(&window, &workflow_id, &state).await {
        Ok(workflow) => workflow,
        Err(err) => {
            state.clear_cancellation(&workflow_id).await;
            return Err(err);
        }
    };

    let message_id = Uuid::new_v4().to_string();
    let mut thinking_step_number: u32 = 0;

    // AtomicU32-backed sequence tracker shared with completion.
    let sequence_tracker = Arc::new(SequenceTracker::new(0));
    let initial_sequence = sequence_tracker.allocate();

    // Look up the orchestrator's display name for the spinner (M4 audit
    // 2026-05-02) and for chunk attribution on the initial reasoning step
    // / completion reasoning. Falls back to agent_id inside the bridge when
    // the agent is not registered yet.
    let agent_name = state
        .orchestrator
        .registry()
        .get(&agent_id)
        .await
        .map(|agent| agent.config().name.clone());

    thinking_step_number = persist_initial_reasoning(
        &state,
        &window,
        &workflow_id,
        &agent_id,
        agent_name.as_deref(),
        &message_id,
        initial_sequence,
        thinking_step_number,
    )
    .await;

    let (task, task_id) =
        match build_task(&state, &workflow_id, &message, &locale, &message_id).await {
            Ok(task) => task,
            Err(err) => {
                emit_error(&window, &workflow_id, &err);
                state.clear_cancellation(&workflow_id).await;
                return Err(err);
            }
        };

    let (report, duration_ms) = match run_orchestrator_with_cancel(
        &window,
        &state,
        &workflow_id,
        &agent_id,
        agent_name.as_deref(),
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
            agent_name: agent_name.as_deref(),
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

    if state.request_cancellation(&validated_id).await {
        info!(workflow_id = %validated_id, "Workflow cancellation requested");
    } else {
        warn!(workflow_id = %validated_id, "No running workflow cancellation token found");
        return Err("No running workflow found for cancellation".to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::models::streaming::events;

    #[test]
    fn test_event_names() {
        assert_eq!(events::WORKFLOW_STREAM, "workflow_stream");
        assert_eq!(events::WORKFLOW_COMPLETE, "workflow_complete");
    }
}
