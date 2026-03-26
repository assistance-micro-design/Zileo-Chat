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

//! Helper functions for streaming workflow execution.

use crate::constants::workflow as wf_const;
use crate::models::message::Message;
use crate::models::streaming::{events, StreamChunk, WorkflowComplete};
use crate::AppState;
use tauri::Emitter;
use tauri::Window;
use tracing::{error, info, warn};

/// Helper function to emit a stream chunk event.
pub fn emit_chunk(window: &Window, chunk: StreamChunk) {
    if let Err(e) = window.emit(events::WORKFLOW_STREAM, &chunk) {
        warn!(error = %e, "Failed to emit stream chunk");
    }
}

/// Helper function to emit a completion event.
pub fn emit_complete(window: &Window, complete: WorkflowComplete) {
    if let Err(e) = window.emit(events::WORKFLOW_COMPLETE, &complete) {
        warn!(error = %e, "Failed to emit completion event");
    }
}

/// Helper function to emit an error and completion.
pub fn emit_error(window: &Window, workflow_id: &str, error: &str) {
    emit_chunk(
        window,
        StreamChunk::error(workflow_id.to_string(), error.to_string()),
    );
    emit_complete(
        window,
        WorkflowComplete::failed(workflow_id.to_string(), error.to_string()),
    );
}

/// Loads conversation history and builds the context payload for the LLM.
///
/// Returns the history context JSON and the number of loaded messages.
pub async fn load_conversation_history(
    state: &AppState,
    workflow_id: &str,
    locale: &str,
) -> (serde_json::Value, usize) {
    let history_query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            role,
            content,
            tokens,
            tokens_input,
            tokens_output,
            model,
            provider,
            cost_usd,
            duration_ms,
            timestamp
        FROM message
        WHERE workflow_id = $wf_id
        ORDER BY timestamp ASC
        LIMIT {}"#,
        wf_const::MESSAGE_HISTORY_LIMIT
    );

    let history_json = state
        .db
        .query_json_with_params(
            &history_query,
            vec![("wf_id".to_string(), serde_json::json!(workflow_id))],
        )
        .await
        .unwrap_or_default();
    let conversation_history: Vec<Message> = history_json
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let has_system_message = conversation_history
        .iter()
        .any(|msg| matches!(msg.role, crate::models::MessageRole::System));

    let history_count = conversation_history.len();

    let history_context = if has_system_message && !conversation_history.is_empty() {
        let api_messages: Vec<serde_json::Value> = conversation_history
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
        serde_json::json!({
            "conversation_messages": api_messages,
            "is_primary_agent": true,
            "workflow_id": workflow_id,
            "locale": locale
        })
    } else {
        serde_json::json!({
            "is_primary_agent": true,
            "workflow_id": workflow_id,
            "locale": locale
        })
    };

    info!(
        history_count = history_count,
        has_system_message = has_system_message,
        is_continuation = has_system_message && !conversation_history.is_empty(),
        "Loaded conversation history for context"
    );

    (history_context, history_count)
}

/// Aggregates sub-agent tokens into separate workflow fields.
///
/// Queries all completed sub_agent_execution records for this workflow
/// and stores their token totals in sub_agent_tokens_input/output.
/// These are kept separate from total_tokens_input/output (main agent only)
/// so the frontend can display both independently and compute combined totals.
pub async fn aggregate_sub_agent_tokens(state: &AppState, workflow_id: &str) {
    let sum_query = "SELECT math::sum(tokens_input) AS total_in, \
                            math::sum(tokens_output) AS total_out \
                     FROM sub_agent_execution \
                     WHERE workflow_id = $wf_id AND status = 'completed' \
                     GROUP ALL";

    match state
        .db
        .db
        .query(sum_query)
        .bind(("wf_id", workflow_id.to_string()))
        .await
    {
        Ok(mut response) => {
            let result: Option<serde_json::Value> = response.take(0).unwrap_or(None);
            if let Some(row) = result {
                let tokens_in = row.get("total_in").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let tokens_out =
                    row.get("total_out").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                if tokens_in > 0 || tokens_out > 0 {
                    let update_query = format!(
                        "UPDATE workflow:`{}` SET \
                            sub_agent_tokens_input = $tokens_in, \
                            sub_agent_tokens_output = $tokens_out",
                        workflow_id
                    );

                    if let Err(e) = state
                        .db
                        .db
                        .query(&update_query)
                        .bind(("tokens_in", tokens_in))
                        .bind(("tokens_out", tokens_out))
                        .await
                    {
                        error!(error = %e, "Failed to store sub-agent tokens");
                    } else {
                        info!(
                            workflow_id = %workflow_id,
                            sub_agent_tokens_in = tokens_in,
                            sub_agent_tokens_out = tokens_out,
                            "Stored sub-agent tokens in separate fields"
                        );
                    }
                }
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to query sub-agent tokens for aggregation");
        }
    }
}
