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

//! Sub-agent execution commands for persistence.
//!
//! Provides Tauri commands for retrieving sub-agent execution logs
//! for workflow state recovery and activity display.

use crate::{models::sub_agent::SubAgentExecution, security::Validator, AppState};
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Loads all sub-agent executions for a workflow, sorted by creation time (oldest first).
///
/// # Arguments
/// * `workflow_id` - The workflow ID to load executions for
///
/// # Returns
/// Vector of sub-agent executions in chronological order
#[tauri::command]
#[instrument(name = "load_workflow_sub_agent_executions", skip(state), fields(workflow_id = %workflow_id))]
pub async fn load_workflow_sub_agent_executions(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<SubAgentExecution>, String> {
    info!("Loading workflow sub-agent executions");

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
            created_at,
            completed_at
        FROM sub_agent_execution
        WHERE workflow_id = '{}'
        ORDER BY created_at ASC"#,
        validated_workflow_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflow sub-agent executions");
        format!("Failed to load workflow sub-agent executions: {}", e)
    })?;

    // Deserialize using serde_json
    let executions: Vec<SubAgentExecution> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<SubAgentExecution>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize sub-agent executions");
            format!("Failed to deserialize sub-agent executions: {}", e)
        })?;

    info!(
        count = executions.len(),
        "Workflow sub-agent executions loaded"
    );
    Ok(executions)
}

/// Deletes all sub-agent executions for a workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to clear executions for
///
/// # Returns
/// Number of executions deleted
#[tauri::command]
#[instrument(name = "clear_workflow_sub_agent_executions", skip(state), fields(workflow_id = %workflow_id))]
pub async fn clear_workflow_sub_agent_executions(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    info!("Clearing workflow sub-agent executions");

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    // First count existing executions
    let count_query = format!(
        "SELECT count() FROM sub_agent_execution WHERE workflow_id = '{}' GROUP ALL",
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
            "DELETE sub_agent_execution WHERE workflow_id = '{}'",
            validated_workflow_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to clear workflow sub-agent executions");
            format!("Failed to clear workflow sub-agent executions: {}", e)
        })?;

    info!(count = count, "Workflow sub-agent executions cleared");
    Ok(count)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_module_compiles() {
        // Basic compile test
        assert!(true);
    }
}
