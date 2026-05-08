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

//! Message commands for conversation persistence.
//!
//! Provides Tauri commands for saving and retrieving conversation messages
//! with associated metrics for workflow state recovery.
//!
//! Enables complete workflow state recovery after application restart
//! by persisting all messages to SurrealDB.

use crate::{
    constants::commands as cmd_const,
    db::extract_count,
    models::{
        merge_into_chat_blocks, sub_agent::SubAgentExecution, ChatBlock, Message, MessageCreate,
        MessageMetrics, PaginatedMessages, ThinkingStep, ToolExecution,
    },
    security::validate_uuid_field,
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Saves a new message to the database.
///
/// # Arguments
/// * `workflow_id` - Associated workflow ID
/// * `role` - Message role (user, assistant, system)
/// * `content` - Message text content
/// * `tokens_input` - Input tokens consumed (optional, for assistant messages)
/// * `tokens_output` - Output tokens generated (optional, for assistant messages)
/// * `model` - Model used for generation (optional)
/// * `provider` - Provider used (optional)
/// * `duration_ms` - Generation duration in milliseconds (optional)
/// * `cost_usd` - Estimated cost in USD (optional)
/// * `cached_tokens` - Cache-read prompt tokens (optional)
/// * `cache_write_tokens` - Cache-write prompt tokens (optional)
/// * `model_id_used` - `llm_model.id` of the model that produced the response (optional)
///
/// # Returns
/// The ID of the created message
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[instrument(
    name = "save_message",
    skip(state, content),
    fields(
        workflow_id = %workflow_id,
        role = %role,
        content_len = content.len()
    )
)]
pub async fn save_message(
    workflow_id: String,
    role: String,
    content: String,
    tokens_input: Option<u64>,
    tokens_output: Option<u64>,
    model: Option<String>,
    provider: Option<String>,
    duration_ms: Option<u64>,
    thinking_tokens: Option<u64>,
    cost_usd: Option<f64>,
    cached_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
    model_id_used: Option<String>,
    message_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Saving message");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Validate role
    let validated_role = match role.as_str() {
        "user" | "assistant" | "system" => role.clone(),
        _ => {
            warn!(role = %role, "Invalid message role");
            return Err(format!(
                "Invalid message role: {}. Expected user, assistant, or system",
                role
            ));
        }
    };

    // Validate content
    if content.is_empty() {
        return Err("Message content cannot be empty".to_string());
    }
    if content.len() > cmd_const::MAX_MESSAGE_CONTENT_LEN {
        return Err(format!(
            "Message content exceeds maximum length of {} characters",
            cmd_const::MAX_MESSAGE_CONTENT_LEN
        ));
    }

    // Use provided message_id (for block association) or generate new one
    let message_id = match message_id {
        Some(id) => validate_uuid_field(&id, "message_id")?,
        None => Uuid::new_v4().to_string(),
    };

    // Build MessageCreate payload.
    //
    // Legacy `tokens` field stores the sum input + output rather than just
    // output, so old consumers that read it get a more meaningful number
    // (a non-zero count even on user messages). Prefer `tokens_input` /
    // `tokens_output` for new code; the field is kept for back-compat.
    let legacy_tokens = (tokens_input.unwrap_or(0) + tokens_output.unwrap_or(0)) as usize;
    let message = MessageCreate {
        workflow_id: validated_workflow_id,
        role: validated_role,
        content,
        tokens: legacy_tokens,
        tokens_input,
        tokens_output,
        model,
        provider,
        cost_usd,
        duration_ms,
        thinking_tokens,
        cached_tokens,
        cache_write_tokens,
        model_id_used,
    };

    // Insert into database
    let id = state
        .db
        .create("message", &message_id, message)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to save message");
            format!("Failed to save message: {}", e)
        })?;

    info!(message_id = %id, "Message saved successfully");
    Ok(message_id)
}

