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
        merge_into_chat_blocks, shift_sequences, sub_agent::SubAgentExecution, ChatBlock, Message,
        MessageAttachment, MessageCreate, MessageMetrics, PaginatedMessages, ThinkingStep,
        ToolExecution,
    },
    security::validate_uuid_field,
    AppState,
};

/// Maximum number of attachments per user message (also enforced UI-side).
pub(crate) const MAX_ATTACHMENTS_PER_MESSAGE: usize = 8;

/// Maximum base64 payload size per attachment. 4 MB binary expands to
/// ~5.33 MB base64; we allow a tiny safety margin.
pub(crate) const MAX_ATTACHMENT_BASE64_BYTES: usize = (4 * 1024 * 1024 * 4 / 3) + 256;

/// Maximum length of the `name` field on an attachment (display-only).
/// Long enough to fit any reasonable filename, short enough that a 1 MB
/// adversarial blob in the name field is rejected at the boundary instead
/// of bloating logs / display surfaces.
pub(crate) const MAX_ATTACHMENT_NAME_LEN: usize = 512;

/// MIME types accepted by the multimodal pipeline. Mirrored client-side in
/// `image-processing.ts` so a rejection at either end yields a clear error.
pub(crate) const ALLOWED_ATTACHMENT_MIMES: &[&str] =
    &["image/png", "image/jpeg", "image/webp", "image/gif"];

