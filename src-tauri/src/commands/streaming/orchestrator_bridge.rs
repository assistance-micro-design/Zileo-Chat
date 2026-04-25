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

//! Bridge between the streaming command and the agent orchestrator.
//!
//! Loads the workflow row, builds the [`Task`] with conversation history,
//! and races the orchestrator execution against the cancellation token.

use crate::{
    agents::core::agent::Report,
    agents::core::agent::Task,
    db::queries::workflow as wf_queries,
    models::{Prompt, StreamChunk, Workflow, WorkflowComplete},
    AppState,
};
use tauri::{State, Window};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

use super::helpers::{emit_chunk, emit_complete, emit_error, load_conversation_history};

/// Outcome of the orchestrator race.
///
/// `Report` is boxed because it is far larger than the other variants
/// (~260 B vs 24 B), which would otherwise inflate the whole enum size.
pub enum BridgeOutcome {
    /// Execution succeeded — caller proceeds with persistence.
    Completed {
        report: Box<Report>,
        duration_ms: u64,
    },
    /// Execution failed — error already emitted, cancellation cleared.
    Failed(String),
    /// User cancelled mid-flight — events already emitted, cancellation cleared.
    Cancelled,
}

/// Load the workflow row and confirm it exists.
///
/// Emits `workflow_stream` error events on failure so the frontend can react,
/// then returns the original error string for the command result.
pub async fn load_workflow(
    window: &Window,
    workflow_id: &str,
    state: &State<'_, AppState>,
) -> Result<Workflow, String> {
    let query = format!("{} WHERE meta::id(id) = $wf_id", wf_queries::SELECT_BASIC);

    let json_results = state
        .db
        .query_json_with_params(
            &query,
            vec![("wf_id".to_string(), serde_json::json!(workflow_id))],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflow");
            emit_error(
                window,
                workflow_id,
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
                window,
                workflow_id,
                &format!("Failed to deserialize workflow: {}", e),
            );
            format!("Failed to deserialize workflow: {}", e)
        })?;

    workflows.into_iter().next().ok_or_else(|| {
        warn!(workflow_id = %workflow_id, "Workflow not found");
        emit_error(window, workflow_id, "Workflow not found");
        "Workflow not found".to_string()
    })
}

/// Build the [`Task`] payload by interpolating skill references and loading
/// conversation history.
pub async fn build_task(
    state: &State<'_, AppState>,
    workflow_id: &str,
    message: &str,
    locale: &str,
) -> (Task, String) {
    let (history_context, _history_count) =
        load_conversation_history(state, workflow_id, locale).await;

    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for streaming workflow");

    // {{skill:name}} → instruction to read via ReadSkillTool
    let resolved_message = Prompt::interpolate_skills(message);

    let task = Task {
        id: task_id.clone(),
        description: resolved_message,
        context: history_context,
    };

    (task, task_id)
}

/// Race the orchestrator against the cancellation token.
///
/// On failure or cancellation this function emits the corresponding stream
/// chunks + completion event AND clears the cancellation token from the
/// app state, so callers only need to handle the [`BridgeOutcome`] variant.
pub async fn run_orchestrator_with_cancel(
    window: &Window,
    state: &State<'_, AppState>,
    workflow_id: &str,
    agent_id: &str,
    task: Task,
    cancellation_token: CancellationToken,
) -> BridgeOutcome {
    emit_chunk(
        window,
        StreamChunk::tool_start(workflow_id.to_string(), agent_id.to_string()),
    );

    let task_id = task.id.clone();
    let start_time = std::time::Instant::now();

    let execution_future = state.orchestrator.execute_with_mcp(
        agent_id,
        task,
        Some(state.mcp_manager.clone()),
        Some(cancellation_token.clone()),
    );

    tokio::select! {
        result = execution_future => {
            match result {
                Ok(report) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    info!(
                        duration_ms,
                        task_id = %task_id,
                        "Streaming execution completed, processing report"
                    );
                    BridgeOutcome::Completed {
                        report: Box::new(report),
                        duration_ms,
                    }
                }
                Err(e) => {
                    error!(error = %e, task_id = %task_id, "Streaming workflow execution failed");
                    emit_chunk(
                        window,
                        StreamChunk::error(workflow_id.to_string(), e.to_string()),
                    );
                    emit_complete(
                        window,
                        WorkflowComplete::failed(workflow_id.to_string(), e.to_string()),
                    );
                    state.clear_cancellation(workflow_id).await;
                    BridgeOutcome::Failed(format!("Execution failed: {}", e))
                }
            }
        }
        _ = cancellation_token.cancelled() => {
            warn!(workflow_id = %workflow_id, "Workflow cancelled by user during execution");
            emit_chunk(
                window,
                StreamChunk::error(workflow_id.to_string(), "Cancelled by user".to_string()),
            );
            emit_complete(
                window,
                WorkflowComplete::cancelled(workflow_id.to_string()),
            );
            state.clear_cancellation(workflow_id).await;
            BridgeOutcome::Cancelled
        }
    }
}
