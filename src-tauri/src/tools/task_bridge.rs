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

//! Task Bridge - Shared helpers for TodoTool <-> Sub-Agent delegation.
//!
//! Provides `extract_task_ids()` and `resolve_and_reassign_tasks()` used by
//! DelegateTaskTool and ParallelTasksTool to bridge todo tasks with sub-agent execution.

use crate::db::DBClient;
use crate::tools::constants::todo::TASK_SELECT_FIELDS;
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use tracing::debug;

/// Extracts task_ids from tool input JSON.
///
/// Returns `None` if the field is absent or the array is empty.
///
/// # Arguments
/// * `input` - JSON input containing an optional `task_ids` array
pub fn extract_task_ids(input: &Value) -> Option<Vec<String>> {
    input["task_ids"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<String>>()
        })
        .filter(|v| !v.is_empty())
}

/// Resolves task_ids from DB and reassigns them to the target agent.
///
/// 1. Reads tasks matching the provided IDs within the workflow scope
/// 2. Updates their `agent_assigned` and `status` to the target agent
/// 3. Returns task summaries for injection into `Task.context.assigned_tasks`
///
/// # Arguments
/// * `db` - Database client
/// * `task_ids` - Task IDs to resolve
/// * `workflow_id` - Workflow scope (prevents cross-workflow access)
/// * `target_agent_id` - Agent ID to assign tasks to
///
/// # Errors
/// * `ToolError::NotFound` if no tasks match the provided IDs
/// * `ToolError::DatabaseError` on query failures
pub async fn resolve_and_reassign_tasks(
    db: &DBClient,
    task_ids: &[String],
    workflow_id: &str,
    target_agent_id: &str,
) -> ToolResult<Vec<Value>> {
    // 1. Read tasks from DB (scoped by workflow_id for security)
    let select_params = vec![
        ("wf_id".to_string(), json!(workflow_id)),
        ("task_ids".to_string(), json!(task_ids)),
    ];

    let tasks: Vec<Value> = db
        .query_with_params(
            &format!(
                "SELECT {} FROM task WHERE workflow_id = $wf_id AND meta::id(id) IN $task_ids",
                TASK_SELECT_FIELDS
            ),
            select_params,
        )
        .await
        .map_err(|e| ToolError::DatabaseError(format!("Failed to resolve tasks: {}", e)))?;

    if tasks.is_empty() {
        return Err(ToolError::NotFound(
            "No tasks found matching the provided task_ids".to_string(),
        ));
    }

    // 2. Reassign: UPDATE agent_assigned + status
    let update_params = vec![
        ("wf_id".to_string(), json!(workflow_id)),
        ("task_ids".to_string(), json!(task_ids)),
        ("agent_id".to_string(), json!(target_agent_id)),
        ("status".to_string(), json!("in_progress")),
    ];

    db.execute_with_params(
        "UPDATE task SET agent_assigned = $agent_id, status = $status \
         WHERE workflow_id = $wf_id AND meta::id(id) IN $task_ids",
        update_params,
    )
    .await
    .map_err(|e| ToolError::DatabaseError(format!("Failed to reassign tasks: {}", e)))?;

    debug!(
        task_count = tasks.len(),
        target_agent = %target_agent_id,
        "Tasks resolved and reassigned"
    );

    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_task_ids_none() {
        let input = json!({"operation": "delegate", "prompt": "do stuff"});
        assert!(extract_task_ids(&input).is_none());
    }

    #[test]
    fn test_extract_task_ids_empty_array() {
        let input = json!({"task_ids": []});
        assert!(extract_task_ids(&input).is_none());
    }

    #[test]
    fn test_extract_task_ids_valid() {
        let input = json!({"task_ids": ["id1", "id2", "id3"]});
        let result = extract_task_ids(&input).unwrap();
        assert_eq!(result, vec!["id1", "id2", "id3"]);
    }

    #[test]
    fn test_extract_task_ids_filters_non_strings() {
        let input = json!({"task_ids": ["id1", 42, "id2", null]});
        let result = extract_task_ids(&input).unwrap();
        assert_eq!(result, vec!["id1", "id2"]);
    }

    #[test]
    fn test_extract_task_ids_not_array() {
        let input = json!({"task_ids": "single_id"});
        assert!(extract_task_ids(&input).is_none());
    }
}
