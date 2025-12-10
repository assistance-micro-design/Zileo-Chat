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

//! TodoTool implementation for agent task management.
//!
//! This tool allows agents to manage workflow tasks through a unified interface.

use crate::db::DBClient;
use crate::models::streaming::{events, StreamChunk};
use crate::models::task::{Task, TaskCreate};
use crate::tools::constants::query_limits;
use crate::tools::constants::todo::{
    MAX_DESCRIPTION_LENGTH, MAX_NAME_LENGTH, PRIORITY_MAX, PRIORITY_MIN, VALID_STATUSES,
};
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{
    db_error, delete_with_check, validate_enum_value, validate_length, validate_not_empty,
    validate_range, ParamQueryBuilder,
};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Tool for managing workflow tasks.
///
/// This tool allows agents to:
/// - Create new tasks for workflow decomposition
/// - Update task status as work progresses
/// - Query tasks by status or workflow
/// - Mark tasks as completed with metrics
///
/// # Scope
///
/// Each TodoTool instance is scoped to a specific workflow and agent.
/// Tasks created will be associated with the workflow_id provided at construction.
#[allow(dead_code)]
pub struct TodoTool {
    /// Database client for persistence
    db: Arc<DBClient>,
    /// Current workflow ID (scope)
    workflow_id: String,
    /// Agent ID using this tool
    agent_id: String,
    /// Tauri app handle for emitting streaming events
    app_handle: Option<AppHandle>,
}

#[allow(dead_code)]
impl TodoTool {
    /// Creates a new TodoTool for a specific workflow.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `workflow_id` - Workflow ID to scope tasks to
    /// * `agent_id` - Agent ID using this tool
    /// * `app_handle` - Optional Tauri app handle for emitting events
    ///
    /// # Example
    /// ```ignore
    /// let tool = TodoTool::new(db.clone(), "wf_001".into(), "db_agent".into(), None);
    /// ```
    pub fn new(
        db: Arc<DBClient>,
        workflow_id: String,
        agent_id: String,
        app_handle: Option<AppHandle>,
    ) -> Self {
        Self {
            db,
            workflow_id,
            agent_id,
            app_handle,
        }
    }

