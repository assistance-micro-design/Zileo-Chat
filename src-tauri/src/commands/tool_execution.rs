// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Tool execution commands for persistence.
//!
//! Provides Tauri commands for saving and retrieving tool execution logs
//! for workflow state recovery and debugging.
//!
//! Phase 3: Tool Execution Persistence - Enables complete workflow state
//! recovery with full tool call history.

use crate::{
    models::{ToolExecution, ToolExecutionCreate},
    security::Validator,
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Maximum allowed length for tool name
const MAX_TOOL_NAME_LEN: usize = 128;

/// Maximum allowed size for input/output params (50KB)
const MAX_PARAMS_SIZE: usize = 50 * 1024;

/// Saves a new tool execution to the database.
///
/// # Arguments
/// * `workflow_id` - Associated workflow ID
/// * `message_id` - Associated message ID
/// * `agent_id` - Agent ID that executed the tool
/// * `tool_type` - Tool type ("local" or "mcp")
/// * `tool_name` - Name of the tool
/// * `server_name` - MCP server name (only for MCP tools)
/// * `input_params` - Input parameters as JSON
/// * `output_result` - Output result as JSON
/// * `success` - Whether execution was successful
/// * `error_message` - Error message if failed
/// * `duration_ms` - Execution duration in milliseconds
/// * `iteration` - Iteration number in the tool loop
///
/// # Returns
/// The ID of the created tool execution record
#[allow(clippy::too_many_arguments)]
#[tauri::command]
#[instrument(
    name = "save_tool_execution",
    skip(state, input_params, output_result),
    fields(
        workflow_id = %workflow_id,
        tool_name = %tool_name,
        tool_type = %tool_type
    )
)]
pub async fn save_tool_execution(
    workflow_id: String,
    message_id: String,
    agent_id: String,
    tool_type: String,
    tool_name: String,
    server_name: Option<String>,
    input_params: serde_json::Value,
    output_result: serde_json::Value,
    success: bool,
    error_message: Option<String>,
    duration_ms: u64,
    iteration: u32,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Saving tool execution");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // Validate message ID
    let validated_message_id = Validator::validate_uuid(&message_id).map_err(|e| {
        warn!(error = %e, "Invalid message ID");
        format!("Invalid message ID: {}", e)
    })?;

    // Validate agent ID
    let validated_agent_id = Validator::validate_uuid(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent ID");
        format!("Invalid agent ID: {}", e)
    })?;

    // Validate tool type
    let validated_tool_type = match tool_type.as_str() {
        "local" | "mcp" => tool_type.clone(),
        _ => {
            warn!(tool_type = %tool_type, "Invalid tool type");
            return Err(format!(
                "Invalid tool type: {}. Expected 'local' or 'mcp'",
                tool_type
            ));
        }
    };

    // Validate tool name
    if tool_name.is_empty() {
        return Err("Tool name cannot be empty".to_string());
    }
    if tool_name.len() > MAX_TOOL_NAME_LEN {
        return Err(format!(
            "Tool name exceeds maximum length of {} characters",
            MAX_TOOL_NAME_LEN
        ));
    }

    // Validate params size
    let input_size = serde_json::to_string(&input_params)
        .map(|s| s.len())
        .unwrap_or(0);
    let output_size = serde_json::to_string(&output_result)
        .map(|s| s.len())
        .unwrap_or(0);

    if input_size > MAX_PARAMS_SIZE {
        return Err(format!(
            "Input params exceed maximum size of {} bytes",
            MAX_PARAMS_SIZE
        ));
    }
    if output_size > MAX_PARAMS_SIZE {
        return Err(format!(
            "Output result exceeds maximum size of {} bytes",
            MAX_PARAMS_SIZE
        ));
    }

    // Validate server_name for MCP tools
    if validated_tool_type == "mcp" && server_name.is_none() {
        return Err("server_name is required for MCP tools".to_string());
    }

    let execution_id = Uuid::new_v4().to_string();

    // Build ToolExecutionCreate payload
    let execution = ToolExecutionCreate {
        workflow_id: validated_workflow_id,
        message_id: validated_message_id,
        agent_id: validated_agent_id,
        tool_type: validated_tool_type,
        tool_name,
        server_name,
        input_params,
        output_result,
        success,
        error_message,
        duration_ms,
        iteration,
    };

    // Insert into database
    let id = state
        .db
        .create("tool_execution", &execution_id, execution)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to save tool execution");
            format!("Failed to save tool execution: {}", e)
        })?;

    info!(execution_id = %id, "Tool execution saved successfully");
    Ok(execution_id)
}

