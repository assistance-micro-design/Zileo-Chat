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
    use crate::tools::constants::workflow as wf_const;
    use tokio::time::{timeout, Duration};
    use uuid::Uuid;

    info!("Starting workflow execution");

    // Validate inputs
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    let validated_message = Validator::validate_message(&message).map_err(|e| {
        warn!(error = %e, "Invalid message");
        format!("Invalid message: {}", e)
    })?;

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    // 1. Load workflow (OPT-WF-1: Use centralized query constant)
    let query = format!(
        "{} WHERE meta::id(id) = '{}'",
        wf_queries::SELECT_BASIC,
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

    // 3. Execute via orchestrator with MCP support (OPT-WF-9: with timeout)
    let execution_future = state.orchestrator.execute_with_mcp(
        &validated_agent_id,
        task,
        Some(state.mcp_manager.clone()),
    );

    let report = timeout(
        Duration::from_secs(wf_const::LLM_EXECUTION_TIMEOUT_SECS),
        execution_future,
    )
    .await
    .map_err(|_| {
        error!(task_id = %task_id, timeout_secs = wf_const::LLM_EXECUTION_TIMEOUT_SECS, "Workflow execution timed out");
        format!(
            "Workflow execution timed out after {} seconds",
            wf_const::LLM_EXECUTION_TIMEOUT_SECS
        )
    })?
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

    // OPT-WF-1: Use centralized query constant
    let query = wf_queries::SELECT_LIST;

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
#[instrument(name = "delete_workflow", skip(state), fields(workflow_id = %workflow_id))]
pub async fn delete_workflow(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Deleting workflow with cascade");

    // Validate input
    let validated_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    // OPT-WF-8: Use centralized cascade delete helper
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
    use crate::tools::constants::workflow as wf_const;
    use tokio::time::{timeout, Duration};

    info!("Loading complete workflow state for recovery");

    // Validate workflow ID
    let validated_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
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

    // Execute all queries in parallel using tokio::try_join! (OPT-WF-9: with timeout)
    let parallel_queries = async {
        tokio::try_join!(
            // Query 1: Load workflow (OPT-WF-1: Use centralized query constant)
            async move {
                let query = format!("{} WHERE meta::id(id) = '{}'", wf_queries::SELECT_BASE, id1);

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
            enable_thinking: true,
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
