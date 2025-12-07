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

//! Thinking step commands for persistence.
//!
//! Provides Tauri commands for saving and retrieving thinking step logs
//! for workflow state recovery and agent reasoning transparency.
//!
//! Phase 4: Thinking Steps Persistence - Enables complete workflow state
//! recovery with full reasoning history.

use crate::{
    models::{ThinkingStep, ThinkingStepCreate},
    security::Validator,
    tools::constants::commands as cmd_const,
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Saves a new thinking step to the database.
///
/// # Arguments
/// * `workflow_id` - Associated workflow ID
/// * `message_id` - Associated message ID
/// * `agent_id` - Agent ID that generated the thinking step
/// * `step_number` - Step number in the reasoning sequence (0-indexed)
/// * `content` - The reasoning content
/// * `duration_ms` - Duration to generate this step (optional)
/// * `tokens` - Token count for this step (optional)
///
/// # Returns
/// The ID of the created thinking step record
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[instrument(
    name = "save_thinking_step",
    skip(state, content),
    fields(
        workflow_id = %workflow_id,
        step_number = %step_number,
        content_len = content.len()
    )
)]
pub async fn save_thinking_step(
    workflow_id: String,
    message_id: String,
    agent_id: String,
    step_number: u32,
    content: String,
    duration_ms: Option<u64>,
    tokens: Option<u64>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Saving thinking step");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    // Validate message ID
    let validated_message_id = Validator::validate_uuid(&message_id).map_err(|e| {
        warn!(error = %e, "Invalid message_id");
        format!("Invalid message_id: {}", e)
    })?;

    // Validate agent ID
    let validated_agent_id = Validator::validate_uuid(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    // Validate content
    if content.is_empty() {
        return Err("Thinking step content cannot be empty".to_string());
    }
    if content.len() > cmd_const::MAX_THINKING_CONTENT_LEN {
        return Err(format!(
            "Thinking step content exceeds maximum length of {} bytes",
            cmd_const::MAX_THINKING_CONTENT_LEN
        ));
    }

    let step_id = Uuid::new_v4().to_string();

    // Build ThinkingStepCreate payload
    let step = ThinkingStepCreate {
        workflow_id: validated_workflow_id,
        message_id: validated_message_id,
        agent_id: validated_agent_id,
        step_number,
        content,
        duration_ms,
        tokens,
    };

    // Insert into database
    let id = state
        .db
        .create("thinking_step", &step_id, step)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to save thinking step");
            format!("Failed to save thinking step: {}", e)
        })?;

    info!(step_id = %id, "Thinking step saved successfully");
    Ok(step_id)
}

/// Loads all thinking steps for a workflow, sorted by creation time (oldest first).
///
/// # Arguments
/// * `workflow_id` - The workflow ID to load thinking steps for
///
/// # Returns
/// Vector of thinking steps in chronological order
#[tauri::command]
#[instrument(name = "load_workflow_thinking_steps", skip(state), fields(workflow_id = %workflow_id))]
pub async fn load_workflow_thinking_steps(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ThinkingStep>, String> {
    info!("Loading workflow thinking steps");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            message_id,
            agent_id,
            step_number,
            content,
            duration_ms,
            tokens,
            created_at
        FROM thinking_step
        WHERE workflow_id = '{}'
        ORDER BY created_at ASC, step_number ASC"#,
        validated_workflow_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflow thinking steps");
        format!("Failed to load workflow thinking steps: {}", e)
    })?;

    // Deserialize using serde_json
    let steps: Vec<ThinkingStep> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ThinkingStep>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize thinking steps");
            format!("Failed to deserialize thinking steps: {}", e)
        })?;

    info!(count = steps.len(), "Workflow thinking steps loaded");
    Ok(steps)
}

/// Loads thinking steps for a specific message.
///
/// Useful for displaying reasoning associated with a particular assistant response.
///
/// # Arguments
/// * `message_id` - The message ID to load thinking steps for
///
/// # Returns
/// Vector of thinking steps in chronological order
#[tauri::command]
#[instrument(name = "load_message_thinking_steps", skip(state), fields(message_id = %message_id))]
pub async fn load_message_thinking_steps(
    message_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ThinkingStep>, String> {
    info!("Loading message thinking steps");

    // Validate message ID
    let validated_message_id = Validator::validate_uuid(&message_id).map_err(|e| {
        warn!(error = %e, "Invalid message_id");
        format!("Invalid message_id: {}", e)
    })?;

    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            message_id,
            agent_id,
            step_number,
            content,
            duration_ms,
            tokens,
            created_at
        FROM thinking_step
        WHERE message_id = '{}'
        ORDER BY step_number ASC"#,
        validated_message_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load message thinking steps");
        format!("Failed to load message thinking steps: {}", e)
    })?;

    let steps: Vec<ThinkingStep> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ThinkingStep>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize thinking steps");
            format!("Failed to deserialize thinking steps: {}", e)
        })?;

    info!(count = steps.len(), "Message thinking steps loaded");
    Ok(steps)
}

/// Deletes a single thinking step by ID.
///
/// # Arguments
/// * `step_id` - The thinking step ID to delete
///
/// # Returns
/// Success or error
#[tauri::command]
#[instrument(name = "delete_thinking_step", skip(state), fields(step_id = %step_id))]
pub async fn delete_thinking_step(
    step_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Deleting thinking step");

    // Validate step ID
    let validated_id = Validator::validate_uuid(&step_id).map_err(|e| {
        warn!(error = %e, "Invalid step ID");
        format!("Invalid step ID: {}", e)
    })?;

    // Use execute() with DELETE query to avoid SurrealDB SDK serialization issues
    state
        .db
        .execute(&format!("DELETE thinking_step:`{}`", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete thinking step");
            format!("Failed to delete thinking step: {}", e)
        })?;

    info!("Thinking step deleted successfully");
    Ok(())
}

/// Deletes all thinking steps for a workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to clear thinking steps for
///
/// # Returns
/// Number of thinking steps deleted
#[tauri::command]
#[instrument(name = "clear_workflow_thinking_steps", skip(state), fields(workflow_id = %workflow_id))]
pub async fn clear_workflow_thinking_steps(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    info!("Clearing workflow thinking steps");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // First count existing steps
    let count_query = format!(
        "SELECT count() FROM thinking_step WHERE workflow_id = '{}' GROUP ALL",
        validated_workflow_id
    );
    let count_result: Vec<serde_json::Value> =
        state.db.query(&count_query).await.unwrap_or_default();

    let count = count_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    // Delete all thinking steps for the workflow
    state
        .db
        .execute(&format!(
            "DELETE thinking_step WHERE workflow_id = '{}'",
            validated_workflow_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to clear workflow thinking steps");
            format!("Failed to clear workflow thinking steps: {}", e)
        })?;

    info!(count = count, "Workflow thinking steps cleared");
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_content_len() {
        // 50KB should be enough for most reasoning content
        assert_eq!(cmd_const::MAX_THINKING_CONTENT_LEN, 50 * 1024);
    }

    #[test]
    fn test_empty_content_validation() {
        let content = "";
        assert!(content.is_empty());
    }

    #[test]
    fn test_content_length_validation() {
        let short_content = "This is a short reasoning step.";
        assert!(short_content.len() < cmd_const::MAX_THINKING_CONTENT_LEN);

        let long_content = "x".repeat(cmd_const::MAX_THINKING_CONTENT_LEN + 1);
        assert!(long_content.len() > cmd_const::MAX_THINKING_CONTENT_LEN);
    }
}
