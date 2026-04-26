use super::*;
use crate::models::sub_agent::constants::MAX_SUB_AGENTS;

#[test]
fn test_tool_definition() {
    // Verify definition has required fields
    let definition = ToolDefinition {
        id: "SpawnAgentTool".to_string(),
        name: "Spawn Sub-Agent".to_string(),
        summary: "Spawn a sub-agent for complex or specialized tasks".to_string(),
        description: "Spawn agent tool for tests".to_string(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        requires_confirmation: false,
    };

    assert_eq!(definition.id, "SpawnAgentTool");
    assert!(!definition.requires_confirmation);
}

#[test]
fn test_spawned_child_serialization() {
    let child = SpawnedChild {
        id: "sub_123".to_string(),
        name: "Test Agent".to_string(),
        task_description: "Analyze something".to_string(),
        status: SubAgentStatus::Running,
        execution_id: "exec_456".to_string(),
    };

    let json = serde_json::to_string(&child).unwrap();
    assert!(json.contains("sub_123"));
    assert!(json.contains("Test Agent"));
    assert!(json.contains("running"));
}

#[test]
fn test_input_validation_spawn() {
    let valid_input = serde_json::json!({
        "operation": "spawn",
        "name": "AnalysisAgent",
        "prompt": "Analyze the code for bugs"
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "spawn");
    assert!(valid_input.get("name").is_some());
    assert!(valid_input.get("prompt").is_some());
}

#[test]
fn test_input_validation_terminate() {
    let valid_input = serde_json::json!({
        "operation": "terminate",
        "child_id": "sub_abc123"
    });

    assert!(valid_input.is_object());
    assert!(valid_input.get("child_id").is_some());
}

#[test]
fn test_input_validation_list() {
    let valid_input = serde_json::json!({
        "operation": "list_children"
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "list_children");
}

#[test]
fn test_max_sub_agents_constant() {
    assert_eq!(MAX_SUB_AGENTS, 15);
}

#[test]
fn test_default_system_prompt() {
    // Verify the default system prompt has meaningful content
    assert!(DEFAULT_SUB_AGENT_SYSTEM_PROMPT.len() > 50);
    assert!(DEFAULT_SUB_AGENT_SYSTEM_PROMPT.contains("sub-agent"));
}

/// Two consecutive accesses to `DEFINITION` must yield identical content.
///
/// This guarantees a stable cache prefix for LLM providers. The previous
/// implementation reformatted the "Available tools for sub-agents" line on
/// every `definition()` call, breaking the cache.
#[test]
fn test_definition_is_stable_across_calls() {
    let def_a = DEFINITION.clone();
    let def_b = DEFINITION.clone();
    assert_eq!(def_a.id, def_b.id);
    assert_eq!(def_a.name, def_b.name);
    assert_eq!(def_a.summary, def_b.summary);
    assert_eq!(def_a.description, def_b.description);
    assert_eq!(def_a.input_schema, def_b.input_schema);
    assert_eq!(def_a.output_schema, def_b.output_schema);
}

#[test]
fn test_definition_includes_primary_agent_constraint() {
    let def = DEFINITION.clone();
    assert!(def.description.contains("PRIMARY AGENT ONLY:"));
    assert!(def
        .description
        .contains(&format!("Maximum {} sub-agent operations", MAX_SUB_AGENTS)));
}

#[test]
fn test_definition_includes_basic_tools_listing() {
    let def = DEFINITION.clone();
    assert!(def.description.contains("Available tools for sub-agents:"));
    // basic_tools() returns Memory/Todo/Calculator/UserQuestion/FileManager.
    assert!(def.description.contains("MemoryTool"));
}
