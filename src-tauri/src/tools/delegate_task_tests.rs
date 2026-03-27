use super::*;
use crate::models::sub_agent::{DelegateResult, SubAgentMetrics};

#[test]
fn test_active_delegation_serialization() {
    let delegation = ActiveDelegation {
        agent_id: "db_agent".to_string(),
        agent_name: "Database Agent".to_string(),
        task_description: "Analyze schema".to_string(),
        status: SubAgentStatus::Running,
        execution_id: "exec_456".to_string(),
    };

    let json = serde_json::to_string(&delegation).unwrap();
    assert!(json.contains("db_agent"));
    assert!(json.contains("Database Agent"));
    assert!(json.contains("running"));
}

#[test]
fn test_delegate_result_serialization() {
    let result = DelegateResult {
        success: true,
        agent_id: "db_agent".to_string(),
        report: "# Analysis Complete\n\nFound 3 optimization opportunities.".to_string(),
        metrics: SubAgentMetrics {
            duration_ms: 1500,
            tokens_input: 200,
            tokens_output: 400,
        },
    };

    let json = serde_json::to_string(&result).unwrap();
    assert!(json.contains("\"success\":true"));
    assert!(json.contains("\"agent_id\":\"db_agent\""));
    assert!(json.contains("\"duration_ms\":1500"));
}

#[test]
fn test_validate_input_accepts_agent_id() {
    let input = serde_json::json!({
        "operation": "delegate",
        "agent_id": "some-uuid-123",
        "prompt": "Analyze the database"
    });
    let result = validate_delegate_operation(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_input_accepts_agent_name() {
    let input = serde_json::json!({
        "operation": "delegate",
        "agent_name": "Database Agent",
        "prompt": "Analyze the database"
    });
    let result = validate_delegate_operation(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_input_rejects_missing_both() {
    let input = serde_json::json!({
        "operation": "delegate",
        "prompt": "Analyze the database"
    });
    let result = validate_delegate_operation(&input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[test]
fn test_definition_has_agent_name_property() {
    let schema = delegate_task_input_schema();
    let properties = schema["properties"].as_object().unwrap();
    assert!(
        properties.contains_key("agent_name"),
        "Schema must contain agent_name property"
    );
    assert!(
        properties.contains_key("agent_id"),
        "Schema must still contain agent_id property"
    );
}

#[test]
fn test_definition_has_task_ids_property() {
    let schema = delegate_task_input_schema();
    let properties = schema["properties"].as_object().unwrap();
    assert!(
        properties.contains_key("task_ids"),
        "Schema must contain task_ids property"
    );
}

#[test]
fn test_validate_delegate_with_task_ids() {
    let input = serde_json::json!({
        "operation": "delegate",
        "agent_name": "DB Agent",
        "prompt": "Analyze",
        "task_ids": ["task_1", "task_2"]
    });
    let result = validate_delegate_operation(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_delegate_empty_task_ids_error() {
    let input = serde_json::json!({
        "operation": "delegate",
        "agent_name": "DB Agent",
        "prompt": "Analyze",
        "task_ids": []
    });
    let result = validate_delegate_operation(&input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[test]
fn test_validate_delegate_without_task_ids_ok() {
    let input = serde_json::json!({
        "operation": "delegate",
        "agent_name": "DB Agent",
        "prompt": "Analyze"
    });
    let result = validate_delegate_operation(&input);
    assert!(
        result.is_ok(),
        "delegate without task_ids should still work"
    );
}