/// Validates a set of attachments for a user message. Returns the validated
/// vector (passes-through on success). Errors describe the first failure.
///
/// `model_supports_vision` gates image attachments at the IPC boundary: when
/// `false`, any image attachment is rejected before the row is persisted.
/// This is the backend half of the defense in depth — the UI also blocks
/// the paste/picker/drop in `ChatInput.svelte` — but this layer is the
/// source of truth because the front-end can be bypassed (custom callers,
/// stale state, ...).
fn validate_attachments(
    role: &str,
    attachments: &[MessageAttachment],
    model_supports_vision: bool,
) -> Result<(), String> {
    if role != "user" {
        return Err(format!(
            "Attachments are only allowed on user messages (got role '{}')",
            role
        ));
    }
    if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(format!(
            "Too many attachments ({}, max {})",
            attachments.len(),
            MAX_ATTACHMENTS_PER_MESSAGE
        ));
    }
    for (i, att) in attachments.iter().enumerate() {
        if att.kind != "image" {
            return Err(format!(
                "Attachment {} has unsupported kind '{}', only 'image' is supported",
                i, att.kind
            ));
        }
        if !model_supports_vision {
            return Err(format!(
                "Attachment {} is an image but the workflow's model does not support vision",
                i
            ));
        }
        if !ALLOWED_ATTACHMENT_MIMES.contains(&att.mime_type.as_str()) {
            return Err(format!(
                "Attachment {} has unsupported MIME type '{}'",
                i, att.mime_type
            ));
        }
        if att.data_base64.is_empty() {
            return Err(format!("Attachment {} has empty data_base64", i));
        }
        if att.data_base64.len() > MAX_ATTACHMENT_BASE64_BYTES {
            return Err(format!(
                "Attachment {} exceeds max size ({} base64 bytes, cap {})",
                i,
                att.data_base64.len(),
                MAX_ATTACHMENT_BASE64_BYTES
            ));
        }
        // The `name` field is display-only but is persisted as-is into
        // SurrealDB and surfaced in logs / UI. Reject NUL and other control
        // characters (anything below 0x20) so a malicious paste cannot smuggle
        // log-injection bytes or panic the DB driver. Cap the length so a
        // multi-MB blob in `name` is rejected at the boundary instead of
        // bloating downstream rows.
        if let Some(name) = att.name.as_ref() {
            if name.len() > MAX_ATTACHMENT_NAME_LEN {
                return Err(format!(
                    "Attachment {} has name longer than {} bytes",
                    i, MAX_ATTACHMENT_NAME_LEN
                ));
            }
            if name.chars().any(|c| c.is_control()) {
                return Err(format!("Attachment {} name contains control characters", i));
            }
        }
    }
    Ok(())
}
/// Resolves whether the model used by the workflow's primary agent supports
/// image attachments.
///
/// Chains three SELECTs (workflow -> agent.llm.model -> llm_model.supports_vision)
/// and fails closed: any error, missing row, or unconfigured field yields
/// `false`. The caller (`save_message_core`) treats the boolean as the
/// authoritative gate at the IPC boundary — if we cannot prove the model
/// supports vision, we refuse the image attachment.
pub(crate) async fn resolve_workflow_supports_vision(
    db: &crate::db::DBClient,
    workflow_id: &str,
) -> bool {
    let wf_query = "SELECT agent_id FROM workflow WHERE meta::id(id) = $workflow_id";
    let agent_id: Option<String> = match db
        .query_json_with_params(
            wf_query,
            vec![("workflow_id".to_string(), serde_json::json!(workflow_id))],
        )
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .next()
            .and_then(|r| r["agent_id"].as_str().map(String::from)),
        Err(e) => {
            warn!(workflow_id = %workflow_id, error = %e, "Failed to resolve workflow agent_id, defaulting supports_vision to false");
            return false;
        }
    };

    let Some(agent_id) = agent_id else {
        return false;
    };

    // Pull the whole `llm` object: projecting nested fields with `AS` is
    // unreliable with query_json under SCHEMAFULL. Grab provider at the
    // same time so the llm_model lookup can disambiguate api_name
    // collisions between providers.
    let agent_query = "SELECT llm FROM agent WHERE meta::id(id) = $agent_id";
    let (model_name, provider) = match db
        .query_json_with_params(
            agent_query,
            vec![("agent_id".to_string(), serde_json::json!(agent_id))],
        )
        .await
    {
        Ok(rows) => {
            let Some(row) = rows.into_iter().next() else {
                return false;
            };
            let llm = &row["llm"];
            (
                llm["model"].as_str().map(String::from),
                llm["provider"].as_str().map(String::from),
            )
        }
        Err(e) => {
            warn!(agent_id = %agent_id, error = %e, "Failed to resolve agent llm, defaulting supports_vision to false");
            return false;
        }
    };

    let Some(model_name) = model_name else {
        return false;
    };

    let model_query = "SELECT supports_vision FROM llm_model \
         WHERE api_name = $api_name \
           AND ($provider IS NONE OR string::lowercase(provider) = string::lowercase($provider))";
    match db
        .query_json_with_params(
            model_query,
            vec![
                ("api_name".to_string(), serde_json::json!(model_name)),
                ("provider".to_string(), serde_json::json!(provider)),
            ],
        )
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .next()
            .and_then(|r| r["supports_vision"].as_bool())
            .unwrap_or(false),
        Err(e) => {
            warn!(model = %model_name, error = %e, "Failed to resolve model supports_vision, defaulting to false");
            false
        }
    }
}

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
    attachments: Option<Vec<MessageAttachment>>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    save_message_core(
        &state.db,
        SaveMessageParams {
            workflow_id,
            role,
            content,
            tokens_input,
            tokens_output,
            model,
            provider,
            duration_ms,
            thinking_tokens,
            cost_usd,
            cached_tokens,
            cache_write_tokens,
            model_id_used,
            message_id,
            attachments,
        },
    )
    .await
}

/// Parameter bundle for [`save_message_core`].
///
/// Grouped into a single struct so the `_core` helper has one argument and the
/// Tauri-command wrapper does not need 16 positional parameters at the call
/// site. Mirrors the IPC payload 1:1.
#[derive(Debug)]
pub(crate) struct SaveMessageParams {
    pub workflow_id: String,
    pub role: String,
    pub content: String,
    pub tokens_input: Option<u64>,
    pub tokens_output: Option<u64>,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub duration_ms: Option<u64>,
    pub thinking_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub cached_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub model_id_used: Option<String>,
    pub message_id: Option<String>,
    pub attachments: Option<Vec<MessageAttachment>>,
}