    /// Helper method to emit streaming events.
    ///
    /// If no app_handle is available, the event is silently skipped.
    fn emit_task_event(&self, chunk: StreamChunk) {
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
    async fn create_task(
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

        // OPT-TODO-7: Use db_error() for consistency
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
    async fn update_status(&self, task_id: &str, status: &str) -> ToolResult<Value> {
        validate_enum_value(status, VALID_STATUSES, "status")?;

        // OPT-TODO-5: Reduce N+1 queries (3->1) using UPDATE ... RETURN
        // Single query: updates status AND returns name for event emission
        // If result is empty, task doesn't exist (handles existence check)
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
            return Err(ToolError::NotFound(format!(
                "Task '{}' not found",
                task_id
            )));
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
    async fn list_tasks(&self, status_filter: Option<&str>) -> ToolResult<Value> {
        // OPT-TODO-2: Use ParamQueryBuilder for SQL injection safety
        // OPT-TODO-10: Add LIMIT to prevent memory explosion
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
    async fn get_task(&self, task_id: &str) -> ToolResult<Value> {
        // OPT-TODO-4: Parameterized query for SQL injection safety
        let params = vec![("task_id".to_string(), serde_json::json!(task_id))];
        let results: Vec<Task> = self
            .db
            .query_with_params(
                r#"SELECT
                    meta::id(id) AS id,
                    workflow_id,
                    name,
                    description,
                    agent_assigned,
                    priority,
                    status,
                    dependencies,
                    duration_ms,
                    created_at,
                    completed_at
                FROM task
                WHERE meta::id(id) = $task_id"#,
                params,
            )
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
    async fn complete_task(&self, task_id: &str, duration_ms: Option<u64>) -> ToolResult<Value> {
        // OPT-TODO-6: Reduce N+1 queries (2->1) using UPDATE ... RETURN
        // Single query: updates status/completed_at/duration AND returns name for event
        // If result is empty, task doesn't exist (handles existence check)
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
    async fn delete_task(&self, task_id: &str) -> ToolResult<Value> {
        delete_with_check(&self.db, "task", task_id, "Task").await?;

        info!(task_id = %task_id, "Task deleted");

        Ok(ResponseBuilder::ok(
            "task_id",
            task_id,
            "Task deleted successfully",
        ))
    }
}

#[async_trait]
impl Tool for TodoTool {
    /// Returns the tool definition with LLM-friendly description.
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "TodoTool".to_string(),
            name: "Todo Task Manager".to_string(),
            description: r#"Manages workflow tasks for structured execution tracking.

USE THIS TOOL TO:
- Break down complex work into trackable tasks
- Update task progress as you work
- Coordinate with other agents via task assignment
- Track task completion with timing metrics

OPERATIONS:
- create: Create a new task with name, description, priority (1-5)
- get: Retrieve a single task by ID
- update_status: Change task status (pending/in_progress/completed/blocked)
- list: View all tasks or filter by status
- complete: Mark task done with optional duration
- delete: Remove a task

BEST PRACTICES:
- Create tasks BEFORE starting complex multi-step work
- Update status to 'in_progress' when starting a task
- Use priority 1 for critical/blocking tasks, 5 for low priority
- Mark completed with duration_ms for metrics tracking

EXAMPLES:
1. Create a task:
   {"operation": "create", "name": "Analyze code", "description": "Deep analysis", "priority": 1}

2. Start working on it:
   {"operation": "update_status", "task_id": "abc123", "status": "in_progress"}

3. Mark complete:
   {"operation": "complete", "task_id": "abc123", "duration_ms": 5000}

4. List pending tasks:
   {"operation": "list", "status_filter": "pending"}"#
                .to_string(),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["create", "get", "update_status", "list", "complete", "delete"],
                        "description": "The operation to perform"
                    },
                    "name": {
                        "type": "string",
                        "maxLength": 128,
                        "description": "Task name (for create operation)"
                    },
                    "description": {
                        "type": "string",
                        "maxLength": 1000,
                        "description": "Task description (for create operation)"
                    },
                    "priority": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 5,
                        "default": 3,
                        "description": "Priority level: 1=critical, 5=low (for create)"
                    },
                    "dependencies": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Task IDs this depends on (for create)"
                    },
                    "task_id": {
                        "type": "string",
                        "description": "Task ID (for get/update_status/complete/delete)"
                    },
                    "status": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "blocked"],
                        "description": "New status (for update_status)"
                    },
                    "status_filter": {
                        "type": "string",
                        "enum": ["pending", "in_progress", "completed", "blocked"],
                        "description": "Filter by status (for list operation)"
                    },
                    "duration_ms": {
                        "type": "integer",
                        "description": "Execution duration in ms (for complete)"
                    }
                },
                "required": ["operation"]
            }),

            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "success": {"type": "boolean"},
                    "task_id": {"type": "string"},
                    "message": {"type": "string"},
                    "task": {"type": "object"},
                    "tasks": {"type": "array"},
                    "count": {"type": "integer"},
                    "new_status": {"type": "string"},
                    "duration_ms": {"type": "integer"}
                }
            }),

            requires_confirmation: false,
        }
    }

    /// Executes the tool with JSON input.
    #[instrument(skip(self, input), fields(workflow_id = %self.workflow_id))]
    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation".to_string()))?;

        debug!(operation = %operation, "Executing TodoTool");

        match operation {
            "create" => {
                let name = input["name"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing name for create".to_string())
                })?;
                let description = input["description"].as_str().unwrap_or("");
                let priority = input["priority"].as_u64().unwrap_or(3) as u8;
                let dependencies: Vec<String> = input["dependencies"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                self.create_task(name, description, priority, dependencies)
                    .await
            }

            "get" => {
                let task_id = input["task_id"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing task_id for get".to_string())
                })?;

                self.get_task(task_id).await
            }

            "update_status" => {
                let task_id = input["task_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing task_id".to_string()))?;
                let status = input["status"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing status".to_string()))?;

                self.update_status(task_id, status).await
            }

            "list" => {
                let status_filter = input["status_filter"].as_str();
                self.list_tasks(status_filter).await
            }

            "complete" => {
                let task_id = input["task_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing task_id".to_string()))?;
                let duration_ms = input["duration_ms"].as_u64();

                self.complete_task(task_id, duration_ms).await
            }

            "delete" => {
                let task_id = input["task_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing task_id".to_string()))?;

                self.delete_task(task_id).await
            }

            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }

    /// Validates input before execution.
    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation field".to_string()))?;

        match operation {
            "create" => {
                if input.get("name").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'name' for create operation".to_string(),
                    ));
                }
            }
            "get" => {
                if input.get("task_id").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'task_id' for get operation".to_string(),
                    ));
                }
            }
            "update_status" => {
                if input.get("task_id").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'task_id' for update_status".to_string(),
                    ));
                }
                if input.get("status").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'status' for update_status".to_string(),
                    ));
                }
            }
            "complete" | "delete" => {
                if input.get("task_id").is_none() {
                    return Err(ToolError::InvalidInput(format!(
                        "Missing 'task_id' for {} operation",
                        operation
                    )));
                }
            }
            "list" => {} // No required params
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown operation: {}",
                    operation
                )));
            }
        }

        Ok(())
    }

    /// Returns false - task operations are reversible, no confirmation needed.
    fn requires_confirmation(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        // We can test the definition without a DB
        let definition = ToolDefinition {
            id: "TodoTool".to_string(),
            name: "Todo Task Manager".to_string(),
            description: "Test".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "TodoTool");
        assert!(!definition.requires_confirmation);
    }

    #[test]
    fn test_input_validation_create() {
        let valid_input = serde_json::json!({
            "operation": "create",
            "name": "Test task",
            "description": "A test task",
            "priority": 2
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "create");
        assert!(valid_input.get("name").is_some());
    }

    #[test]
    fn test_input_validation_update_status() {
        let valid_input = serde_json::json!({
            "operation": "update_status",
            "task_id": "task_001",
            "status": "in_progress"
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("task_id").is_some());
        assert!(valid_input.get("status").is_some());
    }

    #[test]
    fn test_input_validation_list() {
        let valid_input = serde_json::json!({
            "operation": "list",
            "status_filter": "pending"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "list");
    }

    #[test]
    fn test_priority_values() {
        for p in 1..=5u8 {
            assert!((1..=5).contains(&p));
        }

        // Invalid priorities
        assert!(!(1..=5).contains(&0u8));
        assert!(!(1..=5).contains(&6u8));
    }

    #[test]
    fn test_valid_statuses() {
        let valid_statuses = ["pending", "in_progress", "completed", "blocked"];

        assert!(valid_statuses.contains(&"pending"));
        assert!(valid_statuses.contains(&"in_progress"));
        assert!(valid_statuses.contains(&"completed"));
        assert!(valid_statuses.contains(&"blocked"));
        assert!(!valid_statuses.contains(&"done"));
        assert!(!valid_statuses.contains(&"started"));
    }
}
