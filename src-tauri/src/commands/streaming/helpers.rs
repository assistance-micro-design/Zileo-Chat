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
    // Filter to user/assistant only: `system` rows are persisted by the frontend
    // catch{} branch as error notifications (workflowExecutor.service.ts) — they
    // are not real system prompts and must never be replayed to the LLM.
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
          AND role IN ['user', 'assistant']
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

    let history_count = conversation_history.len();
    let is_continuation = !conversation_history.is_empty();

    let history_context = if is_continuation {
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
        history_count,
        is_continuation, "Loaded conversation history for context"
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    /// Inserts a message row directly via SurrealQL using SET syntax.
    ///
    /// Bypasses the `save_message` Tauri command (which validates UUIDs and
    /// drops `system` role through helpers anyway) so tests can construct
    /// arbitrary fixtures, including the `system` rows that the production
    /// code is supposed to filter out.
    async fn insert_message(
        state: &AppState,
        workflow_id: &str,
        role: &str,
        content: &str,
        timestamp_offset_secs: i64,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let offset_duration = format!("{}s", timestamp_offset_secs);
        let query = format!(
            "CREATE message:`{id}` SET \
                workflow_id = $wf_id, \
                role = $role, \
                content = $content, \
                tokens = 0, \
                timestamp = time::now() + <duration>$offset"
        );
        state
            .db
            .db
            .query(&query)
            .bind(("wf_id", workflow_id.to_string()))
            .bind(("role", role.to_string()))
            .bind(("content", content.to_string()))
            .bind(("offset", offset_duration))
            .await
            .expect("Insert message query failed")
            .check()
            .expect("CREATE message failed validation");
    }

    #[tokio::test]
    async fn test_load_conversation_history_empty_workflow() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        let (context, count) = load_conversation_history(&state, &workflow_id, "en").await;

        assert_eq!(count, 0, "No messages → count should be 0");
        assert!(
            context.get("conversation_messages").is_none(),
            "Empty history must NOT inject conversation_messages"
        );
        assert_eq!(context["is_primary_agent"], serde_json::json!(true));
        assert_eq!(context["workflow_id"], serde_json::json!(workflow_id));
        assert_eq!(context["locale"], serde_json::json!("en"));
    }

    #[tokio::test]
    async fn test_load_conversation_history_detects_continuation_without_system_message() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        insert_message(&state, &workflow_id, "user", "Mon nom est Bob", 0).await;
        insert_message(&state, &workflow_id, "assistant", "Enchante Bob", 1).await;

        let (context, count) = load_conversation_history(&state, &workflow_id, "fr").await;

        assert_eq!(count, 2, "Should load 2 messages (user + assistant)");
        let messages = context
            .get("conversation_messages")
            .and_then(|v| v.as_array())
            .expect(
                "conversation_messages MUST be present when user/assistant history exists \
                 (this is the regression that breaks workflow continuation)",
            );
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "Mon nom est Bob");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"], "Enchante Bob");
    }

    #[tokio::test]
    async fn test_load_conversation_history_filters_system_error_messages() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        insert_message(&state, &workflow_id, "user", "Premier tour", 0).await;
        insert_message(&state, &workflow_id, "assistant", "Reponse 1", 1).await;
        insert_message(&state, &workflow_id, "system", "Error: provider offline", 2).await;
        insert_message(&state, &workflow_id, "user", "Deuxieme tour", 3).await;

        let (context, count) = load_conversation_history(&state, &workflow_id, "fr").await;

        assert_eq!(
            count, 3,
            "System error rows must be filtered out (3 user/assistant kept)"
        );
        let messages = context
            .get("conversation_messages")
            .and_then(|v| v.as_array())
            .expect("conversation_messages must be present for non-empty history");
        assert_eq!(messages.len(), 3);
        for msg in messages {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap();
            assert!(
                role == "user" || role == "assistant",
                "Only user/assistant roles allowed in LLM history, got {role}"
            );
        }
        assert_eq!(messages[2]["content"], "Deuxieme tour");
    }

    #[tokio::test]
    async fn test_load_conversation_history_chronological_order() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        insert_message(&state, &workflow_id, "user", "msg-1", 0).await;
        insert_message(&state, &workflow_id, "assistant", "msg-2", 1).await;
        insert_message(&state, &workflow_id, "user", "msg-3", 2).await;
        insert_message(&state, &workflow_id, "assistant", "msg-4", 3).await;

        let (context, _count) = load_conversation_history(&state, &workflow_id, "en").await;
        let messages = context
            .get("conversation_messages")
            .and_then(|v| v.as_array())
            .expect("conversation_messages must be present");

        let contents: Vec<&str> = messages
            .iter()
            .map(|m| m["content"].as_str().unwrap())
            .collect();
        assert_eq!(contents, vec!["msg-1", "msg-2", "msg-3", "msg-4"]);
    }

    #[tokio::test]
    async fn test_load_conversation_history_isolates_workflows() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_a = uuid::Uuid::new_v4().to_string();
        let workflow_b = uuid::Uuid::new_v4().to_string();

        insert_message(&state, &workflow_a, "user", "from A", 0).await;
        insert_message(&state, &workflow_b, "user", "from B", 0).await;

        let (context_a, count_a) = load_conversation_history(&state, &workflow_a, "en").await;
        assert_eq!(count_a, 1);
        assert_eq!(context_a["conversation_messages"][0]["content"], "from A");

        let (context_b, count_b) = load_conversation_history(&state, &workflow_b, "en").await;
        assert_eq!(count_b, 1);
        assert_eq!(context_b["conversation_messages"][0]["content"], "from B");
    }
}
