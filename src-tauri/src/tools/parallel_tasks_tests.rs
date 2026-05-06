use super::*;
use crate::models::sub_agent::{
    constants::MAX_PARALLEL_TASKS_PER_BATCH, ParallelBatchResult, ParallelTaskResult,
    SubAgentMetrics,
};

#[test]
fn test_parallel_task_spec_serialization() {
    let spec = ParallelTaskSpec {
        agent_id: "db_agent".to_string(),
        agent_name: "Database Agent".to_string(),
        prompt: "Analyze schema".to_string(),
        task_ids: None,
    };

    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("db_agent"));
    assert!(json.contains("Database Agent"));
    assert!(json.contains("Analyze schema"));
    assert!(
        !json.contains("task_ids"),
        "task_ids should be skipped when None"
    );

    let deserialized: ParallelTaskSpec = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.agent_id, "db_agent");
    assert_eq!(deserialized.agent_name, "Database Agent");
    assert!(deserialized.task_ids.is_none());
}

#[test]
fn test_validate_batch_size_accepts_one() {
    assert!(validate_batch_size(1).is_ok());
}

#[test]
fn test_validate_batch_size_accepts_max() {
    assert!(validate_batch_size(MAX_PARALLEL_TASKS_PER_BATCH).is_ok());
}

#[test]
fn test_validate_batch_size_rejects_zero() {
    let err = validate_batch_size(0).unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[test]
fn test_validate_batch_size_rejects_above_max() {
    let err = validate_batch_size(MAX_PARALLEL_TASKS_PER_BATCH + 1).unwrap_err();
    match err {
        ToolError::ValidationFailed(msg) => {
            assert!(msg.contains(&MAX_PARALLEL_TASKS_PER_BATCH.to_string()));
            assert!(msg.contains("parallel tasks"));
        }
        other => panic!("expected ValidationFailed, got {:?}", other),
    }
}

#[test]
fn test_validate_batch_size_rejects_total_workflow_limit() {
    // Regression: MAX_SUB_AGENTS (15) is the cumulative per-workflow cap,
    // not a per-batch cap. A batch of 4..=15 must still be rejected.
    let err = validate_batch_size(MAX_SUB_AGENTS).unwrap_err();
    assert!(matches!(err, ToolError::ValidationFailed(_)));
}

#[test]
fn test_definition_text_matches_batch_constant() {
    let def = DEFINITION.clone();
    let expected = format!("max {} per batch", MAX_PARALLEL_TASKS_PER_BATCH);
    assert!(
        def.description.contains(&expected),
        "tool description must mention 'max {} per batch', got: {}",
        MAX_PARALLEL_TASKS_PER_BATCH,
        def.description
    );
}

#[test]
fn test_input_schema_max_items_matches_batch_constant() {
    let schema = parallel_tasks_input_schema();
    let max_items = schema["properties"]["tasks"]["maxItems"]
        .as_u64()
        .expect("maxItems must be a number");
    assert_eq!(max_items as usize, MAX_PARALLEL_TASKS_PER_BATCH);
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
                    cached_tokens: None,
                    cache_write_tokens: None,
                    thinking_tokens: None,
                    cost_usd: None,
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
                    cached_tokens: None,
                    cache_write_tokens: None,
                    thinking_tokens: None,
                    cost_usd: None,
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
        task_ids: None,
    };
    assert_eq!(spec.agent_name, "Database Agent");
    assert_eq!(spec.agent_id, "uuid-123");
}

#[test]
fn test_parallel_task_spec_with_task_ids() {
    let spec = ParallelTaskSpec {
        agent_id: "uuid-123".to_string(),
        agent_name: "Database Agent".to_string(),
        prompt: "Analyze schema".to_string(),
        task_ids: Some(vec!["task_1".to_string(), "task_2".to_string()]),
    };

    let json = serde_json::to_string(&spec).unwrap();
    assert!(json.contains("task_ids"));
    assert!(json.contains("task_1"));
    assert!(json.contains("task_2"));
}

#[test]
fn test_definition_has_task_ids_property() {
    let schema = parallel_tasks_input_schema();
    let items = schema["properties"]["tasks"]["items"]["properties"]
        .as_object()
        .unwrap();
    assert!(
        items.contains_key("task_ids"),
        "Schema items must contain task_ids property"
    );
}
