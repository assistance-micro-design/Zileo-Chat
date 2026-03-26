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

//! TodoTool operation methods.
//!
//! Contains create, update_status, list, get, complete, delete operations,
//! and the event emission helper.

use super::tool::TodoTool;
use crate::constants::query_limits;
use crate::models::streaming::{events, StreamChunk};
use crate::models::task::{Task, TaskCreate};
use crate::tools::constants::todo::{
    MAX_DESCRIPTION_LENGTH, MAX_NAME_LENGTH, PRIORITY_MAX, PRIORITY_MIN, TASK_SELECT_FIELDS,
    VALID_STATUSES,
};
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{
    db_error, delete_with_check, validate_enum_value, validate_length, validate_not_empty,
    validate_range, ParamQueryBuilder,
};
use crate::tools::{ToolError, ToolResult};
use serde_json::Value;
use tauri::Emitter;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

impl TodoTool {
    /// Helper method to emit streaming events.
    ///
    /// If no app_handle is available, the event is silently skipped.
    pub(crate) fn emit_task_event(&self, chunk: StreamChunk) {
        if let Some(ref handle) = self.app_handle {
            if let Err(e) = handle.emit(events::WORKFLOW_STREAM, &chunk) {
                warn!(error = %e, "Failed to emit TodoTool event");
            }
        }
    }

    /// Creates a new task.
    ///
    /// # Arguments
    /// * `name` - Task name (max 128 chars)
    /// * `description` - Task description (max 1000 chars)
    /// * `priority` - Priority level 1-5 (1=critical, 5=low)
    /// * `dependencies` - Task IDs this depends on
    #[instrument(skip(self), fields(workflow_id = %self.workflow_id, agent_id = %self.agent_id))]
    pub(crate) async fn create_task(
        &self,
        name: &str,
        description: &str,
        priority: u8,
        dependencies: Vec<String>,
    ) -> ToolResult<Value> {
        // Validate inputs with actionable error messages
        validate_not_empty(name, "name")?;
        validate_length(name, MAX_NAME_LENGTH, "name")?;
        validate_length(description, MAX_DESCRIPTION_LENGTH, "description")?;
        validate_range(priority, PRIORITY_MIN, PRIORITY_MAX, "priority")?;

        let task_id = Uuid::new_v4().to_string();

        let task = TaskCreate::new(
            self.workflow_id.clone(),
            name.to_string(),
            description.to_string(),
            priority,
        )
        .with_agent(self.agent_id.clone())
        .with_dependencies(dependencies);

        self.db
            .create("task", &task_id, task)
            .await
            .map_err(db_error)?;

        info!(task_id = %task_id, name = %name, "Task created");

        // Emit task creation event
        self.emit_task_event(StreamChunk::task_create(
            &self.workflow_id,
            &task_id,
            name,
            priority,
            Some(self.agent_id.clone()),
        ));

        Ok(ResponseBuilder::ok(
            "task_id",
            task_id,
            "Task created successfully",
        ))
    }

    /// Updates task status.
    ///
    /// # Arguments
    /// * `task_id` - Task ID to update
    /// * `status` - New status (pending/in_progress/completed/blocked)
    #[instrument(skip(self))]
    pub(crate) async fn update_status(&self, task_id: &str, status: &str) -> ToolResult<Value> {
        validate_enum_value(status, VALID_STATUSES, "status")?;

        let params = vec![
            ("task_id".to_string(), serde_json::json!(task_id)),
            ("status".to_string(), serde_json::json!(status)),
        ];
        let result: Vec<Value> = self
            .db
            .query_with_params(
                "UPDATE task SET status = $status WHERE meta::id(id) = $task_id RETURN name",
                params,
            )
            .await
            .map_err(db_error)?;

        if result.is_empty() {
            return Err(ToolError::NotFound(format!("Task '{}' not found", task_id)));
        }

        let task_name = result
            .first()
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Task");

        info!(task_id = %task_id, status = %status, "Task status updated");

        // Emit task update event
        self.emit_task_event(StreamChunk::task_update(
            &self.workflow_id,
            task_id,
            task_name,
            status,
        ));

        Ok(ResponseBuilder::new()
            .success(true)
            .id("task_id", task_id)
            .field("new_status", status)
            .message(format!("Task status updated to '{}'", status))
            .build())
    }