/// Loads all messages for a workflow, sorted by timestamp (oldest first).
///
/// # Arguments
/// * `workflow_id` - The workflow ID to load messages for
///
/// # Returns
/// Vector of messages in chronological order
#[tauri::command]
#[instrument(name = "load_workflow_messages", skip(state), fields(workflow_id = %workflow_id))]
pub async fn load_workflow_messages(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Message>, String> {
    info!("Loading workflow messages");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    // ORDER BY timestamp ASC for chronological order.
    //
    // Bind workflow_id as a parameter for defence-in-depth (UUID is already
    // validated, but parameterised queries keep the SQL static).
    let query = r#"SELECT
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
            thinking_tokens,
            cached_tokens,
            cache_write_tokens,
            model_id_used,
            timestamp
        FROM message
        WHERE workflow_id = $wf_id
        ORDER BY timestamp ASC"#;

    let json_results = state
        .db
        .query_json_with_params(
            query,
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflow messages");
            format!("Failed to load workflow messages: {}", e)
        })?;

    // Deserialize using serde_json which respects our custom deserializers
    let messages: Vec<Message> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Message>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize messages");
            format!("Failed to deserialize messages: {}", e)
        })?;

    info!(count = messages.len(), "Workflow messages loaded");
    Ok(messages)
}

/// Loads messages for a workflow with pagination support.
/// Useful for long conversation histories to reduce initial load time.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to load messages for
/// * `limit` - Maximum number of messages to return (default: 50)
/// * `offset` - Number of messages to skip (default: 0)
///
/// # Returns
/// Paginated result with messages and metadata
#[tauri::command]
#[instrument(
    name = "load_workflow_messages_paginated",
    skip(state),
    fields(workflow_id = %workflow_id, limit = ?limit, offset = ?offset)
)]
pub async fn load_workflow_messages_paginated(
    workflow_id: String,
    limit: Option<u32>,
    offset: Option<u32>,
    state: State<'_, AppState>,
) -> Result<PaginatedMessages, String> {
    info!("Loading paginated workflow messages");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    let limit = limit.unwrap_or(50).min(200); // Cap at 200 max
    let offset = offset.unwrap_or(0);

    // Get total count (bind workflow_id).
    let count_query = "SELECT count() FROM message WHERE workflow_id = $wf_id GROUP ALL";
    let count_result: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            count_query,
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to count workflow messages");
            format!("Failed to count workflow messages: {}", e)
        })?;

    let total = extract_count(&count_result) as u32;

    // Load paginated messages. LIMIT / START accept literals only (not bound
    // parameters in this SDK version), so we still format them in — they come
    // from validated `u32` so injection is impossible. workflow_id is bound.
    let query = format!(
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
            thinking_tokens,
            cached_tokens,
            cache_write_tokens,
            model_id_used,
            timestamp
        FROM message
        WHERE workflow_id = $wf_id
        ORDER BY timestamp ASC
        LIMIT {} START {}"#,
        limit, offset
    );

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
            error!(error = %e, "Failed to load paginated messages");
            format!("Failed to load paginated messages: {}", e)
        })?;

    let messages: Vec<Message> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Message>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize messages");
            format!("Failed to deserialize messages: {}", e)
        })?;

    let has_more = offset + (messages.len() as u32) < total;

    info!(
        count = messages.len(),
        total = total,
        has_more = has_more,
        "Paginated messages loaded"
    );

    Ok(PaginatedMessages {
        messages,
        total,
        offset,
        limit,
        has_more,
    })
}

/// Deletes a single message by ID.
///
/// # Arguments
/// * `message_id` - The message ID to delete
///
/// # Returns
/// Success or error
#[tauri::command]
#[instrument(name = "delete_message", skip(state), fields(message_id = %message_id))]
pub async fn delete_message(message_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting message");

    let validated_id = validate_uuid_field(&message_id, "message_id")?;

    // Use execute() with DELETE query to avoid SurrealDB SDK serialization issues
    // (see CLAUDE.md - db.delete() has issues with table:id format)
    state
        .db
        .execute(&format!("DELETE message:`{}`", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete message");
            format!("Failed to delete message: {}", e)
        })?;

    info!("Message deleted successfully");
    Ok(())
}

/// Deletes all messages for a workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to clear messages for
///
/// # Returns
/// Number of messages deleted
#[tauri::command]
#[instrument(name = "clear_workflow_messages", skip(state), fields(workflow_id = %workflow_id))]
pub async fn clear_workflow_messages(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    info!("Clearing workflow messages");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // First count existing messages (bind workflow_id).
    let count_query = "SELECT count() FROM message WHERE workflow_id = $wf_id GROUP ALL";
    let count_result: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            count_query,
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to count workflow messages before delete");
            format!("Failed to count workflow messages before delete: {}", e)
        })?;

    let count = extract_count(&count_result);

    // Delete all messages for the workflow (bind workflow_id).
    state
        .db
        .execute_with_params(
            "DELETE message WHERE workflow_id = $wf_id",
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to clear workflow messages");
            format!("Failed to clear workflow messages: {}", e)
        })?;

    info!(count = count, "Workflow messages cleared");
    Ok(count)
}

