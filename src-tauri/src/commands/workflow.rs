// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    models::{Workflow, WorkflowMetrics, WorkflowResult, WorkflowStatus},
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};

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
    use chrono::Utc;
    use uuid::Uuid;

    info!("Creating new workflow");

    let workflow = Workflow {
        id: Uuid::new_v4().to_string(),
        name: name.clone(),
        agent_id: agent_id.clone(),
        status: WorkflowStatus::Idle,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
    };

    let id = state.db.create("workflow", workflow).await.map_err(|e| {
        error!(error = %e, "Failed to create workflow");
        format!("Failed to create workflow: {}", e)
    })?;

    info!(workflow_id = %id, "Workflow created successfully");
    Ok(id)
}

/// Executes a workflow with a message
#[tauri::command]
#[instrument(
    name = "execute_workflow",
    skip(state, message),
    fields(
        workflow_id = %workflow_id,
        agent_id = %agent_id,
        message_len = message.len()
    )
)]
pub async fn execute_workflow(
    workflow_id: String,
    message: String,
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowResult, String> {
    use crate::agents::core::agent::Task;
    use uuid::Uuid;

    info!("Starting workflow execution");

    // 1. Load workflow
    let workflows: Vec<Workflow> = state
        .db
        .query(&format!(
            "SELECT * FROM workflow WHERE id = '{}'",
            workflow_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflow");
            format!("Failed to load workflow: {}", e)
        })?;

    let _workflow = workflows.first().ok_or_else(|| {
        warn!(workflow_id = %workflow_id, "Workflow not found");
        "Workflow not found".to_string()
    })?;

    // 2. Create task
    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for workflow");

    let task = Task {
        id: task_id.clone(),
        description: message,
        context: serde_json::json!({}),
    };

    // 3. Execute via orchestrator
    let report = state
        .orchestrator
        .execute(&agent_id, task)
        .await
        .map_err(|e| {
            error!(error = %e, task_id = %task_id, "Workflow execution failed");
            format!("Execution failed: {}", e)
        })?;

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
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
    };

    info!(
        duration_ms = result.metrics.duration_ms,
        tokens_input = result.metrics.tokens_input,
        tokens_output = result.metrics.tokens_output,
        tools_count = result.tools_used.len(),
        "Workflow execution completed"
    );

    Ok(result)
}

/// Loads all workflows
#[tauri::command]
#[instrument(name = "load_workflows", skip(state))]
pub async fn load_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String> {
    info!("Loading workflows");

    let workflows: Vec<Workflow> = state
        .db
        .query("SELECT * FROM workflow ORDER BY updated_at DESC")
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflows");
            format!("Failed to load workflows: {}", e)
        })?;

    info!(count = workflows.len(), "Workflows loaded");
    Ok(workflows)
}

/// Deletes a workflow
#[tauri::command]
#[instrument(name = "delete_workflow", skip(state), fields(workflow_id = %id))]
pub async fn delete_workflow(id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting workflow");

    state
        .db
        .delete(&format!("workflow:{}", id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete workflow");
            format!("Failed to delete workflow: {}", e)
        })?;

    info!("Workflow deleted successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::{AgentOrchestrator, AgentRegistry};
    use crate::agents::SimpleAgent;
    use crate::db::DBClient;
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Helper to create test AppState with temporary database (schemaless for tests)
    async fn setup_test_state_for_orchestrator() -> AppState {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = Arc::new(
            DBClient::new(db_path_str)
                .await
                .expect("Failed to create test DB"),
        );
        // Skip schema initialization for these tests - focus on orchestrator logic

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));

        // Register test agent
        let config = AgentConfig {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Demo".to_string(),
                model: "test".to_string(),
                temperature: 0.7,
                max_tokens: 1000,
            },
            tools: vec![],
            mcp_servers: vec![],
            system_prompt: "Test agent".to_string(),
        };
        let agent = SimpleAgent::new(config);
        registry
            .register("test_agent".to_string(), Arc::new(agent))
            .await;

        // Leak temp_dir to keep it alive during test
        std::mem::forget(temp_dir);

        AppState {
            db,
            registry,
            orchestrator,
        }
    }

    #[tokio::test]
    async fn test_workflow_status_values() {
        // Test all WorkflowStatus variants serialize correctly
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Idle).unwrap(),
            "\"idle\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&WorkflowStatus::Error).unwrap(),
            "\"error\""
        );
    }

    #[tokio::test]
    async fn test_workflow_result_structure() {
        let result = WorkflowResult {
            report: "# Test Report\n\nContent here".to_string(),
            metrics: WorkflowMetrics {
                duration_ms: 100,
                tokens_input: 50,
                tokens_output: 75,
                cost_usd: 0.001,
                provider: "Test".to_string(),
                model: "test-model".to_string(),
            },
            tools_used: vec!["tool1".to_string()],
            mcp_calls: vec![],
        };

        // Verify serialization works
        let json = serde_json::to_string(&result);
        assert!(json.is_ok(), "WorkflowResult should serialize");

        // Verify fields
        assert!(result.report.contains("# Test Report"));
        assert_eq!(result.metrics.duration_ms, 100);
        assert_eq!(result.metrics.tokens_input, 50);
        assert_eq!(result.tools_used.len(), 1);
    }

    #[tokio::test]
    async fn test_orchestrator_execute_task() {
        let state = setup_test_state_for_orchestrator().await;

        use crate::agents::core::agent::Task;

        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            description: "Test task description".to_string(),
            context: serde_json::json!({}),
        };

        let result = state.orchestrator.execute("test_agent", task).await;
        assert!(result.is_ok(), "Orchestrator execution should succeed");

        let report = result.unwrap();
        assert!(report.content.contains("# Agent Report"));
    }

    #[tokio::test]
    async fn test_orchestrator_execute_nonexistent_agent() {
        let state = setup_test_state_for_orchestrator().await;

        use crate::agents::core::agent::Task;

        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            description: "Test task".to_string(),
            context: serde_json::json!({}),
        };

        let result = state.orchestrator.execute("nonexistent_agent", task).await;
        assert!(result.is_err(), "Should fail for nonexistent agent");
    }

    #[tokio::test]
    async fn test_workflow_metrics_defaults() {
        let metrics = WorkflowMetrics {
            duration_ms: 0,
            tokens_input: 0,
            tokens_output: 0,
            cost_usd: 0.0,
            provider: String::new(),
            model: String::new(),
        };

        assert_eq!(metrics.duration_ms, 0);
        assert_eq!(metrics.cost_usd, 0.0);
    }
}
