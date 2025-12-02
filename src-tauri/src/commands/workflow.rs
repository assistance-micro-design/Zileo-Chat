// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    models::{
        Message, ThinkingStep, ToolExecution, Workflow, WorkflowCreate, WorkflowFullState,
        WorkflowMetrics, WorkflowResult, WorkflowStatus, WorkflowToolExecution,
    },
    security::Validator,
    AppState,
};
use std::sync::Arc;
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

    // Validate inputs
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    let validated_message = Validator::validate_message(&message).map_err(|e| {
        warn!(error = %e, "Invalid message");
        format!("Invalid message: {}", e)
    })?;

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent ID");
        format!("Invalid agent ID: {}", e)
    })?;

    // 1. Load workflow with explicit ID conversion to avoid SurrealDB Thing enum issues
    // Use meta::id() to extract the UUID without SurrealDB's angle brackets
    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            name,
            agent_id,
            status,
            created_at,
            updated_at,
            completed_at
        FROM workflow
        WHERE meta::id(id) = '{}'"#,
        validated_workflow_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflow");
        format!("Failed to load workflow: {}", e)
    })?;

    let workflows: Vec<Workflow> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Workflow>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize workflow");
            format!("Failed to deserialize workflow: {}", e)
        })?;

    let _workflow = workflows.first().ok_or_else(|| {
        warn!(workflow_id = %validated_workflow_id, "Workflow not found");
        "Workflow not found".to_string()
    })?;

    // 2. Create task
    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for workflow");

    let task = Task {
        id: task_id.clone(),
        description: validated_message,
        context: serde_json::json!({}),
    };

    // 3. Execute via orchestrator with MCP support
    let report = state
        .orchestrator
        .execute_with_mcp(&validated_agent_id, task, Some(state.mcp_manager.clone()))
        .await
        .map_err(|e| {
            error!(error = %e, task_id = %task_id, "Workflow execution failed");
            format!("Execution failed: {}", e)
        })?;

    // 4. Get agent config for accurate provider/model info
    let (provider, model) = match state.registry.get(&validated_agent_id).await {
        Some(agent) => {
            let config = agent.config();
            (config.llm.provider.clone(), config.llm.model.clone())
        }
        None => {
            // Fallback if agent not found (shouldn't happen after successful execution)
            ("Unknown".to_string(), validated_agent_id.clone())
        }
    };

    // 5. Build result
    // Note: cost_usd calculation requires provider-specific pricing APIs (future enhancement)
    // Convert tool executions to IPC-friendly format
    let tool_executions: Vec<WorkflowToolExecution> = report
        .metrics
        .tool_executions
        .iter()
        .map(|te| WorkflowToolExecution {
            tool_type: te.tool_type.clone(),
            tool_name: te.tool_name.clone(),
            server_name: te.server_name.clone(),
            input_params: te.input_params.clone(),
            output_result: te.output_result.clone(),
            success: te.success,
            error_message: te.error_message.clone(),
            duration_ms: te.duration_ms,
            iteration: te.iteration,
        })
        .collect();

    let result = WorkflowResult {
        report: report.content,
        metrics: WorkflowMetrics {
            duration_ms: report.metrics.duration_ms,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cost_usd: 0.0,
            provider,
            model,
        },
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
        tool_executions,
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
///
/// Uses a query that explicitly converts the record ID to string to avoid
/// SurrealDB SDK serialization issues with the Thing type.
#[tauri::command]
#[instrument(name = "load_workflows", skip(state))]
pub async fn load_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String> {
    info!("Loading workflows");

    // Query with explicit ID conversion to avoid SurrealDB Thing enum serialization issues
    // Use meta::id() to extract the UUID without SurrealDB's angle brackets
    let query = r#"
        SELECT
            meta::id(id) AS id,
            name,
            agent_id,
            status,
            created_at,
            updated_at,
            completed_at,
            (total_tokens_input ?? 0) AS total_tokens_input,
            (total_tokens_output ?? 0) AS total_tokens_output,
            (total_cost_usd ?? 0.0) AS total_cost_usd,
            model_id
        FROM workflow
        ORDER BY updated_at DESC
    "#;

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
#[instrument(name = "delete_workflow", skip(state), fields(workflow_id = %id))]
pub async fn delete_workflow(id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting workflow with cascade");

    // Validate input
    let validated_id = Validator::validate_uuid(&id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // Delete related entities in parallel (order doesn't matter for independent tables)
    let db = Arc::clone(&state.db);
    let db2 = Arc::clone(&state.db);
    let db3 = Arc::clone(&state.db);
    let db4 = Arc::clone(&state.db);
    let db5 = Arc::clone(&state.db);
    let db6 = Arc::clone(&state.db);
    let db7 = Arc::clone(&state.db);

    let id1 = validated_id.clone();
    let id2 = validated_id.clone();
    let id3 = validated_id.clone();
    let id4 = validated_id.clone();
    let id5 = validated_id.clone();
    let id6 = validated_id.clone();
    let id7 = validated_id.clone();

    // Execute cascade deletes in parallel
    let (tasks, messages, tools, thinking, sub_agents, validations, memories) = tokio::join!(
        // Delete tasks
        async move {
            let query = format!("DELETE task WHERE workflow_id = '{}'", id1);
            match db.execute(&query).await {
                Ok(_) => info!("Deleted tasks for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete tasks (may not exist)"),
            }
        },
        // Delete messages
        async move {
            let query = format!("DELETE message WHERE workflow_id = '{}'", id2);
            match db2.execute(&query).await {
                Ok(_) => info!("Deleted messages for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete messages (may not exist)"),
            }
        },
        // Delete tool executions
        async move {
            let query = format!("DELETE tool_execution WHERE workflow_id = '{}'", id3);
            match db3.execute(&query).await {
                Ok(_) => info!("Deleted tool executions for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete tool executions (may not exist)"),
            }
        },
        // Delete thinking steps
        async move {
            let query = format!("DELETE thinking_step WHERE workflow_id = '{}'", id4);
            match db4.execute(&query).await {
                Ok(_) => info!("Deleted thinking steps for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete thinking steps (may not exist)"),
            }
        },
        // Delete sub-agent executions
        async move {
            let query = format!("DELETE sub_agent_execution WHERE workflow_id = '{}'", id5);
            match db5.execute(&query).await {
                Ok(_) => info!("Deleted sub-agent executions for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete sub-agent executions (may not exist)"),
            }
        },
        // Delete validation requests
        async move {
            let query = format!("DELETE validation_request WHERE workflow_id = '{}'", id6);
            match db6.execute(&query).await {
                Ok(_) => info!("Deleted validation requests for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete validation requests (may not exist)"),
            }
        },
        // Delete workflow-scoped memories
        async move {
            let query = format!("DELETE memory WHERE workflow_id = '{}'", id7);
            match db7.execute(&query).await {
                Ok(_) => info!("Deleted memories for workflow"),
                Err(e) => warn!(error = %e, "Failed to delete memories (may not exist)"),
            }
        }
    );

    // Consume the unit values to avoid warnings
    let _ = (tasks, messages, tools, thinking, sub_agents, validations, memories);

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
/// Phase 5: Complete State Recovery
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
    info!("Loading complete workflow state for recovery");

    // Validate workflow ID
    let validated_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    // Clone db Arc for parallel queries
    let db = Arc::clone(&state.db);
    let db2 = Arc::clone(&state.db);
    let db3 = Arc::clone(&state.db);
    let db4 = Arc::clone(&state.db);

    let id1 = validated_id.clone();
    let id2 = validated_id.clone();
    let id3 = validated_id.clone();
    let id4 = validated_id.clone();

    // Execute all queries in parallel using tokio::try_join!
    let (workflow_result, messages_result, tools_result, thinking_result) = tokio::try_join!(
        // Query 1: Load workflow
        async move {
            let query = format!(
                r#"SELECT
                    meta::id(id) AS id,
                    name,
                    agent_id,
                    status,
                    created_at,
                    updated_at,
                    completed_at,
                    (total_tokens_input ?? 0) AS total_tokens_input,
                    (total_tokens_output ?? 0) AS total_tokens_output,
                    (total_cost_usd ?? 0.0) AS total_cost_usd,
                    model_id
                FROM workflow
                WHERE meta::id(id) = '{}'"#,
                id1
            );

            let json_results = db.query_json(&query).await.map_err(|e| {
                error!(error = %e, "Failed to load workflow");
                format!("Failed to load workflow: {}", e)
            })?;

            let workflows: Vec<Workflow> = json_results
                .into_iter()
                .map(serde_json::from_value)
                .collect::<std::result::Result<Vec<Workflow>, _>>()
                .map_err(|e| {
                    error!(error = %e, "Failed to deserialize workflow");
                    format!("Failed to deserialize workflow: {}", e)
                })?;

            workflows.into_iter().next().ok_or_else(|| {
                warn!(workflow_id = %id1, "Workflow not found");
                "Workflow not found".to_string()
            })
        },
        // Query 2: Load messages
        async move {
            let query = format!(
                r#"SELECT
                    meta::id(id) AS id,
                    workflow_id,
                    role,
                    content,
                    tokens,
                    tokens_input,
                    tokens_output,
                    model,
                    provider,
                    cost_usd,
                    duration_ms,
                    timestamp
                FROM message
                WHERE workflow_id = '{}'
                ORDER BY timestamp ASC"#,
                id2
            );

            let json_results = db2.query_json(&query).await.map_err(|e| {
                error!(error = %e, "Failed to load messages");
                format!("Failed to load messages: {}", e)
            })?;

            let messages: Vec<Message> = json_results
                .into_iter()
                .map(serde_json::from_value)
                .collect::<std::result::Result<Vec<Message>, _>>()
                .map_err(|e| {
                    error!(error = %e, "Failed to deserialize messages");
                    format!("Failed to deserialize messages: {}", e)
                })?;

            Ok::<Vec<Message>, String>(messages)
        },
        // Query 3: Load tool executions
        async move {
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
                    created_at
                FROM tool_execution
                WHERE workflow_id = '{}'
                ORDER BY created_at ASC"#,
                id3
            );

            let json_results = db3.query_json(&query).await.map_err(|e| {
                error!(error = %e, "Failed to load tool executions");
                format!("Failed to load tool executions: {}", e)
            })?;

            let tools: Vec<ToolExecution> = json_results
                .into_iter()
                .map(serde_json::from_value)
                .collect::<std::result::Result<Vec<ToolExecution>, _>>()
                .map_err(|e| {
                    error!(error = %e, "Failed to deserialize tool executions");
                    format!("Failed to deserialize tool executions: {}", e)
                })?;

            Ok::<Vec<ToolExecution>, String>(tools)
        },
        // Query 4: Load thinking steps
        async move {
            let query = format!(
                r#"SELECT
                    meta::id(id) AS id,
                    workflow_id,
                    message_id,
                    agent_id,
                    step_number,
                    content,
                    duration_ms,
                    tokens,
                    created_at
                FROM thinking_step
                WHERE workflow_id = '{}'
                ORDER BY created_at ASC, step_number ASC"#,
                id4
            );

            let json_results = db4.query_json(&query).await.map_err(|e| {
                error!(error = %e, "Failed to load thinking steps");
                format!("Failed to load thinking steps: {}", e)
            })?;

            let steps: Vec<ThinkingStep> = json_results
                .into_iter()
                .map(serde_json::from_value)
                .collect::<std::result::Result<Vec<ThinkingStep>, _>>()
                .map_err(|e| {
                    error!(error = %e, "Failed to deserialize thinking steps");
                    format!("Failed to deserialize thinking steps: {}", e)
                })?;

            Ok::<Vec<ThinkingStep>, String>(steps)
        }
    )?;

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
            max_tool_iterations: 50,
        };
        let agent = SimpleAgent::new(config);
        registry
            .register("test_agent".to_string(), Arc::new(agent))
            .await;

        let llm_manager = Arc::new(crate::llm::ProviderManager::new());
        let mcp_manager = Arc::new(
            crate::mcp::MCPManager::new(db.clone())
                .await
                .expect("Failed to create MCP manager"),
        );

        // Leak temp_dir to keep it alive during test
        std::mem::forget(temp_dir);

        AppState {
            db: db.clone(),
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory: Arc::new(crate::tools::ToolFactory::new(db, None)),
            embedding_service: Arc::new(tokio::sync::RwLock::new(None)),
            streaming_cancellations: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
            app_handle: Arc::new(std::sync::RwLock::new(None)),
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
            tool_executions: vec![],
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