/// Returns lightweight metrics from the most recent assistant message of a workflow.
///
/// When the user switches to a workflow that is not currently streaming, the
/// frontend calls this to restore the session display from the last persisted
/// assistant message — so the user sees "what the last run cost" instead of
/// blank zeros.
///
/// # Returns
/// `Some(MessageMetrics)` if the workflow has at least one assistant message,
/// `None` if it has no assistant messages yet (fresh workflow).
#[tauri::command]
#[instrument(
    name = "get_workflow_last_assistant_message_metrics",
    skip(state),
    fields(workflow_id = %workflow_id)
)]
pub async fn get_workflow_last_assistant_message_metrics(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Option<MessageMetrics>, String> {
    last_assistant_message_metrics_core(&state.db, &workflow_id).await
}

/// Core implementation of `get_workflow_last_assistant_message_metrics`,
/// extracted so it can be exercised by integration tests against a real
/// SurrealDB instance (the `#[tauri::command]` wrapper requires a live
/// `tauri::State` and isn't directly testable).
pub(crate) async fn last_assistant_message_metrics_core(
    db: &crate::db::DBClient,
    workflow_id: &str,
) -> Result<Option<MessageMetrics>, String> {
    let validated_workflow_id = validate_uuid_field(workflow_id, "workflow_id")?;

    // ERR_SURREAL_005: SurrealDB requires every ORDER BY field to appear in
    // the SELECT clause. Without `timestamp` here, the query rejects with
    // "Missing order idiom `timestamp` in statement selection". The field is
    // discarded post-deserialisation since `MessageMetrics` doesn't carry it.
    let query = "SELECT \
            tokens_input, tokens_output, cached_tokens, cache_write_tokens, \
            thinking_tokens, cost_usd, model_id_used, timestamp \
        FROM message \
        WHERE workflow_id = $wf_id AND role = 'assistant' \
        ORDER BY timestamp DESC LIMIT 1";

    let rows = db
        .query_json_with_params(
            query,
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load last assistant message metrics");
            format!("Failed to load metrics: {}", e)
        })?;

    let row = match rows.into_iter().next() {
        Some(r) => r,
        None => return Ok(None),
    };

    // serde_json::from_value gracefully handles missing optional fields.
    let metrics: MessageMetrics = serde_json::from_value(row).map_err(|e| {
        error!(error = %e, "Failed to deserialize MessageMetrics");
        format!("Failed to deserialize metrics: {}", e)
    })?;

    Ok(Some(metrics))
}

