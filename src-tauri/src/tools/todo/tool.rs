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
//! This file contains the struct, constructor, and Tool trait implementation.
//! Operation methods are in `operations.rs`.

use crate::db::DBClient;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;
use tracing::debug;

/// Cached tool definition (built once, cloned per call).
static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "TodoTool".to_string(),
    name: "Task Manager".to_string(),
    summary: "Manage workflow tasks for decomposition and progress tracking".to_string(),
    description: ToolDescriptionBuilder::new(
        "Manages workflow tasks for structured decomposition and progress tracking.",
    )
    .use_when(&[
        "Breaking down a complex task into smaller, trackable steps",
        "Tracking the status of a multi-step workflow",
        "You need to report progress to the user",
        "Reviewing tasks assigned to a sub-agent (primary agent only)",
        "Reassigning tasks between agents (primary agent only)",
    ])
    .do_not_use(&[
        "Simple, single-step task (just do it directly)",
        "Tracking conversation state (use MemoryTool instead)",
    ])
    .operations(&[
        ("create", "Create new task (name, optional description, priority 1-5)"),
        ("get", "Get task details by ID"),
        ("update_status", "Change status (pending/in_progress/completed/blocked)"),
        ("list", "List tasks (optional status_filter). Sub-agents only see their own tasks."),
        ("complete", "Mark task as completed with optional duration_ms"),
        ("delete", "Remove a task"),
        ("list_agent_tasks", "List tasks assigned to a specific agent with completion stats (primary only)"),
        ("reassign_tasks", "Reassign tasks to a different agent (primary only)"),
    ])
    .examples(&[
        serde_json::json!({"operation": "create", "name": "Analyze DB schema", "priority": 1}),
        serde_json::json!({"operation": "complete", "task_id": "uuid", "duration_ms": 5000}),
        serde_json::json!({"operation": "reassign_tasks", "task_ids": ["id1"], "agent_name": "Security Agent"}),
    ])
    .build(),
    input_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["create", "get", "update_status", "list", "complete", "delete", "list_agent_tasks", "reassign_tasks"],
                "description": "Operation to perform"
            },
            "name": {
                "type": "string",
                "description": "Task name (for create)"
            },
            "description": {
                "type": "string",
                "description": "Task description (for create)"
            },
            "priority": {
                "type": "integer",
                "minimum": 1,
                "maximum": 5,
                "default": 3,
                "description": "Priority 1-5 (1=critical)"
            },
            "dependencies": {
                "type": "array",
                "items": {"type": "string"},
                "description": "IDs of tasks this depends on"
            },
            "task_id": {
                "type": "string",
                "description": "Task ID (for get/update/complete/delete)"
            },
            "status": {
                "type": "string",
                "enum": ["pending", "in_progress", "completed", "blocked"],
                "description": "New status (for update_status)"
            },
            "status_filter": {
                "type": "string",
                "enum": ["pending", "in_progress", "completed", "blocked"],
                "description": "Filter tasks by status (for list)"
            },
            "duration_ms": {
                "type": "integer",
                "description": "Execution duration in milliseconds (for complete)"
            },
            "agent_id": {
                "type": "string",
                "description": "Target agent ID or name (for list_agent_tasks). Either agent_id or agent_name required."
            },
            "agent_name": {
                "type": "string",
                "description": "Target agent name (for list_agent_tasks/reassign_tasks). Alternative to agent_id/new_agent_id."
            },
            "task_ids": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Task IDs to reassign (for reassign_tasks)"
            },
            "new_agent_id": {
                "type": "string",
                "description": "New agent ID or name for reassignment (for reassign_tasks). Either new_agent_id or agent_name required."
            }
        },
        "required": ["operation"]
    }),
    output_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "success": {"type": "boolean"},
            "task_id": {"type": "string"},
            "task": {"type": "object"},
            "tasks": {"type": "array"},
            "count": {"type": "integer"},
            "message": {"type": "string"}
        }
    }),
    requires_confirmation: false,
}
});

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
pub struct TodoTool {
    /// Database client for persistence
    pub(crate) db: Arc<DBClient>,
    /// Current workflow ID (scope)
    pub(crate) workflow_id: String,
    /// Agent ID using this tool
    pub(crate) agent_id: String,
    /// Tauri app handle for emitting streaming events
    pub(crate) app_handle: Option<AppHandle>,
    /// Whether this is the primary workflow agent.
    /// Primary agents see all workflow tasks; sub-agents only see their own.
    pub(crate) is_primary_agent: bool,
}

impl TodoTool {
    /// Creates a new TodoTool for a specific workflow.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `workflow_id` - Workflow ID to scope tasks to
    /// * `agent_id` - Agent ID using this tool
    /// * `app_handle` - Optional Tauri app handle for emitting events
    /// * `is_primary_agent` - Whether this is the primary workflow agent.
    ///   Primary agents see all tasks; sub-agents only see their own.
    ///
    /// # Example
    /// ```ignore
    /// let tool = TodoTool::new(db.clone(), "wf_001".into(), "db_agent".into(), None, true);
    /// ```
    pub fn new(
        db: Arc<DBClient>,
        workflow_id: String,
        agent_id: String,
        app_handle: Option<AppHandle>,
        is_primary_agent: bool,
    ) -> Self {
        Self {
            db,
            workflow_id,
            agent_id,
            app_handle,
            is_primary_agent,
        }
    }
}

#[async_trait]
impl Tool for TodoTool {
    fn id(&self) -> &str {
        "TodoTool"
    }

    /// Returns the tool definition with LLM-friendly description.
    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

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

            "list_agent_tasks" => {
                let agent_ref = input["agent_id"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        input["agent_name"]
                            .as_str()
                            .filter(|s| !s.trim().is_empty())
                    })
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "Missing 'agent_id' or 'agent_name' for list_agent_tasks".to_string(),
                        )
                    })?;
                let status_filter = input["status_filter"].as_str();
                self.list_agent_tasks(agent_ref, status_filter).await
            }

            "reassign_tasks" => {
                let task_ids: Vec<String> = input["task_ids"]
                    .as_array()
                    .ok_or_else(|| ToolError::InvalidInput("Missing task_ids array".to_string()))?
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                let new_agent_ref = input["new_agent_id"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        input["agent_name"]
                            .as_str()
                            .filter(|s| !s.trim().is_empty())
                    })
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "Missing 'new_agent_id' or 'agent_name' for reassign_tasks".to_string(),
                        )
                    })?;
                self.reassign_tasks(&task_ids, new_agent_ref).await
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
            "list_agent_tasks" => {
                let has_agent_id = input
                    .get("agent_id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty());
                let has_agent_name = input
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty());
                if !has_agent_id && !has_agent_name {
                    return Err(ToolError::InvalidInput(
                        "Missing 'agent_id' or 'agent_name' for list_agent_tasks".to_string(),
                    ));
                }
            }
            "reassign_tasks" => {
                if input.get("task_ids").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'task_ids' for reassign_tasks".to_string(),
                    ));
                }
                let has_new_agent_id = input
                    .get("new_agent_id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty());
                let has_agent_name = input
                    .get("agent_name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.trim().is_empty());
                if !has_new_agent_id && !has_agent_name {
                    return Err(ToolError::InvalidInput(
                        "Missing 'new_agent_id' or 'agent_name' for reassign_tasks".to_string(),
                    ));
                }
            }
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown operation: {}",
                    operation
                )));
            }
        }

        Ok(())
    }
}
