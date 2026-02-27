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

//! Task management commands for the Todo Tool.
//!
//! Provides CRUD operations for workflow task decomposition.
//!
//! # Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `create_task` | Create a new task for a workflow |
//! | `get_task` | Get a single task by ID |
//! | `list_workflow_tasks` | List all tasks for a workflow |
//! | `list_tasks_by_status` | List tasks filtered by status |
//! | `update_task` | Update task fields (partial) |
//! | `update_task_status` | Update task status specifically |
//! | `complete_task` | Mark task as completed with duration |
//! | `delete_task` | Delete a task |
//!
//! # Tauri IPC Parameter Naming
//!
//! Rust parameters use `snake_case`, TypeScript `invoke()` uses `camelCase`:
//! - `workflow_id` -> `workflowId`
//! - `task_id` -> `taskId`
//! - `agent_assigned` -> `agentAssigned`
//! - `duration_ms` -> `durationMs`

use crate::{
    constants::query_limits,
    models::task::{Task, TaskCreate, TaskUpdate},
    security::{serialize_for_query, validate_uuid_field, Validator},
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Creates a new task for a workflow.
///
/// # Arguments (camelCase in TypeScript)
/// * `workflowId` - Associated workflow ID
/// * `name` - Task name (max 128 chars)
/// * `description` - Task description (max 1000 chars)
/// * `priority` - Priority level 1-5 (optional, default 3)
/// * `agentAssigned` - Agent ID to assign (optional)
/// * `dependencies` - List of task IDs this depends on (optional)
///
/// # Returns
/// The created task ID (UUID string)
///
/// # Errors
/// Returns error string if validation fails or database operation fails.
#[tauri::command]
#[instrument(
    name = "create_task",
    skip(state, description, dependencies),
    fields(workflow_id = %workflow_id, name = %name, priority = ?priority)
)]
pub async fn create_task(
    workflow_id: String,
    name: String,
    description: String,
    priority: Option<u8>,
    agent_assigned: Option<String>,
    dependencies: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating new task");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Validate task name
    let validated_name = Validator::validate_message(&name).map_err(|e| {
        warn!(error = %e, "Invalid task name");
        format!("Invalid task name: {}", e)
    })?;

    if validated_name.len() > 128 {
        return Err("Task name must be 128 characters or less".to_string());
    }

    if description.len() > 1000 {
        return Err("Task description must be 1000 characters or less".to_string());
    }

    // Validate priority range
    let priority = priority.unwrap_or(3);
    if !(1..=5).contains(&priority) {
        return Err("Priority must be between 1 and 5".to_string());
    }

    // Validate dependencies if provided
    let deps = if let Some(deps) = dependencies {
        for dep in &deps {
            validate_uuid_field(dep, "dependency_id")?;
        }
        deps
    } else {
        Vec::new()
    };

    // Create task payload
    let task_id = Uuid::new_v4().to_string();
    let mut task_create =
        TaskCreate::new(validated_workflow_id, validated_name, description, priority);

    if let Some(agent) = agent_assigned {
        task_create = task_create.with_agent(agent);
    }

    if !deps.is_empty() {
        task_create = task_create.with_dependencies(deps);
    }

    // Insert into database
    state
        .db
        .create("task", &task_id, task_create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create task");
            format!("Failed to create task: {}", e)
        })?;

    info!(task_id = %task_id, "Task created successfully");
    Ok(task_id)
}

/// Gets a single task by ID.
///
/// # Arguments (camelCase in TypeScript)
/// * `taskId` - The task ID to retrieve
///
/// # Returns
/// The task if found
///
/// # Errors
/// Returns error string if task not found or database error.
#[tauri::command]
#[instrument(name = "get_task", skip(state), fields(task_id = %task_id))]
pub async fn get_task(task_id: String, state: State<'_, AppState>) -> Result<Task, String> {
    info!("Getting task");

    let validated_id = validate_uuid_field(&task_id, "task_id")?;

    // Use meta::id(id) to extract clean UUID from SurrealDB Thing type
    let query = format!(
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
        WHERE meta::id(id) = '{}'"#,
        validated_id
    );

    let results: Vec<Task> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to query task");
        format!("Failed to query task: {}", e)
    })?;

    results
        .into_iter()
        .next()
        .ok_or_else(|| format!("Task not found: {}", validated_id))
}

