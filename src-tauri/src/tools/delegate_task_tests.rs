use super::*;
use crate::models::sub_agent::{DelegateResult, SubAgentMetrics};
use crate::models::AgentConfig;
use crate::tools::delegate_task_execution::build_agent_listing_entry;

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
            cached_tokens: None,
            cache_write_tokens: None,
            thinking_tokens: None,
            cost_usd: None,
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

/// Builds an AgentConfig from a JSON literal using #[serde(default)] for
/// fields not specified. Keeps the test fixtures minimal.
fn make_agent_config(value: serde_json::Value) -> AgentConfig {
    serde_json::from_value(value).expect("test AgentConfig must be valid")
}

#[test]
fn test_list_agents_includes_folders_when_file_manager_present() {
    let config = make_agent_config(serde_json::json!({
        "id": "fs_agent",
        "name": "FS Agent",
        "lifecycle": "permanent",
        "tools": ["FileManagerTool"],
        "folders": ["/home/test/proj-a", "/home/test/proj-b"]
    }));

    let entry = build_agent_listing_entry("fs_agent", &config, vec!["filesystem".to_string()]);

    assert_eq!(entry["id"], serde_json::json!("fs_agent"));
    assert_eq!(entry["name"], serde_json::json!("FS Agent"));
    assert_eq!(entry["lifecycle"], serde_json::json!("permanent"));
    assert_eq!(entry["tools"], serde_json::json!(["FileManagerTool"]));
    assert_eq!(entry["has_file_manager"], serde_json::json!(true));
    assert_eq!(
        entry["folders"],
        serde_json::json!(["/home/test/proj-a", "/home/test/proj-b"])
    );
}

#[test]
fn test_list_agents_returns_empty_folders_when_no_file_manager() {
    // Degenerate config: folders were saved but FileManagerTool is not active.
    // The listing must hide the folders so the LLM does not see a misleading
    // capability contract.
    let config = make_agent_config(serde_json::json!({
        "id": "mem_agent",
        "name": "Memory-only Agent",
        "lifecycle": "permanent",
        "tools": ["MemoryTool"],
        "folders": ["/home/test/proj-a"]
    }));

    let entry = build_agent_listing_entry("mem_agent", &config, vec![]);

    assert_eq!(entry["has_file_manager"], serde_json::json!(false));
    assert_eq!(
        entry["folders"],
        serde_json::json!([]),
        "folders must be forced to [] when FileManagerTool is not in tools"
    );
}

#[test]
fn test_list_agents_returns_empty_folders_when_folders_unconfigured() {
    // FileManagerTool is present but the agent has no authorized folder.
    // The LLM must see the tool flag = true but folders empty, so it knows
    // the agent cannot perform file-bound tasks despite the tool being enabled.
    let config = make_agent_config(serde_json::json!({
        "id": "fs_empty",
        "name": "FS Agent (empty)",
        "lifecycle": "permanent",
        "tools": ["FileManagerTool"],
        "folders": []
    }));

    let entry = build_agent_listing_entry("fs_empty", &config, vec![]);

    assert_eq!(entry["has_file_manager"], serde_json::json!(true));
    assert_eq!(entry["folders"], serde_json::json!([]));
}
