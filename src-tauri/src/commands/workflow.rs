// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    models::{Workflow, WorkflowMetrics, WorkflowResult, WorkflowStatus},
    AppState,
};
use tauri::State;

/// Creates a new workflow
#[tauri::command]
pub async fn create_workflow(
    name: String,
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    use chrono::Utc;
    use uuid::Uuid;

    let workflow = Workflow {
        id: Uuid::new_v4().to_string(),
        name,
        agent_id,
        status: WorkflowStatus::Idle,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
    };

    let id = state
        .db
        .create("workflow", workflow)
        .await
        .map_err(|e| format!("Failed to create workflow: {}", e))?;

    Ok(id)
}

/// Executes a workflow with a message
#[tauri::command]
pub async fn execute_workflow(
    workflow_id: String,
    message: String,
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowResult, String> {
    use crate::agents::core::agent::Task;
    use uuid::Uuid;

    // 1. Load workflow
    let workflows: Vec<Workflow> = state
        .db
        .query(&format!(
            "SELECT * FROM workflow WHERE id = '{}'",
            workflow_id
        ))
        .await
        .map_err(|e| format!("Failed to load workflow: {}", e))?;

    let _workflow = workflows.first().ok_or("Workflow not found")?;

    // 2. Create task
    let task = Task {
        id: Uuid::new_v4().to_string(),
        description: message,
        context: serde_json::json!({}),
    };

    // 3. Execute via orchestrator
    let report = state
        .orchestrator
        .execute(&agent_id, task)
        .await
        .map_err(|e| format!("Execution failed: {}", e))?;

    // 4. Build result
    let result = WorkflowResult {
        report: report.content,
        metrics: WorkflowMetrics {
            duration_ms: report.metrics.duration_ms,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cost_usd: 0.0,
            provider: "Demo".to_string(),
            model: "simple_agent".to_string(),
        },
        tools_used: report.metrics.tools_used,
        mcp_calls: report.metrics.mcp_calls,
    };

    Ok(result)
}

/// Loads all workflows
#[tauri::command]
pub async fn load_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String> {
    let workflows = state
        .db
        .query("SELECT * FROM workflow ORDER BY updated_at DESC")
        .await
        .map_err(|e| format!("Failed to load workflows: {}", e))?;

    Ok(workflows)
}

/// Deletes a workflow
#[tauri::command]
pub async fn delete_workflow(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .db
        .delete(&format!("workflow:{}", id))
        .await
        .map_err(|e| format!("Failed to delete workflow: {}", e))?;

    Ok(())
}