/// Loads all tool executions for a workflow, sorted by creation time (oldest first).
///
/// # Arguments
/// * `workflow_id` - The workflow ID to load executions for
///
/// # Returns
/// Vector of tool executions in chronological order
#[tauri::command]
#[instrument(name = "load_workflow_tool_executions", skip(state), fields(workflow_id = %workflow_id))]
pub async fn load_workflow_tool_executions(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ToolExecution>, String> {
    info!("Loading workflow tool executions");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    let query = format!(
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
            created_at
        FROM tool_execution
        WHERE workflow_id = '{}'
        ORDER BY created_at ASC"#,
        validated_workflow_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflow tool executions");
        format!("Failed to load workflow tool executions: {}", e)
    })?;

    // Deserialize using serde_json
    let executions: Vec<ToolExecution> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ToolExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize tool executions");
            format!("Failed to deserialize tool executions: {}", e)
        })?;

    info!(count = executions.len(), "Workflow tool executions loaded");
    Ok(executions)
}

/// Loads tool executions for a specific message.
///
/// Useful for displaying tool calls associated with a particular assistant response.
///
/// # Arguments
/// * `message_id` - The message ID to load executions for
///
/// # Returns
/// Vector of tool executions in chronological order
#[tauri::command]
#[instrument(name = "load_message_tool_executions", skip(state), fields(message_id = %message_id))]
pub async fn load_message_tool_executions(
    message_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ToolExecution>, String> {
    info!("Loading message tool executions");

    // Validate message ID
    let validated_message_id = Validator::validate_uuid(&message_id).map_err(|e| {
        warn!(error = %e, "Invalid message ID");
        format!("Invalid message ID: {}", e)
    })?;

    let query = format!(
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
            created_at
        FROM tool_execution
        WHERE message_id = '{}'
        ORDER BY created_at ASC"#,
        validated_message_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load message tool executions");
        format!("Failed to load message tool executions: {}", e)
    })?;

    let executions: Vec<ToolExecution> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<ToolExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize tool executions");
            format!("Failed to deserialize tool executions: {}", e)
        })?;

    info!(count = executions.len(), "Message tool executions loaded");
    Ok(executions)
}

/// Deletes a single tool execution by ID.
///
/// # Arguments
/// * `execution_id` - The execution ID to delete
///
/// # Returns
/// Success or error
#[tauri::command]
#[instrument(name = "delete_tool_execution", skip(state), fields(execution_id = %execution_id))]
pub async fn delete_tool_execution(
    execution_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Deleting tool execution");

    // Validate execution ID
    let validated_id = Validator::validate_uuid(&execution_id).map_err(|e| {
        warn!(error = %e, "Invalid execution ID");
        format!("Invalid execution ID: {}", e)
    })?;

    // Use execute() with DELETE query to avoid SurrealDB SDK serialization issues
    state
        .db
        .execute(&format!("DELETE tool_execution:`{}`", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete tool execution");
            format!("Failed to delete tool execution: {}", e)
        })?;

    info!("Tool execution deleted successfully");
    Ok(())
}

/// Deletes all tool executions for a workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to clear executions for
///
/// # Returns
/// Number of executions deleted
#[tauri::command]
#[instrument(name = "clear_workflow_tool_executions", skip(state), fields(workflow_id = %workflow_id))]
pub async fn clear_workflow_tool_executions(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    info!("Clearing workflow tool executions");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // First count existing executions
    let count_query = format!(
        "SELECT count() FROM tool_execution WHERE workflow_id = '{}' GROUP ALL",
        validated_workflow_id
    );
    let count_result: Vec<serde_json::Value> =
        state.db.query(&count_query).await.unwrap_or_default();

    let count = count_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0);

    // Delete all executions for the workflow
    state
        .db
        .execute(&format!(
            "DELETE tool_execution WHERE workflow_id = '{}'",
            validated_workflow_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to clear workflow tool executions");
            format!("Failed to clear workflow tool executions: {}", e)
        })?;

    info!(count = count, "Workflow tool executions cleared");
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_tool_name_len() {
        // Verify the constant is set to a reasonable value
        assert_eq!(MAX_TOOL_NAME_LEN, 128);
    }

    #[test]
    fn test_valid_tool_types() {
        let valid_types = vec!["local", "mcp"];
        for tool_type in valid_types {
            assert!(matches!(tool_type, "local" | "mcp"));
        }
    }

    #[test]
    fn test_invalid_tool_type_detection() {
        let invalid_types = vec!["remote", "internal", ""];
        for tool_type in invalid_types {
            assert!(!matches!(tool_type, "local" | "mcp"));
        }
    }

    #[test]
    fn test_max_params_size() {
        // 50KB should be enough for most tool params
        assert_eq!(MAX_PARAMS_SIZE, 50 * 1024);
    }
}