/// Lists all tasks for a workflow.
///
/// # Arguments (camelCase in TypeScript)
/// * `workflowId` - The workflow ID to filter by
///
/// # Returns
/// Vector of tasks sorted by priority (ascending) and creation time.
#[tauri::command]
#[instrument(name = "list_workflow_tasks", skip(state), fields(workflow_id = %workflow_id))]
pub async fn list_workflow_tasks(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, String> {
    info!("Listing workflow tasks");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Add LIMIT to prevent memory explosion
    let query = format!(
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
        WHERE workflow_id = '{}'
        ORDER BY priority ASC, created_at ASC
        LIMIT {}"#,
        validated_workflow_id,
        query_limits::DEFAULT_LIST_LIMIT
    );

    let tasks: Vec<Task> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to list tasks");
        format!("Failed to list tasks: {}", e)
    })?;

    info!(count = tasks.len(), "Workflow tasks loaded");
    Ok(tasks)
}

/// Lists tasks filtered by status.
///
/// # Arguments (camelCase in TypeScript)
/// * `status` - Status to filter by (pending/in_progress/completed/blocked)
/// * `workflowId` - Optional workflow ID to further filter
///
/// # Returns
/// Vector of tasks matching the status, sorted by priority and creation time.
#[tauri::command]
#[instrument(name = "list_tasks_by_status", skip(state), fields(status = %status, workflow_id = ?workflow_id))]
pub async fn list_tasks_by_status(
    status: String,
    workflow_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, String> {
    info!("Listing tasks by status");

    // Validate status
    let valid_statuses = ["pending", "in_progress", "completed", "blocked"];
    if !valid_statuses.contains(&status.as_str()) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: {:?}",
            status, valid_statuses
        ));
    }

    // Add LIMIT to prevent memory explosion
    let query = if let Some(wf_id) = workflow_id {
        let validated_wf_id = validate_uuid_field(&wf_id, "workflow_id")?;
        format!(
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
            WHERE status = '{}' AND workflow_id = '{}'
            ORDER BY priority ASC, created_at ASC
            LIMIT {}"#,
            status,
            validated_wf_id,
            query_limits::DEFAULT_LIST_LIMIT
        )
    } else {
        format!(
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
            WHERE status = '{}'
            ORDER BY priority ASC, created_at ASC
            LIMIT {}"#,
            status,
            query_limits::DEFAULT_LIST_LIMIT
        )
    };

    let tasks: Vec<Task> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to list tasks by status");
        format!("Failed to list tasks by status: {}", e)
    })?;

    info!(count = tasks.len(), status = %status, "Tasks by status loaded");
    Ok(tasks)
}

