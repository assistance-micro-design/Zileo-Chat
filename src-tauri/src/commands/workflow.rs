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

use crate::{
    db::queries::{cascade, workflow as wf_queries},
    models::{
        Message, ThinkingStep, ToolExecution, Workflow, WorkflowCreate, WorkflowFullState,
        WorkflowStatus,
    },
    security::{validate_uuid_field, Validator},
    AppState,
};
use std::sync::Arc;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Batch delete result containing the count of deleted workflows
#[derive(Debug, Clone, serde::Serialize)]
pub struct BatchDeleteResult {
    /// Number of workflows successfully deleted
    pub deleted: u64,
    /// IDs of workflows that were skipped (running status)
    pub skipped_running: Vec<String>,
}

/// Creates a new workflow
#[tauri::command]
#[instrument(
    name = "create_workflow",
    skip(state),
    fields(workflow_name = %name, agent_id = %agent_id)
)]
pub async fn create_workflow(
    name: String,
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use uuid::Uuid;

    info!("Creating new workflow");

    // Validate inputs
    let validated_name = Validator::validate_workflow_name(&name).map_err(|e| {
        warn!(error = %e, "Invalid workflow name");
        format!("Invalid workflow name: {}", e)
    })?;

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent ID");
        format!("Invalid agent ID: {}", e)
    })?;

    // Generate unique ID
    let workflow_id = Uuid::new_v4().to_string();

    // Use WorkflowCreate to avoid passing datetime fields
    // The database will set created_at and updated_at via DEFAULT time::now()
    // ID is passed separately using table:id format
    let workflow = WorkflowCreate::new(validated_name, validated_agent_id, WorkflowStatus::Idle);

    let id = state
        .db
        .create("workflow", &workflow_id, workflow)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create workflow");
            format!("Failed to create workflow: {}", e)
        })?;

    info!(workflow_id = %id, "Workflow created successfully");
    Ok(id)
}

/// Loads all workflows
///
/// Uses a query that explicitly converts the record ID to string to avoid
/// SurrealDB SDK serialization issues with the Thing type.
#[tauri::command]
#[instrument(name = "load_workflows", skip(state))]
pub async fn load_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String> {
    info!("Loading workflows");

    // Use centralized query constant
    let query = &*wf_queries::SELECT_LIST;

    let json_results = state.db.query_json(query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflows");
        format!("Failed to load workflows: {}", e)
    })?;

    // Deserialize using serde_json which respects our custom deserializers
    let workflows: Vec<Workflow> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Workflow>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize workflows");
            format!("Failed to deserialize workflows: {}", e)
        })?;

    info!(count = workflows.len(), "Workflows loaded");
    Ok(workflows)
}

/// Renames a workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to rename
/// * `name` - The new workflow name
///
/// # Returns
/// The updated Workflow entity
#[tauri::command]
#[instrument(name = "rename_workflow", skip(state), fields(workflow_id = %workflow_id, new_name = %name))]
pub async fn rename_workflow(
    workflow_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    info!("Renaming workflow");

    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;
    let validated_name = Validator::validate_workflow_name(&name).map_err(|e| {
        warn!(error = %e, "Invalid workflow name");
        format!("Invalid workflow name: {}", e)
    })?;

    let name_json = crate::security::serialize_for_query(&validated_name, "name")?;

    let query = format!(
        "UPDATE workflow:`{}` SET name = {}, updated_at = time::now() RETURN {}",
        validated_id,
        name_json,
        wf_queries::RETURN_FIELDS
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to rename workflow");
        format!("Failed to rename workflow: {}", e)
    })?;

    let workflow: Workflow = json_results
        .into_iter()
        .next()
        .ok_or_else(|| "Workflow not found".to_string())
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| {
                error!(error = %e, "Failed to deserialize renamed workflow");
                format!("Failed to deserialize workflow: {}", e)
            })
        })?;

    info!("Workflow renamed successfully");
    Ok(workflow)
}