/// Loads execution blocks (thinking steps + tool calls) for a message,
/// merged and sorted by sequence for chronological display.
///
/// Queries both `tool_execution` and `thinking_step` tables for the given
/// message_id, then merges them into a unified ordered stream of ChatBlocks.
///
/// # Arguments
/// * `message_id` - The message ID to load blocks for
///
/// # Returns
/// Vector of ChatBlocks sorted by sequence number
#[tauri::command]
#[instrument(name = "load_message_blocks", skip(state), fields(message_id = %message_id))]
pub async fn load_message_blocks(
    message_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ChatBlock>, String> {
    info!("Loading message blocks");

    let validated_message_id = validate_uuid_field(&message_id, "message_id")?;

    // Query tool executions for this message
    let tool_query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            message_id,
            agent_id,
            tool_type,
            tool_name,
            server_name,
            input_params,
            output_result,
            success,
            error_message,
            duration_ms,
            iteration,
            sequence,
            created_at
        FROM tool_execution
        WHERE message_id = '{}'
        ORDER BY sequence ASC, created_at ASC"#,
        validated_message_id
    );

    // Query thinking steps for this message
    let thinking_query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            message_id,
            agent_id,
            step_number,
            content,
            duration_ms,
            tokens,
            sequence,
            source,
            created_at
        FROM thinking_step
        WHERE message_id = '{}'
        ORDER BY sequence ASC, step_number ASC"#,
        validated_message_id
    );

    // Execute both queries
    let tool_json = state.db.query_json(&tool_query).await.map_err(|e| {
        error!(error = %e, "Failed to load tool executions for blocks");
        format!("Failed to load tool executions: {}", e)
    })?;

    let thinking_json = state.db.query_json(&thinking_query).await.map_err(|e| {
        error!(error = %e, "Failed to load thinking steps for blocks");
        format!("Failed to load thinking steps: {}", e)
    })?;

    // Query sub-agent executions linked to this message
    let sub_agent_query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            parent_agent_id,
            sub_agent_id,
            sub_agent_name,
            task_description,
            status,
            duration_ms,
            tokens_input,
            tokens_output,
            result_summary,
            error_message,
            parent_message_id,
            created_at,
            completed_at
        FROM sub_agent_execution
        WHERE parent_message_id = '{}'
        ORDER BY created_at ASC"#,
        validated_message_id
    );

    let sub_agent_json = state.db.query_json(&sub_agent_query).await.map_err(|e| {
        error!(error = %e, "Failed to load sub-agent executions for blocks");
        format!("Failed to load sub-agent executions: {}", e)
    })?;

    // Deserialize tool executions
    let tool_executions: Vec<ToolExecution> = tool_json
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ToolExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize tool executions");
            format!("Failed to deserialize tool executions: {}", e)
        })?;

    // Deserialize thinking steps
    let thinking_steps: Vec<ThinkingStep> = thinking_json
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ThinkingStep>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize thinking steps");
            format!("Failed to deserialize thinking steps: {}", e)
        })?;

    // Deserialize sub-agent executions
    let sub_agent_executions: Vec<SubAgentExecution> = sub_agent_json
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<SubAgentExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize sub-agent executions");
            format!("Failed to deserialize sub-agent executions: {}", e)
        })?;

    // Load internal blocks from sub-agent executions
    // Sub-agent internals are persisted with message_id = execution_id (not assistant message_id)
    // Re-sequence them to appear after primary blocks but before sub_agent completion blocks
    let mut all_tool_executions = tool_executions;
    let mut all_thinking_steps = thinking_steps;

    let primary_max_seq = all_tool_executions
        .iter()
        .map(|t| t.sequence)
        .chain(all_thinking_steps.iter().map(|t| t.sequence))
        .max()
        .unwrap_or(0);

    let mut seq_offset = primary_max_seq + 1;

    for sa in &sub_agent_executions {
        let sa_tool_query = format!(
            r#"SELECT
                meta::id(id) AS id, workflow_id, message_id, agent_id,
                tool_type, tool_name, server_name, input_params, output_result,
                success, error_message, duration_ms, iteration, sequence, created_at
            FROM tool_execution
            WHERE message_id = '{}'
            ORDER BY sequence ASC, created_at ASC"#,
            sa.id
        );
        let sa_thinking_query = format!(
            r#"SELECT
                meta::id(id) AS id, workflow_id, message_id, agent_id,
                step_number, content, duration_ms, tokens, sequence, source, created_at
            FROM thinking_step
            WHERE message_id = '{}'
            ORDER BY sequence ASC, step_number ASC"#,
            sa.id
        );

        if let Ok(sa_tools_json) = state.db.query_json(&sa_tool_query).await {
            let mut sa_tools: Vec<ToolExecution> = sa_tools_json
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect();
            // Re-sequence to avoid conflicts with primary blocks
            for t in &mut sa_tools {
                t.sequence += seq_offset;
            }
            seq_offset += sa_tools.iter().map(|t| t.sequence).max().unwrap_or(0) + 1;
            all_tool_executions.extend(sa_tools);
        }

        if let Ok(sa_thinking_json) = state.db.query_json(&sa_thinking_query).await {
            let mut sa_thinking: Vec<ThinkingStep> = sa_thinking_json
                .into_iter()
                .filter_map(|v| serde_json::from_value(v).ok())
                .collect();
            // Re-sequence to avoid conflicts with primary blocks
            for t in &mut sa_thinking {
                t.sequence += seq_offset;
            }
            seq_offset += sa_thinking.iter().map(|t| t.sequence).max().unwrap_or(0) + 1;
            all_thinking_steps.extend(sa_thinking);
        }
    }

    // Merge into unified ChatBlocks sorted by sequence
    let blocks = merge_into_chat_blocks(
        &all_tool_executions,
        &all_thinking_steps,
        &sub_agent_executions,
    );

    info!(
        tool_count = all_tool_executions.len(),
        thinking_count = all_thinking_steps.len(),
        sub_agent_count = sub_agent_executions.len(),
        total_blocks = blocks.len(),
        "Message blocks loaded (including sub-agent internals)"
    );

    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    /// Inserts an `assistant` row directly (matches the columns the metrics
    /// query reads). `timestamp` is offset to allow ordering tests without
    /// having to wait between inserts.
    #[allow(clippy::too_many_arguments)]
    async fn insert_assistant_message(
        db: &crate::db::DBClient,
        workflow_id: &str,
        tokens_input: i64,
        tokens_output: i64,
        cost_usd: f64,
        model_id_used: &str,
        timestamp_offset_secs: i64,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let offset = format!("{}s", timestamp_offset_secs);
        let query = format!(
            "CREATE message:`{id}` SET \
                workflow_id = $wf_id, \
                role = 'assistant', \
                content = 'test response', \
                tokens = {sum}, \
                tokens_input = {tokens_input}, \
                tokens_output = {tokens_output}, \
                cost_usd = {cost_usd}, \
                model_id_used = $model_id, \
                timestamp = time::now() + <duration>$offset",
            sum = tokens_input + tokens_output
        );
        db.db
            .query(&query)
            .bind(("wf_id", workflow_id.to_string()))
            .bind(("model_id", model_id_used.to_string()))
            .bind(("offset", offset))
            .await
            .expect("Insert message query failed")
            .check()
            .expect("CREATE message failed validation");
        id
    }

    // Regression: the production query rejected with ERR_SURREAL_005
    // ("Missing order idiom `timestamp` in statement selection") because
    // `timestamp` was used in ORDER BY but absent from SELECT. This test
    // exercises the real query so the same bug can never silently land again.
    #[tokio::test]
    async fn last_assistant_metrics_query_runs_against_real_db() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        insert_assistant_message(&state.db, &workflow_id, 1234, 567, 0.0123, "gpt-x", 0).await;

        let metrics = last_assistant_message_metrics_core(&state.db, &workflow_id)
            .await
            .expect("query must succeed (regression: ERR_SURREAL_005)")
            .expect("seeded message must be returned");

        assert_eq!(metrics.tokens_input, Some(1234));
        assert_eq!(metrics.tokens_output, Some(567));
        assert_eq!(metrics.cost_usd, Some(0.0123));
        assert_eq!(metrics.model_id_used.as_deref(), Some("gpt-x"));
    }

    #[tokio::test]
    async fn last_assistant_metrics_returns_none_for_empty_workflow() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        let result = last_assistant_message_metrics_core(&state.db, &workflow_id)
            .await
            .expect("empty workflow must succeed, not error");

        assert!(result.is_none(), "no assistant rows -> None");
    }

    #[tokio::test]
    async fn last_assistant_metrics_picks_most_recent_row() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        // Three iterations of a continuation; the LIMIT 1 + ORDER BY DESC
        // must return the LAST one (highest cost in this fixture).
        insert_assistant_message(&state.db, &workflow_id, 100, 50, 0.001, "model-a", 0).await;
        insert_assistant_message(&state.db, &workflow_id, 200, 100, 0.002, "model-b", 1).await;
        insert_assistant_message(&state.db, &workflow_id, 300, 150, 0.003, "model-c", 2).await;

        let metrics = last_assistant_message_metrics_core(&state.db, &workflow_id)
            .await
            .expect("query OK")
            .expect("rows present");

        assert_eq!(metrics.tokens_input, Some(300));
        assert_eq!(metrics.cost_usd, Some(0.003));
        assert_eq!(metrics.model_id_used.as_deref(), Some("model-c"));
    }

    #[tokio::test]
    async fn last_assistant_metrics_validates_workflow_id() {
        let (state, _db_guard) = setup_test_state().await;
        let result = last_assistant_message_metrics_core(&state.db, "not-a-uuid").await;
        assert!(
            result.is_err(),
            "invalid UUID must be rejected at validation"
        );
    }
}