/// Validates and inserts a message row. Extracted from the Tauri-command
/// wrapper so integration tests can exercise the full validate + insert path
/// without instantiating `tauri::State`.
pub(crate) async fn save_message_core(
    db: &crate::db::DBClient,
    params: SaveMessageParams,
) -> Result<String, String> {
    info!("Saving message");

    let validated_workflow_id = validate_uuid_field(&params.workflow_id, "workflow_id")?;

    let validated_role = match params.role.as_str() {
        "user" | "assistant" | "system" => params.role.clone(),
        _ => {
            warn!(role = %params.role, "Invalid message role");
            return Err(format!(
                "Invalid message role: {}. Expected user, assistant, or system",
                params.role
            ));
        }
    };

    if params.content.is_empty() && params.attachments.as_ref().is_none_or(|a| a.is_empty()) {
        return Err("Message content cannot be empty".to_string());
    }
    if params.content.len() > cmd_const::MAX_MESSAGE_CONTENT_LEN {
        return Err(format!(
            "Message content exceeds maximum length of {} characters",
            cmd_const::MAX_MESSAGE_CONTENT_LEN
        ));
    }

    if let Some(ref atts) = params.attachments {
        if !atts.is_empty() {
            // Resolve once (single SELECT chain), pass the bool to the pure
            // validator. Fails closed: an unresolved workflow/agent/model
            // chain yields `false`, so an image attachment is rejected with
            // a clear error rather than silently persisted.
            let supports_vision =
                resolve_workflow_supports_vision(db, &validated_workflow_id).await;
            validate_attachments(&validated_role, atts, supports_vision)?;
        }
    }

    let message_id = match params.message_id {
        Some(id) => validate_uuid_field(&id, "message_id")?,
        None => Uuid::new_v4().to_string(),
    };

    let legacy_tokens =
        (params.tokens_input.unwrap_or(0) + params.tokens_output.unwrap_or(0)) as usize;
    let normalized_attachments = params.attachments.filter(|a| !a.is_empty());
    let message = MessageCreate {
        workflow_id: validated_workflow_id,
        role: validated_role,
        content: params.content,
        tokens: legacy_tokens,
        tokens_input: params.tokens_input,
        tokens_output: params.tokens_output,
        model: params.model,
        provider: params.provider,
        cost_usd: params.cost_usd,
        duration_ms: params.duration_ms,
        thinking_tokens: params.thinking_tokens,
        cached_tokens: params.cached_tokens,
        cache_write_tokens: params.cache_write_tokens,
        model_id_used: params.model_id_used,
        attachments: normalized_attachments,
    };

    let id = db
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
    load_workflow_messages_core(&state.db, &workflow_id).await
}