/// Deletes a workflow and all related entities (cascade delete).
///
/// Deletes in order:
/// - Tasks (TodoTool)
/// - Messages
/// - Tool executions
/// - Thinking steps
/// - Sub-agent executions
/// - Validation requests
/// - Memories (workflow-scoped)
/// - Workflow itself
#[tauri::command]
#[instrument(name = "delete_workflow", skip(state), fields(workflow_id = %workflow_id))]
pub async fn delete_workflow(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Deleting workflow with cascade");

    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Use centralized cascade delete helper
    // This eliminates 8 Arc clones + 8 ID clones by using a single helper function
    cascade::delete_workflow_related(&state.db, &validated_id).await;

    // Finally delete the workflow itself
    state
        .db
        .delete(&format!("workflow:{}", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete workflow");
            format!("Failed to delete workflow: {}", e)
        })?;

    info!("Workflow and all related entities deleted successfully");
    Ok(())
}

/// Loads complete workflow state for recovery after restart.
///
/// Executes parallel queries using tokio::try_join! for optimal performance:
/// - Workflow metadata
/// - All messages
/// - Tool execution history
/// - Thinking steps
///
/// # Arguments
/// * `workflow_id` - The workflow ID to load full state for
///
/// # Returns
/// Complete WorkflowFullState with all related data
#[tauri::command]
#[instrument(name = "load_workflow_full_state", skip(state), fields(workflow_id = %workflow_id))]
pub async fn load_workflow_full_state(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowFullState, String> {
    use crate::constants::workflow as wf_const;
    use tokio::time::{timeout, Duration};

    info!("Loading complete workflow state for recovery");

    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Build query strings for all 4 parallel queries
    let wf_query = format!(
        "{} WHERE meta::id(id) = '{}'",
        &*wf_queries::SELECT_BASE,
        validated_id
    );
    let msg_query = format!(
        "SELECT meta::id(id) AS id, workflow_id, role, content, tokens, tokens_input, tokens_output, model, provider, cost_usd, duration_ms, timestamp FROM message WHERE workflow_id = '{}' ORDER BY timestamp ASC",
        validated_id
    );
    let tool_query = format!(
        "SELECT meta::id(id) AS id, workflow_id, message_id, agent_id, tool_type, tool_name, server_name, input_params, output_result, success, error_message, duration_ms, iteration, created_at FROM tool_execution WHERE workflow_id = '{}' ORDER BY created_at ASC",
        validated_id
    );
    let think_query = format!(
        "SELECT meta::id(id) AS id, workflow_id, message_id, agent_id, step_number, content, duration_ms, tokens, created_at FROM thinking_step WHERE workflow_id = '{}' ORDER BY created_at ASC, step_number ASC",
        validated_id
    );

    // Clone db Arc for parallel queries
    let db1 = Arc::clone(&state.db);
    let db2 = Arc::clone(&state.db);
    let db3 = Arc::clone(&state.db);
    let db4 = Arc::clone(&state.db);

    // Execute all queries in parallel using tokio::try_join! (with timeout)
    let parallel_queries = async {
        tokio::try_join!(
            async move {
                let wfs: Vec<Workflow> = query_and_deserialize(&db1, &wf_query, "workflow").await?;
                wfs.into_iter()
                    .next()
                    .ok_or_else(|| "Workflow not found".to_string())
            },
            async move { query_and_deserialize::<Message>(&db2, &msg_query, "messages").await },
            async move {
                query_and_deserialize::<ToolExecution>(&db3, &tool_query, "tool executions").await
            },
            async move {
                query_and_deserialize::<ThinkingStep>(&db4, &think_query, "thinking steps").await
            },
        )
    };

    let (workflow_result, messages_result, tools_result, thinking_result) = timeout(
        Duration::from_secs(wf_const::FULL_STATE_LOAD_TIMEOUT_SECS),
        parallel_queries,
    )
    .await
    .map_err(|_| {
        error!(
            workflow_id = %validated_id,
            timeout_secs = wf_const::FULL_STATE_LOAD_TIMEOUT_SECS,
            "Full state load timed out"
        );
        format!(
            "Full state load timed out after {} seconds",
            wf_const::FULL_STATE_LOAD_TIMEOUT_SECS
        )
    })??;

    let full_state = WorkflowFullState {
        workflow: workflow_result,
        messages: messages_result,
        tool_executions: tools_result,
        thinking_steps: thinking_result,
    };

    info!(
        messages = full_state.messages.len(),
        tools = full_state.tool_executions.len(),
        thinking = full_state.thinking_steps.len(),
        "Workflow full state loaded successfully"
    );

    Ok(full_state)
}

/// Deletes multiple workflows in a single batch operation.
///
/// Validates all IDs, rejects workflows with 'running' status,
/// and performs cascade delete for each valid workflow in sequence.
///
/// # Arguments
/// * `workflow_ids` - List of workflow IDs to delete
///
/// # Returns
/// BatchDeleteResult with count of deleted and list of skipped running IDs
///
/// # Errors
/// Returns error if all IDs are invalid or if a database error occurs
#[tauri::command]
#[instrument(name = "delete_workflows_batch", skip(state), fields(count = workflow_ids.len()))]
pub async fn delete_workflows_batch(
    workflow_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<BatchDeleteResult, String> {
    info!(count = workflow_ids.len(), "Batch deleting workflows");

    if workflow_ids.is_empty() {
        return Ok(BatchDeleteResult {
            deleted: 0,
            skipped_running: vec![],
        });
    }

    // Validate all UUIDs
    let mut validated_ids = Vec::with_capacity(workflow_ids.len());
    for id in &workflow_ids {
        let validated = validate_uuid_field(id, "workflow_id")?;
        validated_ids.push(validated);
    }

    // Check status for all workflows in a single query - reject running ones
    let mut to_delete = Vec::new();
    let mut skipped_running = Vec::new();

    let record_ids: Vec<String> = validated_ids
        .iter()
        .map(|id| format!("workflow:`{}`", id))
        .collect();
    let in_clause = record_ids.join(", ");
    let query = format!(
        "SELECT meta::id(id) AS id, status FROM workflow WHERE id IN [{}]",
        in_clause
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to check workflow statuses for batch delete");
        format!("Failed to check workflow statuses: {}", e)
    })?;

    // Build a set of found IDs and their statuses
    for row in json_results {
        let id = row
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let status_str = row
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        if status_str == "running" {
            warn!(workflow_id = %id, "Skipping running workflow in batch delete");
            skipped_running.push(id);
        } else {
            to_delete.push(id);
        }
    }
    // Workflows not found in results are silently skipped (already deleted)

    // Cascade delete each valid workflow
    let mut deleted: u64 = 0;
    for id in &to_delete {
        cascade::delete_workflow_related(&state.db, id).await;

        state
            .db
            .delete(&format!("workflow:{}", id))
            .await
            .map_err(|e| {
                error!(error = %e, workflow_id = %id, "Failed to delete workflow in batch");
                format!("Failed to delete workflow {}: {}", id, e)
            })?;

        deleted += 1;
    }

    info!(
        deleted = deleted,
        skipped_running = skipped_running.len(),
        "Batch delete completed"
    );

    Ok(BatchDeleteResult {
        deleted,
        skipped_running,
    })
}

/// Moves a single workflow to a folder (or removes from folder).
///
/// # Arguments
/// * `workflow_id` - The workflow ID to move
/// * `folder_id` - Target folder ID, or None to remove from folder
///
/// # Returns
/// The updated Workflow entity
#[tauri::command]
#[instrument(name = "move_workflow_to_folder", skip(state), fields(workflow_id = %workflow_id))]
pub async fn move_workflow_to_folder(
    workflow_id: String,
    folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    info!("Moving workflow to folder");

    let validated_wf_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    let folder_clause = match &folder_id {
        Some(fid) => {
            let validated_fid = validate_uuid_field(fid, "folder_id")?;
            let fid_json = crate::security::serialize_for_query(&validated_fid, "folder_id")?;
            format!("folder_id = {}", fid_json)
        }
        None => "folder_id = NONE".to_string(),
    };

    let query = format!(
        "UPDATE workflow:`{}` SET {}, updated_at = time::now() RETURN {}",
        validated_wf_id,
        folder_clause,
        wf_queries::RETURN_FIELDS
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to move workflow to folder");
        format!("Failed to move workflow to folder: {}", e)
    })?;

    let workflow: Workflow = json_results
        .into_iter()
        .next()
        .ok_or_else(|| "Workflow not found".to_string())
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| format!("Failed to deserialize workflow: {}", e))
        })?;

    info!(folder_id = ?folder_id, "Workflow moved to folder");
    Ok(workflow)
}

