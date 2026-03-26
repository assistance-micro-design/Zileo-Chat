use super::*;
use crate::models::sub_agent::{constants::MAX_SUB_AGENTS, DelegateResult, SubAgentMetrics};

#[test]
fn test_tool_definition() {
    let definition = ToolDefinition {
        id: "DelegateTaskTool".to_string(),
        name: "Delegate Task".to_string(),
        description: "Test".to_string(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        requires_confirmation: false,
    };

    assert_eq!(definition.id, "DelegateTaskTool");
    assert!(!definition.requires_confirmation);
}

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
fn test_input_validation_delegate() {
    let valid_input = serde_json::json!({
        "operation": "delegate",
        "agent_id": "db_agent",
        "prompt": "Analyze the database schema"
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "delegate");
    assert!(valid_input.get("agent_id").is_some());
    assert!(valid_input.get("prompt").is_some());
}

#[test]
fn test_input_validation_list() {
    let valid_input = serde_json::json!({
        "operation": "list_agents"
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "list_agents");
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
fn test_max_sub_agents_constant() {
    assert_eq!(MAX_SUB_AGENTS, 15);
}

// --- DelegateTaskTool accepts agent_name ---

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