/// Loads all messages for a workflow with the canonical field selection.
///
/// Extracted as a `_core` helper so `load_workflow_full_state` can delegate
/// here — a single source of truth for the SELECT, so adding a column does
/// not require updating divergent copies.
pub(crate) async fn load_workflow_messages_core(
    db: &crate::db::DBClient,
    workflow_id: &str,
) -> Result<Vec<Message>, String> {
    info!("Loading workflow messages");

    let validated_workflow_id = validate_uuid_field(workflow_id, "workflow_id")?;

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
            attachments,
            timestamp
        FROM message
        WHERE workflow_id = $wf_id
        ORDER BY timestamp ASC"#;

    let json_results = db
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
            attachments,
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
    // `cached_tokens` / `cache_write_tokens` / `thinking_tokens` are pulled
    // through here so `merge_into_chat_blocks` can project them into
    // `SubAgentBlockData`.
    let sub_agent_query = "SELECT \
            meta::id(id) AS id, workflow_id, parent_agent_id, sub_agent_id, \
            sub_agent_name, task_description, status, duration_ms, \
            tokens_input, tokens_output, cost_usd, \
            cached_tokens, cache_write_tokens, thinking_tokens, \
            result_summary, error_message, \
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
        // the primary blocks but before later sub-agents. Tools and thinking
        // steps of one sub-agent share its tracker's sequence space, so both
        // families shift by the SAME offset — separate offsets would push
        // every thinking step after every tool and destroy the interleaving.
        for sa in &owned_sub_agents {
            let mut sa_tools = sub_agent_tools.remove(&sa.id).unwrap_or_default();
            let mut sa_thinking = sub_agent_thinking.remove(&sa.id).unwrap_or_default();
            seq_offset = shift_sequences(&mut sa_tools, &mut sa_thinking, seq_offset);
            all_tools.extend(sa_tools);
            all_thinking.extend(sa_thinking);
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

    fn valid_attachment_with_name(name: Option<&str>) -> MessageAttachment {
        MessageAttachment {
            kind: "image".into(),
            mime_type: "image/png".into(),
            data_base64: "AAAA".into(),
            name: name.map(String::from),
            size_bytes: Some(3),
        }
    }

    #[test]
    fn validate_attachments_accepts_name_with_unicode_filename() {
        // Defence-in-depth check must not flag legitimate filenames with
        // accents, CJK, or emoji — the rule targets control bytes, not
        // multi-byte printable characters.
        let atts = vec![valid_attachment_with_name(Some("Capture_écran.png"))];
        validate_attachments("user", &atts, true).expect("unicode name must be accepted");
    }

    #[test]
    fn validate_attachments_rejects_name_with_null_byte() {
        let atts = vec![valid_attachment_with_name(Some("evil\0name.png"))];
        let err = validate_attachments("user", &atts, true).expect_err("NUL byte must be rejected");
        assert!(err.contains("control characters"), "got: {}", err);
    }

    #[test]
    fn validate_attachments_rejects_name_with_newline() {
        // Newlines / carriage returns enable log-injection: a name like
        // `"foo\nINFO: fake log line"` would split a log entry in two.
        let atts = vec![valid_attachment_with_name(Some("foo\nbar.png"))];
        let err = validate_attachments("user", &atts, true).expect_err("LF must be rejected");
        assert!(err.contains("control characters"), "got: {}", err);
    }

    #[test]
    fn validate_attachments_rejects_oversize_name() {
        let huge = "a".repeat(MAX_ATTACHMENT_NAME_LEN + 1);
        let atts = vec![valid_attachment_with_name(Some(&huge))];
        let err =
            validate_attachments("user", &atts, true).expect_err("oversize name must be rejected");
        assert!(err.contains("longer than"), "got: {}", err);
    }

    #[test]
    fn validate_attachments_accepts_missing_name() {
        let atts = vec![valid_attachment_with_name(None)];
        validate_attachments("user", &atts, true).expect("missing name must be accepted");
    }

    #[test]
    fn validate_attachments_rejects_image_when_model_lacks_vision() {
        // Backend half of the vision gate (defense in depth): images must
        // be refused at the IPC boundary if the workflow's model does not
        // support them. The UI also blocks paste/picker/drop, but this is
        // the source of truth — bypassing the UI must not silently persist
        // images that the agent cannot consume.
        let atts = vec![valid_attachment_with_name(Some("photo.png"))];
        let err = validate_attachments("user", &atts, false)
            .expect_err("non-vision model must reject image attachments");
        assert!(err.contains("does not support vision"), "got: {}", err);
    }

    #[test]
    fn validate_attachments_accepts_image_when_model_supports_vision() {
        let atts = vec![valid_attachment_with_name(Some("photo.png"))];
        validate_attachments("user", &atts, true)
            .expect("vision-capable model must accept image attachments");
    }

    #[test]
    fn validate_attachments_no_op_when_empty_regardless_of_vision() {
        // Empty attachment lists short-circuit at the caller, but the
        // validator itself should not depend on vision support when there
        // is no image to gate.
        validate_attachments("user", &[], false).expect("empty list must always pass");
        validate_attachments("user", &[], true).expect("empty list must always pass");
    }

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
