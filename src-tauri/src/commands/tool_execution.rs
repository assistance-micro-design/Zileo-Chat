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

//! Tool execution commands for persistence.
//!
//! Provides Tauri commands for saving and retrieving tool execution logs
//! for workflow state recovery and debugging.
//!
//! Enables complete workflow state recovery with full tool call history.

use crate::{
    constants::commands as cmd_const,
    db::extract_count,
    models::{ToolExecution, ToolExecutionCreate},
    security::validate_uuid_field,
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

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
/// Validates tool-specific fields (type, name, params size, server_name).
fn validate_tool_fields(
    tool_type: &str,
    tool_name: &str,
    server_name: &Option<String>,
    input_params: &serde_json::Value,
    output_result: &serde_json::Value,
) -> Result<(), String> {
    // Validate tool type
    match tool_type {
        "local" | "mcp" => {}
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
    if tool_name.len() > cmd_const::MAX_TOOL_NAME_LEN {
        return Err(format!(
            "Tool name exceeds maximum length of {} characters",
            cmd_const::MAX_TOOL_NAME_LEN
        ));
    }

    // Validate params size
    let input_size = serde_json::to_string(input_params)
        .map(|s| s.len())
        .unwrap_or(0);
    let output_size = serde_json::to_string(output_result)
        .map(|s| s.len())
        .unwrap_or(0);

    if input_size > cmd_const::MAX_PARAMS_SIZE {
        return Err(format!(
            "Input params exceed maximum size of {} bytes",
            cmd_const::MAX_PARAMS_SIZE
        ));
    }
    if output_size > cmd_const::MAX_PARAMS_SIZE {
        return Err(format!(
            "Output result exceeds maximum size of {} bytes",
            cmd_const::MAX_PARAMS_SIZE
        ));
    }

    // Validate server_name for MCP tools
    if tool_type == "mcp" && server_name.is_none() {
        return Err("server_name is required for MCP tools".to_string());
    }

    Ok(())
}

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

    validate_uuid_field(&workflow_id, "workflow_id")?;
    validate_uuid_field(&message_id, "message_id")?;
    validate_uuid_field(&agent_id, "agent_id")?;
    validate_tool_fields(
        &tool_type,
        &tool_name,
        &server_name,
        &input_params,
        &output_result,
    )?;

    let execution_id = Uuid::new_v4().to_string();

    let execution = ToolExecutionCreate {
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
        sequence: 0,
    };

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

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

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
            sequence,
            created_at
        FROM tool_execution
        WHERE workflow_id = '{}'
        ORDER BY sequence ASC, created_at ASC"#,
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

    let validated_message_id = validate_uuid_field(&message_id, "message_id")?;

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
            sequence,
            created_at
        FROM tool_execution
        WHERE message_id = '{}'
        ORDER BY sequence ASC, created_at ASC"#,
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

/// Loads a single tool execution by ID.
///
/// # Arguments
/// * `execution_id` - The tool execution UUID
///
/// # Returns
/// The full ToolExecution record including input_params and output_result
#[tauri::command]
#[instrument(name = "get_tool_execution", skip(state), fields(execution_id = %execution_id))]
pub async fn get_tool_execution(
    execution_id: String,
    state: State<'_, AppState>,
) -> Result<ToolExecution, String> {
    info!("Loading tool execution by ID");

    let validated_id = validate_uuid_field(&execution_id, "execution_id")?;

    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id, message_id, agent_id, tool_type, tool_name,
            server_name, input_params, output_result, success,
            error_message, duration_ms, iteration, sequence, created_at
        FROM tool_execution
        WHERE meta::id(id) = '{}'"#,
        validated_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to get tool execution");
        format!("Failed to get tool execution: {}", e)
    })?;

    let execution: Option<ToolExecution> = json_results
        .into_iter()
        .next()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize tool execution");
            format!("Failed to deserialize tool execution: {}", e)
        })?;

    execution.ok_or_else(|| format!("Tool execution not found: {}", execution_id))
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

    let validated_id = validate_uuid_field(&execution_id, "execution_id")?;

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

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // First count existing executions
    let count_query = format!(
        "SELECT count() FROM tool_execution WHERE workflow_id = '{}' GROUP ALL",
        validated_workflow_id
    );
    let count_result: Vec<serde_json::Value> = state.db.query(&count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count workflow tool executions");
        format!("Failed to count workflow tool executions: {}", e)
    })?;

    let count = extract_count(&count_result);

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