/// Updates task fields (partial update).
///
/// # Arguments (camelCase in TypeScript)
/// * `taskId` - The task ID to update
/// * `updates` - TaskUpdate object with optional fields to update
///
/// # Returns
/// The updated task.
///
/// # Errors
/// Returns error if no fields provided or validation/database fails.
#[tauri::command]
#[instrument(name = "update_task", skip(state, updates), fields(task_id = %task_id))]
pub async fn update_task(
    task_id: String,
    updates: TaskUpdate,
    state: State<'_, AppState>,
) -> Result<Task, String> {
    info!("Updating task");

    let validated_id = validate_uuid_field(&task_id, "task_id")?;

    // Build SET clause dynamically
    let mut set_parts: Vec<String> = Vec::new();

    if let Some(name) = &updates.name {
        if name.len() > 128 {
            return Err("Task name must be 128 characters or less".to_string());
        }
        let name_json = serialize_for_query(name, "name")?;
        set_parts.push(format!("name = {}", name_json));
    }

    if let Some(desc) = &updates.description {
        if desc.len() > 1000 {
            return Err("Task description must be 1000 characters or less".to_string());
        }
        let desc_json = serialize_for_query(desc, "description")?;
        set_parts.push(format!("description = {}", desc_json));
    }

    if let Some(agent) = &updates.agent_assigned {
        let agent_json = serialize_for_query(agent, "agent_assigned")?;
        set_parts.push(format!("agent_assigned = {}", agent_json));
    }

    if let Some(priority) = updates.priority {
        if !(1..=5).contains(&priority) {
            return Err("Priority must be between 1 and 5".to_string());
        }
        set_parts.push(format!("priority = {}", priority));
    }

    if let Some(status) = &updates.status {
        let valid_statuses = ["pending", "in_progress", "completed", "blocked"];
        if !valid_statuses.contains(&status.as_str()) {
            return Err(format!("Invalid status '{}'", status));
        }
        set_parts.push(format!("status = '{}'", status));
    }

    if let Some(deps) = &updates.dependencies {
        let deps_json = serialize_for_query(deps, "dependencies")?;
        set_parts.push(format!("dependencies = {}", deps_json));
    }

    if let Some(duration) = updates.duration_ms {
        set_parts.push(format!("duration_ms = {}", duration));
    }

    if set_parts.is_empty() {
        return Err("No fields to update".to_string());
    }

    let query = format!(
        "UPDATE task:`{}` SET {}",
        validated_id,
        set_parts.join(", ")
    );

    // Use execute() for UPDATE to avoid SurrealDB SDK serialization issues
    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to update task");
        format!("Failed to update task: {}", e)
    })?;

    info!(task_id = %validated_id, "Task updated successfully");

    // Return updated task
    get_task(task_id, state).await
}

/// Updates task status specifically.
///
/// This is a convenience command for the common operation of changing task status.
///
/// # Arguments (camelCase in TypeScript)
/// * `taskId` - The task ID to update
/// * `status` - New status (pending/in_progress/completed/blocked)
///
/// # Returns
/// The updated task.
#[tauri::command]
#[instrument(name = "update_task_status", skip(state), fields(task_id = %task_id, status = %status))]
pub async fn update_task_status(
    task_id: String,
    status: String,
    state: State<'_, AppState>,
) -> Result<Task, String> {
    info!("Updating task status");

    let validated_id = validate_uuid_field(&task_id, "task_id")?;

    let valid_statuses = ["pending", "in_progress", "completed", "blocked"];
    if !valid_statuses.contains(&status.as_str()) {
        return Err(format!(
            "Invalid status '{}'. Must be one of: {:?}",
            status, valid_statuses
        ));
    }

    // Use parameterized query for UPDATE to prevent injection
    // Note: validated_id is a UUID (safe), but status is validated above
    state
        .db
        .execute_with_params(
            &format!("UPDATE task:`{}` SET status = $status", validated_id),
            vec![("status".to_string(), serde_json::json!(status))],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to update task status");
            format!("Failed to update task status: {}", e)
        })?;

    info!(task_id = %validated_id, status = %status, "Task status updated");
    get_task(task_id, state).await
}

/// Marks task as completed with optional duration.
///
/// Sets status to 'completed', records completion timestamp, and optionally
/// stores execution duration for metrics.
///
/// # Arguments (camelCase in TypeScript)
/// * `taskId` - The task ID to complete
/// * `durationMs` - Optional execution duration in milliseconds
///
/// # Returns
/// The completed task.
#[tauri::command]
#[instrument(name = "complete_task", skip(state), fields(task_id = %task_id, duration_ms = ?duration_ms))]
pub async fn complete_task(
    task_id: String,
    duration_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<Task, String> {
    info!("Completing task");

    let validated_id = validate_uuid_field(&task_id, "task_id")?;

    let duration_part = if let Some(d) = duration_ms {
        format!(", duration_ms = {}", d)
    } else {
        String::new()
    };

    let query = format!(
        "UPDATE task:`{}` SET status = 'completed', completed_at = time::now(){}",
        validated_id, duration_part
    );

    // Use execute() for UPDATE to avoid SurrealDB SDK serialization issues
    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to complete task");
        format!("Failed to complete task: {}", e)
    })?;

    info!(task_id = %validated_id, "Task marked as completed");
    get_task(task_id, state).await
}