/// Moves multiple workflows to a folder (or removes from folder).
///
/// # Arguments
/// * `workflow_ids` - List of workflow IDs to move
/// * `folder_id` - Target folder ID, or None to remove from folder
///
/// # Returns
/// Number of workflows moved
#[tauri::command]
#[instrument(name = "move_workflows_to_folder", skip(state), fields(count = workflow_ids.len()))]
pub async fn move_workflows_to_folder(
    workflow_ids: Vec<String>,
    folder_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    info!(
        count = workflow_ids.len(),
        "Batch moving workflows to folder"
    );

    if workflow_ids.is_empty() {
        return Ok(0);
    }

    // Validate all IDs
    let mut validated_ids = Vec::with_capacity(workflow_ids.len());
    for id in &workflow_ids {
        validated_ids.push(validate_uuid_field(id, "workflow_id")?);
    }

    let folder_clause = match &folder_id {
        Some(fid) => {
            let validated_fid = validate_uuid_field(fid, "folder_id")?;
            let fid_json = crate::security::serialize_for_query(&validated_fid, "folder_id")?;
            format!("folder_id = {}", fid_json)
        }
        None => "folder_id = NONE".to_string(),
    };

    let record_ids: Vec<String> = validated_ids
        .iter()
        .map(|id| format!("workflow:`{}`", id))
        .collect();
    let in_clause = record_ids.join(", ");
    let query = format!(
        "UPDATE workflow SET {}, updated_at = time::now() WHERE id IN [{}]",
        folder_clause, in_clause
    );

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to batch move workflows to folder");
        format!("Failed to batch move workflows: {}", e)
    })?;

    let moved = validated_ids.len() as u64;
    info!(moved = moved, "Workflows moved to folder");
    Ok(moved)
}

