use super::*;

#[test]
fn test_parse_memory_type_valid() {
    assert!(matches!(
        parse_memory_type("user_pref"),
        Ok(MemoryType::UserPref)
    ));
    assert!(matches!(
        parse_memory_type("context"),
        Ok(MemoryType::Context)
    ));
    assert!(matches!(
        parse_memory_type("knowledge"),
        Ok(MemoryType::Knowledge)
    ));
    assert!(matches!(
        parse_memory_type("decision"),
        Ok(MemoryType::Decision)
    ));
}

#[test]
fn test_parse_memory_type_invalid() {
    let result = parse_memory_type("invalid");
    assert!(result.is_err());
    match result {
        Err(ToolError::ValidationFailed(msg)) => assert!(msg.contains("Invalid memory type")),
        _ => panic!("Expected ValidationFailed error"),
    }
}

#[test]
fn test_resolve_storage_scope_auto_general() {
    let input = MemoryInput::from_json(&serde_json::json!({
        "operation": "add",
        "type": "knowledge",
        "content": "test"
    }))
    .unwrap();
    let wf = Some("wf_001".to_string());

    // knowledge -> general (None)
    assert!(resolve_storage_scope("knowledge", &input, &wf).is_none());
    assert!(resolve_storage_scope("user_pref", &input, &wf).is_none());
}

#[test]
fn test_resolve_storage_scope_auto_workflow() {
    let input = MemoryInput::from_json(&serde_json::json!({
        "operation": "add",
        "type": "context",
        "content": "test"
    }))
    .unwrap();
    let wf = Some("wf_001".to_string());

    // context, decision -> workflow-scoped
    assert_eq!(
        resolve_storage_scope("context", &input, &wf),
        Some("wf_001".to_string())
    );
    assert_eq!(
        resolve_storage_scope("decision", &input, &wf),
        Some("wf_001".to_string())
    );
}

#[test]
fn test_resolve_storage_scope_explicit_override() {
    let input = MemoryInput::from_json(&serde_json::json!({
        "operation": "add",
        "type": "decision",
        "content": "test",
        "scope": "general"
    }))
    .unwrap();
    let wf = Some("wf_001".to_string());

    // explicit "general" overrides auto-scope
    assert!(resolve_storage_scope("decision", &input, &wf).is_none());
}

#[test]
fn test_resolve_query_workflow_id_explicit_override() {
    let input = MemoryInput::from_json(&serde_json::json!({
        "operation": "list",
        "workflow_id": "explicit_wf"
    }))
    .unwrap();
    let default = Some("default_wf".to_string());

    assert_eq!(
        resolve_query_workflow_id(&input, &default),
        Some("explicit_wf".to_string())
    );
}

#[test]
fn test_resolve_query_workflow_id_falls_back_to_default() {
    let input = MemoryInput::from_json(&serde_json::json!({
        "operation": "list"
    }))
    .unwrap();
    let default = Some("default_wf".to_string());

    assert_eq!(
        resolve_query_workflow_id(&input, &default),
        Some("default_wf".to_string())
    );
}

#[test]
fn test_default_importance_for_type() {
    assert!((default_importance_for_type("user_pref") - 0.8).abs() < f64::EPSILON);
    assert!((default_importance_for_type("decision") - 0.7).abs() < f64::EPSILON);
    assert!((default_importance_for_type("knowledge") - 0.6).abs() < f64::EPSILON);
    assert!((default_importance_for_type("context") - 0.3).abs() < f64::EPSILON);
    assert!((default_importance_for_type("unknown") - 0.5).abs() < f64::EPSILON);
}

#[test]
fn test_default_expires_at_for_type() {
    // context -> Some (7-day TTL)
    assert!(default_expires_at_for_type("context").is_some());
    // others -> None
    assert!(default_expires_at_for_type("knowledge").is_none());
    assert!(default_expires_at_for_type("user_pref").is_none());
    assert!(default_expires_at_for_type("decision").is_none());
}