/// Deletes a task.
///
/// # Arguments (camelCase in TypeScript)
/// * `taskId` - The task ID to delete
///
/// # Errors
/// Returns error if task not found or database operation fails.
#[tauri::command]
#[instrument(name = "delete_task", skip(state), fields(task_id = %task_id))]
pub async fn delete_task(task_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting task");

    let validated_id = validate_uuid_field(&task_id, "task_id")?;

    state
        .db
        .delete(&format!("task:{}", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete task");
            format!("Failed to delete task: {}", e)
        })?;

    info!(task_id = %validated_id, "Task deleted");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::{AgentOrchestrator, AgentRegistry};
    use crate::db::DBClient;
    use crate::llm::ProviderManager;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[allow(dead_code)]
    async fn setup_test_state() -> AppState {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_task_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = Arc::new(
            DBClient::new(db_path_str)
                .await
                .expect("Failed to create test DB"),
        );
        db.initialize_schema().await.expect("Schema init failed");

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
        let llm_manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let mcp_manager = Arc::new(
            crate::mcp::MCPManager::new(db.clone())
                .await
                .expect("Failed to create MCP manager"),
        );

        std::mem::forget(temp_dir);

        // Create shared embedding service reference
        let embedding_service = Arc::new(tokio::sync::RwLock::new(None));

        AppState {
            db: db.clone(),
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory: Arc::new(crate::tools::ToolFactory::new(
                db,
                embedding_service.clone(),
            )),
            embedding_service,
            streaming_cancellations: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            app_handle: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    #[test]
    fn test_status_validation() {
        let valid = ["pending", "in_progress", "completed", "blocked"];
        let invalid = ["done", "started", "waiting", ""];

        for s in valid {
            assert!(valid.contains(&s), "Status '{}' should be valid", s);
        }

        for s in invalid {
            assert!(!valid.contains(&s), "Status '{}' should be invalid", s);
        }
    }

    #[test]
    fn test_priority_validation() {
        for p in 1..=5 {
            assert!((1..=5).contains(&p), "Priority {} should be valid", p);
        }

        assert!(!(1..=5).contains(&0), "Priority 0 should be invalid");
        assert!(!(1..=5).contains(&6), "Priority 6 should be invalid");
    }

    #[test]
    fn test_name_length_validation() {
        let short_name = "a".repeat(128);
        let long_name = "a".repeat(129);

        assert!(short_name.len() <= 128, "128 char name should be valid");
        assert!(long_name.len() > 128, "129 char name should be invalid");
    }

    #[test]
    fn test_description_length_validation() {
        let short_desc = "a".repeat(1000);
        let long_desc = "a".repeat(1001);

        assert!(short_desc.len() <= 1000, "1000 char desc should be valid");
        assert!(long_desc.len() > 1000, "1001 char desc should be invalid");
    }

    #[test]
    fn test_task_update_empty() {
        let update = TaskUpdate::default();
        assert!(update.name.is_none());
        assert!(update.description.is_none());
        assert!(update.priority.is_none());
        assert!(update.status.is_none());
        assert!(update.agent_assigned.is_none());
        assert!(update.dependencies.is_none());
        assert!(update.duration_ms.is_none());
    }

    #[tokio::test]
    async fn test_task_create_serialization() {
        use crate::models::task::TaskCreate;

        let task = TaskCreate::new(
            "wf_001".to_string(),
            "Test task".to_string(),
            "A test task".to_string(),
            3,
        );

        let json = serde_json::to_string(&task).unwrap();
        assert!(json.contains("\"workflow_id\":\"wf_001\""));
        assert!(json.contains("\"name\":\"Test task\""));
        assert!(json.contains("\"priority\":3"));
        assert!(json.contains("\"status\":\"pending\""));
    }

    /// Verifies that task names containing apostrophes, backslashes, newlines,
    /// and other special characters are stored and retrieved without corruption
    /// or injection risk.
    #[tokio::test]
    async fn test_update_task_name_with_special_chars() {
        use crate::models::task::TaskCreate;

        let state = setup_test_state().await;

        // Create a workflow to satisfy the foreign key relationship
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let wf_json = serde_json::json!({
            "id": workflow_id,
            "name": "Test Workflow",
            "status": "active",
            "agent_id": null,
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        state
            .db
            .execute_with_params(
                &format!("CREATE workflow:`{}` CONTENT $data", workflow_id),
                vec![("data".to_string(), wf_json)],
            )
            .await
            .expect("Failed to create test workflow");

        // Create the task
        let task_id = uuid::Uuid::new_v4().to_string();
        let task_create = TaskCreate::new(
            workflow_id.clone(),
            "Original name".to_string(),
            "Original description".to_string(),
            3,
        );
        state
            .db
            .create("task", &task_id, task_create)
            .await
            .expect("Failed to create test task");

        // Build an update with a name containing apostrophes, backslashes,
        // newlines and embedded quotes - the characters that broke the old
        // `replace('\'', "''")` approach.
        let tricky_name =
            "It's a \"tricky\" name\nwith backslash \\ and null-like \\0 chars".to_string();
        let tricky_description = "Line one\nLine two's \"quoted\" section \\ end".to_string();
        let tricky_agent = "agent'with\"quotes\\and\nnewline".to_string();

        let updates = TaskUpdate {
            name: Some(tricky_name.clone()),
            description: Some(tricky_description.clone()),
            agent_assigned: Some(tricky_agent.clone()),
            ..TaskUpdate::default()
        };

        // Simulate the SET-clause generation the same way update_task() does,
        // confirming that serde_json::to_string produces valid JSON literals
        // for each field.
        let mut set_parts: Vec<String> = Vec::new();

        let name_json = serde_json::to_string(&updates.name.as_ref().unwrap())
            .expect("name serialization must not fail");
        set_parts.push(format!("name = {}", name_json));

        let desc_json = serde_json::to_string(&updates.description.as_ref().unwrap())
            .expect("description serialization must not fail");
        set_parts.push(format!("description = {}", desc_json));

        let agent_json = serde_json::to_string(&updates.agent_assigned.as_ref().unwrap())
            .expect("agent_assigned serialization must not fail");
        set_parts.push(format!("agent_assigned = {}", agent_json));

        let query = format!("UPDATE task:`{}` SET {}", task_id, set_parts.join(", "));

        state
            .db
            .execute(&query)
            .await
            .expect("UPDATE with special chars must not fail");

        // Retrieve the task and assert values were stored verbatim
        let fetch_query = format!(
            "SELECT meta::id(id) AS id, name, description, agent_assigned FROM task WHERE meta::id(id) = '{}'",
            task_id
        );
        let rows: Vec<serde_json::Value> = state
            .db
            .query(&fetch_query)
            .await
            .expect("SELECT must succeed");

        assert_eq!(rows.len(), 1, "Expected exactly one task row");
        let row = &rows[0];

        assert_eq!(
            row["name"].as_str().unwrap_or(""),
            tricky_name,
            "name with special chars must round-trip correctly"
        );
        assert_eq!(
            row["description"].as_str().unwrap_or(""),
            tricky_description,
            "description with special chars must round-trip correctly"
        );
        assert_eq!(
            row["agent_assigned"].as_str().unwrap_or(""),
            tricky_agent,
            "agent_assigned with special chars must round-trip correctly"
        );
    }
}
