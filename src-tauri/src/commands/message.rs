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
use std::collections::{HashMap, HashSet};
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

/// Loads ChatBlocks for every assistant message of a workflow in a single
/// round-trip.
///
/// Internally executes 3 scoped queries (`tool_execution`, `thinking_step`,
/// `sub_agent_execution`) filtered by `workflow_id`, regroups the rows by
/// owning message id (primary message vs sub-agent internals), and merges
/// each group into a unified `ChatBlock` stream sorted by sequence.
///
/// # Arguments
/// * `workflow_id` - Workflow ID whose blocks to batch-load
///
/// # Returns
/// Map of `message_id` -> sequenced `Vec<ChatBlock>`. Messages with no blocks
/// are absent (callers should treat missing entries as empty).
#[tauri::command]
#[instrument(name = "load_workflow_blocks", skip(state), fields(workflow_id = %workflow_id))]
pub async fn load_workflow_blocks(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<HashMap<String, Vec<ChatBlock>>, String> {
    load_workflow_blocks_core(&state.db, &workflow_id).await
}

/// Core implementation of `load_workflow_blocks`, extracted so it can be
/// exercised by integration tests against a real SurrealDB instance.
pub(crate) async fn load_workflow_blocks_core(
    db: &crate::db::DBClient,
    workflow_id: &str,
) -> Result<HashMap<String, Vec<ChatBlock>>, String> {
    info!("Loading workflow blocks");

    let validated_workflow_id = validate_uuid_field(workflow_id, "workflow_id")?;
    let wf_param = vec![(
        "wf_id".to_string(),
        serde_json::json!(validated_workflow_id),
    )];

    // 1. Tool executions for the whole workflow (primary + sub-agent internals).
    let tool_query = "SELECT \
            meta::id(id) AS id, workflow_id, message_id, agent_id, \
            tool_type, tool_name, server_name, input_params, output_result, \
            success, error_message, duration_ms, iteration, sequence, created_at \
        FROM tool_execution \
        WHERE workflow_id = $wf_id \
        ORDER BY sequence ASC, created_at ASC";

    let tool_json = db
        .query_json_with_params(tool_query, wf_param.clone())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to batch-load tool executions");
            format!("Failed to batch-load tool executions: {}", e)
        })?;
    let tool_executions: Vec<ToolExecution> = tool_json
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ToolExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize batched tool executions");
            format!("Failed to deserialize tool executions: {}", e)
        })?;

    // 2. Thinking steps for the whole workflow.
    let thinking_query = "SELECT \
            meta::id(id) AS id, workflow_id, message_id, agent_id, \
            step_number, content, duration_ms, tokens, sequence, source, created_at \
        FROM thinking_step \
        WHERE workflow_id = $wf_id \
        ORDER BY sequence ASC, step_number ASC";

    let thinking_json = db
        .query_json_with_params(thinking_query, wf_param.clone())
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to batch-load thinking steps");
            format!("Failed to batch-load thinking steps: {}", e)
        })?;
    let thinking_steps: Vec<ThinkingStep> = thinking_json
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ThinkingStep>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize batched thinking steps");
            format!("Failed to deserialize thinking steps: {}", e)
        })?;

    // 3. Sub-agent executions for the whole workflow.
    let sub_agent_query = "SELECT \
            meta::id(id) AS id, workflow_id, parent_agent_id, sub_agent_id, \
            sub_agent_name, task_description, status, duration_ms, \
            tokens_input, tokens_output, result_summary, error_message, \
            parent_message_id, created_at, completed_at \
        FROM sub_agent_execution \
        WHERE workflow_id = $wf_id \
        ORDER BY created_at ASC";

    let sub_agent_json = db
        .query_json_with_params(sub_agent_query, wf_param)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to batch-load sub-agent executions");
            format!("Failed to batch-load sub-agent executions: {}", e)
        })?;
    let sub_agent_executions: Vec<SubAgentExecution> = sub_agent_json
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<SubAgentExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize batched sub-agent executions");
            format!("Failed to deserialize sub-agent executions: {}", e)
        })?;

    // Bucket primary tool/thinking rows by their owning message id, separating
    // sub-agent internals (where `message_id` matches a sub_agent_execution.id).
    let sub_agent_ids: HashSet<String> = sub_agent_executions
        .iter()
        .map(|sa| sa.id.clone())
        .collect();

    let mut primary_tools: HashMap<String, Vec<ToolExecution>> = HashMap::new();
    let mut sub_agent_tools: HashMap<String, Vec<ToolExecution>> = HashMap::new();
    for t in tool_executions {
        let bucket = if sub_agent_ids.contains(&t.message_id) {
            sub_agent_tools.entry(t.message_id.clone()).or_default()
        } else {
            primary_tools.entry(t.message_id.clone()).or_default()
        };
        bucket.push(t);
    }

    let mut primary_thinking: HashMap<String, Vec<ThinkingStep>> = HashMap::new();
    let mut sub_agent_thinking: HashMap<String, Vec<ThinkingStep>> = HashMap::new();
    for ts in thinking_steps {
        let bucket = if sub_agent_ids.contains(&ts.message_id) {
            sub_agent_thinking.entry(ts.message_id.clone()).or_default()
        } else {
            primary_thinking.entry(ts.message_id.clone()).or_default()
        };
        bucket.push(ts);
    }

    let mut sub_agents_by_parent: HashMap<String, Vec<SubAgentExecution>> = HashMap::new();
    for sa in &sub_agent_executions {
        // A sub-agent without a parent_message_id is not yet attached to an
        // assistant message (in-flight or unfinished); skip it so its blocks
        // don't leak into a foreign bucket.
        if let Some(parent_id) = sa.parent_message_id.clone() {
            sub_agents_by_parent
                .entry(parent_id)
                .or_default()
                .push(sa.clone());
        }
    }

    // Set of every primary assistant message that owns at least one block.
    let mut primary_message_ids: HashSet<String> = HashSet::new();
    primary_message_ids.extend(primary_tools.keys().cloned());
    primary_message_ids.extend(primary_thinking.keys().cloned());
    primary_message_ids.extend(sub_agents_by_parent.keys().cloned());

    // Build agent_id -> agent_name lookup so merge_into_chat_blocks can
    // project a human-readable label onto each Tool/Thinking block. Issued
    // as a single bulk query (the workflow rarely contains more than a
    // handful of distinct agent_ids). A miss leaves agent_name null on the
    // block; the frontend falls back on a truncated agent_id (e.g. sub-agent
    // garbage-collected from the registry before replay).
    let mut agent_ids_seen: HashSet<String> = HashSet::new();
    for tools in primary_tools.values().chain(sub_agent_tools.values()) {
        for t in tools {
            if !t.agent_id.is_empty() {
                agent_ids_seen.insert(t.agent_id.clone());
            }
        }
    }
    for thinks in primary_thinking.values().chain(sub_agent_thinking.values()) {
        for ts in thinks {
            if !ts.agent_id.is_empty() {
                agent_ids_seen.insert(ts.agent_id.clone());
            }
        }
    }

    let agent_name_lookup: HashMap<String, String> = if agent_ids_seen.is_empty() {
        HashMap::new()
    } else {
        let ids_vec: Vec<String> = agent_ids_seen.into_iter().collect();
        let lookup_query = "SELECT meta::id(id) AS id, name FROM agent \
                            WHERE meta::id(id) IN $ids";
        let lookup_param = vec![("ids".to_string(), serde_json::json!(ids_vec))];
        match db.query_json_with_params(lookup_query, lookup_param).await {
            Ok(rows) => rows
                .into_iter()
                .filter_map(|row| {
                    let id = row.get("id").and_then(|v| v.as_str())?.to_string();
                    let name = row.get("name").and_then(|v| v.as_str())?.to_string();
                    Some((id, name))
                })
                .collect(),
            Err(e) => {
                warn!(error = %e, "Failed to bulk-load agent names; replay falls back to agent_id");
                HashMap::new()
            }
        }
    };

    let mut result: HashMap<String, Vec<ChatBlock>> = HashMap::new();

    for message_id in primary_message_ids {
        let mut all_tools = primary_tools.remove(&message_id).unwrap_or_default();
        let mut all_thinking = primary_thinking.remove(&message_id).unwrap_or_default();
        let owned_sub_agents = sub_agents_by_parent
            .get(&message_id)
            .cloned()
            .unwrap_or_default();

        let primary_max_seq = all_tools
            .iter()
            .map(|t| t.sequence)
            .chain(all_thinking.iter().map(|t| t.sequence))
            .max()
            .unwrap_or(0);
        let mut seq_offset = primary_max_seq + 1;

        // Re-sequence each sub-agent's internal blocks so they appear after
        // the primary blocks but before later sub-agents.
        for sa in &owned_sub_agents {
            if let Some(mut sa_tools) = sub_agent_tools.remove(&sa.id) {
                for t in &mut sa_tools {
                    t.sequence += seq_offset;
                }
                let max = sa_tools.iter().map(|t| t.sequence).max().unwrap_or(0);
                seq_offset = max + 1;
                all_tools.extend(sa_tools);
            }
            if let Some(mut sa_thinking) = sub_agent_thinking.remove(&sa.id) {
                for t in &mut sa_thinking {
                    t.sequence += seq_offset;
                }
                let max = sa_thinking.iter().map(|t| t.sequence).max().unwrap_or(0);
                seq_offset = max + 1;
                all_thinking.extend(sa_thinking);
            }
        }

        let blocks = merge_into_chat_blocks(
            &all_tools,
            &all_thinking,
            &owned_sub_agents,
            &agent_name_lookup,
        );
        if !blocks.is_empty() {
            result.insert(message_id, blocks);
        }
    }

    info!(
        message_count = result.len(),
        sub_agent_count = sub_agent_executions.len(),
        "Workflow blocks loaded (batched)"
    );

    Ok(result)
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

    /// Helper: insert a tool_execution row directly with a known sequence.
    #[allow(clippy::too_many_arguments)]
    async fn insert_tool_execution(
        db: &crate::db::DBClient,
        workflow_id: &str,
        message_id: &str,
        agent_id: &str,
        tool_name: &str,
        sequence: u32,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let query = format!(
            "CREATE tool_execution:`{id}` SET \
                workflow_id = $wf_id, message_id = $msg_id, agent_id = $agent_id, \
                tool_type = 'local', tool_name = $tool_name, \
                input_params = '{{}}', output_result = '{{}}', success = true, \
                duration_ms = 5, iteration = 0, sequence = $seq, created_at = time::now()"
        );
        db.db
            .query(&query)
            .bind(("wf_id", workflow_id.to_string()))
            .bind(("msg_id", message_id.to_string()))
            .bind(("agent_id", agent_id.to_string()))
            .bind(("tool_name", tool_name.to_string()))
            .bind(("seq", sequence))
            .await
            .expect("insert tool_execution failed")
            .check()
            .expect("CREATE tool_execution validation failed");
    }

    /// Helper: insert a thinking_step row directly.
    async fn insert_thinking_step(
        db: &crate::db::DBClient,
        workflow_id: &str,
        message_id: &str,
        agent_id: &str,
        sequence: u32,
        step_number: u32,
    ) {
        let id = uuid::Uuid::new_v4().to_string();
        let query = format!(
            "CREATE thinking_step:`{id}` SET \
                workflow_id = $wf_id, message_id = $msg_id, agent_id = $agent_id, \
                step_number = $step, content = 'thinking...', \
                duration_ms = 1, tokens = 1, sequence = $seq, source = 'model_thinking', \
                created_at = time::now()"
        );
        db.db
            .query(&query)
            .bind(("wf_id", workflow_id.to_string()))
            .bind(("msg_id", message_id.to_string()))
            .bind(("agent_id", agent_id.to_string()))
            .bind(("step", step_number))
            .bind(("seq", sequence))
            .await
            .expect("insert thinking_step failed")
            .check()
            .expect("CREATE thinking_step validation failed");
    }

    #[tokio::test]
    async fn load_workflow_blocks_returns_empty_for_workflow_without_blocks() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();

        let result = load_workflow_blocks_core(&state.db, &workflow_id)
            .await
            .expect("must succeed on empty workflow");

        assert!(result.is_empty(), "no blocks -> empty map");
    }

    #[tokio::test]
    async fn load_workflow_blocks_groups_blocks_by_message() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let msg_a = uuid::Uuid::new_v4().to_string();
        let msg_b = uuid::Uuid::new_v4().to_string();
        let agent_id = uuid::Uuid::new_v4().to_string();

        // msg_a: 1 thinking step + 1 tool execution
        insert_thinking_step(&state.db, &workflow_id, &msg_a, &agent_id, 1, 1).await;
        insert_tool_execution(&state.db, &workflow_id, &msg_a, &agent_id, "MemoryTool", 2).await;

        // msg_b: 1 tool execution only
        insert_tool_execution(&state.db, &workflow_id, &msg_b, &agent_id, "SearchTool", 1).await;

        let result = load_workflow_blocks_core(&state.db, &workflow_id)
            .await
            .expect("query OK");

        assert_eq!(result.len(), 2, "two messages with blocks");
        let blocks_a = result.get(&msg_a).expect("msg_a present");
        assert_eq!(blocks_a.len(), 2);
        let blocks_b = result.get(&msg_b).expect("msg_b present");
        assert_eq!(blocks_b.len(), 1);
    }

    #[tokio::test]
    async fn load_workflow_blocks_validates_workflow_id() {
        let (state, _db_guard) = setup_test_state().await;
        let result = load_workflow_blocks_core(&state.db, "not-a-uuid").await;
        assert!(result.is_err(), "invalid UUID must be rejected");
    }

    #[tokio::test]
    async fn load_workflow_blocks_isolates_workflows() {
        let (state, _db_guard) = setup_test_state().await;
        let wf_keep = uuid::Uuid::new_v4().to_string();
        let wf_other = uuid::Uuid::new_v4().to_string();
        let msg_keep = uuid::Uuid::new_v4().to_string();
        let msg_other = uuid::Uuid::new_v4().to_string();
        let agent_id = uuid::Uuid::new_v4().to_string();

        insert_tool_execution(&state.db, &wf_keep, &msg_keep, &agent_id, "Keep", 1).await;
        insert_tool_execution(&state.db, &wf_other, &msg_other, &agent_id, "Other", 1).await;

        let result = load_workflow_blocks_core(&state.db, &wf_keep)
            .await
            .expect("query OK");

        assert_eq!(result.len(), 1);
        assert!(result.contains_key(&msg_keep));
        assert!(!result.contains_key(&msg_other));
    }
}