    /// Lists tasks for current workflow.
    ///
    /// # Arguments
    /// * `status_filter` - Optional status to filter by
    #[instrument(skip(self))]
    pub(crate) async fn list_tasks(&self, status_filter: Option<&str>) -> ToolResult<Value> {
        let mut builder = ParamQueryBuilder::new("task")
            .select(&[
                "name",
                "description",
                "status",
                "priority",
                "agent_assigned",
                "created_at",
            ])
            .where_eq_param(
                "workflow_id",
                "wf_id",
                serde_json::json!(self.workflow_id.clone()),
            );

        if let Some(status) = status_filter {
            builder = builder.where_eq_param("status", "status_filter", serde_json::json!(status));
        }

        let (query, params) = builder
            .order_by("priority", false) // ASC
            .limit(query_limits::DEFAULT_LIST_LIMIT)
            .build();

        let tasks: Vec<Value> = self
            .db
            .query_with_params(&query, params)
            .await
            .map_err(db_error)?;

        debug!(count = tasks.len(), "Tasks listed");

        Ok(ResponseBuilder::new()
            .success(true)
            .field("workflow_id", self.workflow_id.clone())
            .count(tasks.len())
            .data("tasks", tasks)
            .build())
    }

    /// Gets a single task by ID.
    ///
    /// # Arguments
    /// * `task_id` - Task ID to retrieve
    #[instrument(skip(self))]
    pub(crate) async fn get_task(&self, task_id: &str) -> ToolResult<Value> {
        let params = vec![("task_id".to_string(), serde_json::json!(task_id))];
        let query = format!(
            "SELECT {} FROM task WHERE meta::id(id) = $task_id",
            TASK_SELECT_FIELDS
        );
        let results: Vec<Task> = self
            .db
            .query_with_params(&query, params)
            .await
            .map_err(db_error)?;

        match results.into_iter().next() {
            Some(task) => Ok(serde_json::json!({
                "success": true,
                "task": task
            })),
            None => Err(ToolError::NotFound(format!(
                "Task '{}' does not exist in workflow '{}'",
                task_id, self.workflow_id
            ))),
        }
    }

    /// Marks task as completed with optional duration.
    ///
    /// # Arguments
    /// * `task_id` - Task ID to complete
    /// * `duration_ms` - Optional execution duration in milliseconds
    #[instrument(skip(self))]
    pub(crate) async fn complete_task(&self, task_id: &str, duration_ms: Option<u64>) -> ToolResult<Value> {
        let (update_query, update_params) = match duration_ms {
            Some(duration) => (
                "UPDATE task SET status = $status, completed_at = time::now(), duration_ms = $duration WHERE meta::id(id) = $task_id RETURN name".to_string(),
                vec![
                    ("task_id".to_string(), serde_json::json!(task_id)),
                    ("status".to_string(), serde_json::json!("completed")),
                    ("duration".to_string(), serde_json::json!(duration)),
                ],
            ),
            None => (
                "UPDATE task SET status = $status, completed_at = time::now() WHERE meta::id(id) = $task_id RETURN name".to_string(),
                vec![
                    ("task_id".to_string(), serde_json::json!(task_id)),
                    ("status".to_string(), serde_json::json!("completed")),
                ],
            ),
        };

        let result: Vec<Value> = self
            .db
            .query_with_params(&update_query, update_params)
            .await
            .map_err(db_error)?;

        if result.is_empty() {
            return Err(ToolError::NotFound(format!(
                "Task '{}' not found. Cannot mark as completed",
                task_id
            )));
        }

        let task_name = result
            .first()
            .and_then(|v| v.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown Task");

        info!(task_id = %task_id, duration_ms = ?duration_ms, "Task completed");

        // Emit task completion event
        self.emit_task_event(StreamChunk::task_complete(
            &self.workflow_id,
            task_id,
            task_name,
            duration_ms,
        ));

        Ok(serde_json::json!({
            "success": true,
            "task_id": task_id,
            "status": "completed",
            "duration_ms": duration_ms,
            "message": format!("Task '{}' marked as completed", task_id)
        }))
    }

    /// Deletes a task.
    ///
    /// # Arguments
    /// * `task_id` - Task ID to delete
    #[instrument(skip(self))]
    pub(crate) async fn delete_task(&self, task_id: &str) -> ToolResult<Value> {
        delete_with_check(&self.db, "task", task_id, "Task").await?;

        info!(task_id = %task_id, "Task deleted");

        Ok(ResponseBuilder::ok(
            "task_id",
            task_id,
            "Task deleted successfully",
        ))
    }
}
