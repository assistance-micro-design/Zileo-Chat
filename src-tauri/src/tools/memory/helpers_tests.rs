use super::*;

#[test]
fn test_build_scope_condition_workflow() {
    let mut params = Vec::new();
    let wf = Some("wf_001".to_string());
    let cond = build_scope_condition("workflow", &wf, &mut params);
    assert_eq!(cond, Some("workflow_id = $workflow_id".to_string()));
    assert_eq!(params.len(), 1);
}

#[test]
fn test_build_scope_condition_general() {
    let mut params = Vec::new();
    let wf = Some("wf_001".to_string());
    let cond = build_scope_condition("general", &wf, &mut params);
    assert_eq!(cond, Some("workflow_id IS NONE".to_string()));
    assert!(params.is_empty());
}

#[test]
fn test_build_scope_condition_both() {
    let mut params = Vec::new();
    let wf = Some("wf_001".to_string());
    let cond = build_scope_condition("both", &wf, &mut params);
    assert!(cond.is_some());
    let cond_str = cond.unwrap();
    assert!(cond_str.contains("workflow_id = $workflow_id"));
    assert!(cond_str.contains("workflow_id IS NONE"));
}

#[test]
fn test_build_scope_condition_workflow_no_id() {
    let mut params = Vec::new();
    let wf: Option<String> = None;
    let cond = build_scope_condition("workflow", &wf, &mut params);
    assert!(cond.is_none());
}

#[test]
fn test_expiration_filter_format() {
    let filter = expiration_filter();
    assert!(filter.contains("expires_at IS NONE"));
    assert!(filter.contains("time::now()"));
}
