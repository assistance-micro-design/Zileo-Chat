use super::*;
use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::agents::SimpleAgent;
use crate::db::DBClient;
use crate::models::{AgentConfig, LLMConfig, Lifecycle, WorkflowMetrics, WorkflowResult};
use crate::test_utils::test_tempdir;
use std::sync::Arc;
use tempfile::TempDir;

/// Helper to create test AppState with temporary database (schemaless for tests).
///
/// Returns `(AppState, TempDir)`. The caller must bind the `TempDir` so the
/// directory outlives the test; dropping it early breaks RocksDB.
async fn setup_test_state_for_orchestrator() -> (AppState, TempDir) {
    let temp_dir = test_tempdir();
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
            is_reasoning: false,
            context_window: None,
        },
        tools: vec![],
        mcp_servers: vec![],
        skills: vec![],
        folders: vec![],
        require_file_confirmation: true,
        system_prompt: "Test agent".to_string(),
        max_tool_iterations: 50,
        reasoning_effort: None,
    };
    let agent = SimpleAgent::new(config);
    registry
        .register("test_agent".to_string(), Arc::new(agent))
        .await;

    let llm_manager = Arc::new(crate::llm::ProviderManager::new().expect("test provider manager"));
    let mcp_manager = Arc::new(
        crate::mcp::MCPManager::new(db.clone())
            .await
            .expect("Failed to create MCP manager"),
    );

    // Create shared embedding service reference
    let embedding_service = Arc::new(tokio::sync::RwLock::new(None));

    let state = AppState {
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
        streaming_cancellations: Arc::new(
            tokio::sync::Mutex::new(std::collections::HashMap::new()),
        ),
        app_handle: Arc::new(std::sync::RwLock::new(None)),
        audit_cleanup_handle: Arc::new(tokio::sync::Mutex::new(None)),
    };

    (state, temp_dir)
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
        response: "Content here".to_string(),
        metrics: WorkflowMetrics {
            duration_ms: 100,
            tokens_input: 50,
            tokens_output: 75,
            cost_usd: 0.001,
            provider: "Test".to_string(),
            model: "test-model".to_string(),
            cached_tokens: None,
            cache_write_tokens: None,
            thinking_tokens: None,
            iteration_metrics: vec![],
        },
        tools_used: vec!["tool1".to_string()],
        mcp_calls: vec![],
        tool_executions: vec![],
        message_id: "test-message-id".to_string(),
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
    let (state, _db_guard) = setup_test_state_for_orchestrator().await;

    use crate::agents::core::agent::Task;

    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: "Test task description".to_string(),
        context: serde_json::json!({}),
    };

    let result = state
        .orchestrator
        .execute_with_mcp("test_agent", task, None, None)
        .await;
    assert!(result.is_ok(), "Orchestrator execution should succeed");

    let report = result.unwrap();
    assert!(report.content.contains("# Agent Report"));
}

#[tokio::test]
async fn test_orchestrator_execute_nonexistent_agent() {
    let (state, _db_guard) = setup_test_state_for_orchestrator().await;

    use crate::agents::core::agent::Task;

    let task = Task {
        id: uuid::Uuid::new_v4().to_string(),
        description: "Test task".to_string(),
        context: serde_json::json!({}),
    };

    let result = state
        .orchestrator
        .execute_with_mcp("nonexistent_agent", task, None, None)
        .await;
    assert!(result.is_err(), "Should fail for nonexistent agent");
}

#[tokio::test]
async fn test_batch_delete_result_serialization() {
    let result = BatchDeleteResult {
        deleted: 3,
        skipped_running: vec!["id-1".to_string(), "id-2".to_string()],
    };
    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"deleted\":3"));
    assert!(json.contains("\"skipped_running\""));

    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["deleted"], 3);
    assert_eq!(parsed["skipped_running"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_toggle_workflow_pinned() {
    let (state, _db_guard) = crate::test_utils::setup_test_state().await;

    // Seed a workflow
    let workflow_id = uuid::Uuid::new_v4().to_string();
    let wf_json = serde_json::json!({
        "name": "Pin Test Workflow",
        "status": "idle",
        "agent_id": "test-agent",
        "pinned": false,
    });
    state
        .db
        .execute_with_params(
            &format!("CREATE workflow:`{}` CONTENT $data", workflow_id),
            vec![("data".to_string(), wf_json)],
        )
        .await
        .expect("Failed to create test workflow");

    // Toggle pin ON
    let query_on = format!(
        "UPDATE workflow:`{}` SET pinned = !pinned, updated_at = time::now() RETURN {}",
        workflow_id,
        wf_queries::RETURN_FIELDS
    );
    let results_on = state
        .db
        .query_json(&query_on)
        .await
        .expect("Toggle ON failed");
    let wf_on: Workflow = serde_json::from_value(results_on.into_iter().next().unwrap()).unwrap();
    assert!(wf_on.pinned, "Workflow should be pinned after first toggle");

    // Toggle pin OFF
    let query_off = query_on.clone();
    let results_off = state
        .db
        .query_json(&query_off)
        .await
        .expect("Toggle OFF failed");
    let wf_off: Workflow = serde_json::from_value(results_off.into_iter().next().unwrap()).unwrap();
    assert!(
        !wf_off.pinned,
        "Workflow should be unpinned after second toggle"
    );
}