/// Toggles the pinned state of a workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to toggle
///
/// # Returns
/// The updated Workflow entity
#[tauri::command]
#[instrument(name = "toggle_workflow_pinned", skip(state), fields(workflow_id = %workflow_id))]
pub async fn toggle_workflow_pinned(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Workflow, String> {
    info!("Toggling workflow pinned state");

    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    let query = format!(
        "UPDATE workflow:`{}` SET pinned = !pinned, updated_at = time::now() RETURN {}",
        validated_id,
        wf_queries::RETURN_FIELDS
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to toggle workflow pinned");
        format!("Failed to toggle workflow pinned: {}", e)
    })?;

    let workflow: Workflow = json_results
        .into_iter()
        .next()
        .ok_or_else(|| "Workflow not found".to_string())
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| format!("Failed to deserialize workflow: {}", e))
        })?;

    info!(pinned = workflow.pinned, "Workflow pinned state toggled");
    Ok(workflow)
}

/// Executes a query_json call and deserializes the results into a typed Vec.
///
/// Shared between execute_workflow, load_workflow_full_state, and similar commands
/// to eliminate repeated query-then-deserialize boilerplate.
async fn query_and_deserialize<T: serde::de::DeserializeOwned>(
    db: &crate::db::DBClient,
    query: &str,
    entity_label: &str,
) -> Result<Vec<T>, String> {
    let json_results = db.query_json(query).await.map_err(|e| {
        error!(error = %e, "Failed to load {}", entity_label);
        format!("Failed to load {}: {}", entity_label, e)
    })?;

    json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<T>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize {}", entity_label);
            format!("Failed to deserialize {}: {}", entity_label, e)
        })
}

#[cfg(test)]
#[path = "workflow_tests.rs"]
mod tests;
