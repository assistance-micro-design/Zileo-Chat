use super::*;
use crate::models::sub_agent::{ParallelBatchResult, ParallelTaskResult, SubAgentMetrics};

#[test]
fn test_tool_definition() {
    let definition = ToolDefinition {
        id: "ParallelTasksTool".to_string(),
        name: "Parallel Tasks".to_string(),
        summary: "Execute multiple independent tasks concurrently".to_string(),
        description: "Parallel tasks tool for tests".to_string(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        requires_confirmation: false,
    };

    assert_eq!(definition.id, "ParallelTasksTool");
    assert!(!definition.requires_confirmation);
}

#[test]
fn test_parallel_task_spec_serialization() {
    let spec = ParallelTaskSpec {
        agent_id: "db_agent".to_string(),
        agent_name: "Database Agent".to_string(),
        prompt: "Analyze schema".to_string(),
    };

    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("db_agent"));
    assert!(json.contains("Database Agent"));
    assert!(json.contains("Analyze schema"));

    let deserialized: ParallelTaskSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.agent_id, "db_agent");
    assert_eq!(deserialized.agent_name, "Database Agent");
}

#[test]
fn test_input_validation_execute_batch() {
    let valid_input = serde_json::json!({
        "operation": "execute_batch",
        "tasks": [
            {"agent_id": "db_agent", "prompt": "Analyze database"},
            {"agent_id": "api_agent", "prompt": "Check API security"}
        ]
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "execute_batch");
    assert!(valid_input["tasks"].is_array());
    assert_eq!(valid_input["tasks"].as_array().unwrap().len(), 2);
}

#[test]
fn test_input_validation_too_many_tasks() {
    let mut tasks = Vec::new();
    for i in 0..=MAX_SUB_AGENTS {
        tasks.push(serde_json::json!({"agent_id": format!("a{}", i), "prompt": format!("p{}", i)}));
    }
    let invalid_input = serde_json::json!({
        "operation": "execute_batch",
        "tasks": tasks
    });

    let tasks_len = invalid_input["tasks"].as_array().unwrap().len();
    assert!(tasks_len > MAX_SUB_AGENTS);
}

#[test]
fn test_parallel_batch_result_serialization() {
    let result = ParallelBatchResult {
        success: true,
        completed: 2,
        failed: 0,
        results: vec![
            ParallelTaskResult {
                agent_id: "agent_1".to_string(),
                success: true,
                report: Some("Report 1".to_string()),
                error: None,
                metrics: Some(SubAgentMetrics {
                    duration_ms: 1000,
                    tokens_input: 100,
                    tokens_output: 200,
                }),
            },
            ParallelTaskResult {
                agent_id: "agent_2".to_string(),
                success: true,
                report: Some("Report 2".to_string()),
                error: None,
                metrics: Some(SubAgentMetrics {
                    duration_ms: 1500,
                    tokens_input: 150,
                    tokens_output: 250,
                }),
            },
        ],
        aggregated_report: "# Combined Report".to_string(),
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"completed\":2"));
    assert!(json.contains("\"failed\":0"));
    assert!(json.contains("agent_1"));
    assert!(json.contains("agent_2"));
}

#[test]
fn test_parallel_task_result_with_error() {
    let result = ParallelTaskResult {
        agent_id: "failed_agent".to_string(),
        success: false,
        report: None,
        error: Some("Connection timeout".to_string()),
        metrics: None,
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":false"));
    assert!(json.contains("Connection timeout"));
    assert!(json.contains("\"report\":null"));
}

#[test]
fn test_max_sub_agents_constant() {
    assert_eq!(MAX_SUB_AGENTS, 15);
}

// --- ParallelTasksTool accepts agent_name ---

#[test]
fn test_validate_parallel_task_accepts_agent_id() {
    let task = serde_json::json!({
        "agent_id": "some-uuid-123",
        "prompt": "Analyze the database"
    });
    let result = validate_parallel_task_item(&task, 0);
    assert!(result.is_ok());
}

#[test]
fn test_validate_parallel_task_accepts_agent_name() {
    let task = serde_json::json!({
        "agent_name": "Database Agent",
        "prompt": "Analyze the database"
    });
    let result = validate_parallel_task_item(&task, 0);
    assert!(result.is_ok());
}

#[test]
fn test_validate_parallel_task_rejects_missing_both() {
    let task = serde_json::json!({
        "prompt": "Analyze the database"
    });
    let result = validate_parallel_task_item(&task, 0);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[test]
fn test_definition_has_agent_name_property() {
    let schema = parallel_tasks_input_schema();
    let items = schema["properties"]["tasks"]["items"]["properties"]
        .as_object()
        .unwrap();
    assert!(
        items.contains_key("agent_name"),
        "Schema items must contain agent_name property"
    );
    assert!(
        items.contains_key("agent_id"),
        "Schema items must still contain agent_id property"
    );
}

#[test]
fn test_parallel_task_spec_includes_agent_name() {
    let spec = ParallelTaskSpec {
        agent_id: "uuid-123".to_string(),
        agent_name: "Database Agent".to_string(),
        prompt: "Analyze schema".to_string(),
    };
    assert_eq!(spec.agent_name, "Database Agent");
    assert_eq!(spec.agent_id, "uuid-123");
}
