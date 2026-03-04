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
        merge_into_chat_blocks, ChatBlock, Message, MessageCreate, PaginatedMessages, ThinkingStep,
        ToolExecution,
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

    // Build MessageCreate payload
    let message = MessageCreate {
        workflow_id: validated_workflow_id,
        role: validated_role,
        content,
        tokens: tokens_output.unwrap_or(0) as usize,
        tokens_input,
        tokens_output,
        model,
        provider,
        cost_usd: None, // Cost calculation is provider-specific (future enhancement)
        duration_ms,
        thinking_tokens,
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
    // ORDER BY timestamp ASC for chronological order
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
            timestamp
        FROM message
        WHERE workflow_id = '{}'
        ORDER BY timestamp ASC"#,
        validated_workflow_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
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

    // Get total count
    let count_query = format!(
        "SELECT count() FROM message WHERE workflow_id = '{}' GROUP ALL",
        validated_workflow_id
    );
    let count_result: Vec<serde_json::Value> =
        state.db.query(&count_query).await.unwrap_or_default();

    let total = extract_count(&count_result) as u32;

    // Load paginated messages
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
            timestamp
        FROM message
        WHERE workflow_id = '{}'
        ORDER BY timestamp ASC
        LIMIT {} START {}"#,
        validated_workflow_id, limit, offset
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
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

    // First count existing messages
    let count_query = format!(
        "SELECT count() FROM message WHERE workflow_id = '{}' GROUP ALL",
        validated_workflow_id
    );
    let count_result: Vec<serde_json::Value> =
        state.db.query(&count_query).await.unwrap_or_default();

    let count = extract_count(&count_result);

    // Delete all messages for the workflow
    state
        .db
        .execute(&format!(
            "DELETE message WHERE workflow_id = '{}'",
            validated_workflow_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to clear workflow messages");
            format!("Failed to clear workflow messages: {}", e)
        })?;

    info!(count = count, "Workflow messages cleared");
    Ok(count)
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

    // Merge into unified ChatBlocks sorted by sequence
    let blocks = merge_into_chat_blocks(&tool_executions, &thinking_steps);

    info!(
        tool_count = tool_executions.len(),
        thinking_count = thinking_steps.len(),
        total_blocks = blocks.len(),
        "Message blocks loaded"
    );

    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_message_content_len() {
        // 100KB should be enough for most message content
        assert_eq!(cmd_const::MAX_MESSAGE_CONTENT_LEN, 100_000);
    }

    #[test]
    fn test_valid_roles() {
        let valid_roles = vec!["user", "assistant", "system"];
        for role in valid_roles {
            assert!(matches!(role, "user" | "assistant" | "system"));
        }
    }

    #[test]
    fn test_invalid_role_detection() {
        let invalid_roles = vec!["admin", "bot", ""];
        for role in invalid_roles {
            assert!(!matches!(role, "user" | "assistant" | "system"));
        }
    }
}
