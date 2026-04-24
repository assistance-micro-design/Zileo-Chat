use super::*;

#[test]
fn test_task_update_empty() {
    let update = TaskUpdate::default();
    assert!(update.name.is_none());
    assert!(update.description.is_none());
    assert!(update.priority.is_none());
    assert!(update.status.is_none());
    assert!(update.agent_assigned.is_none());
    assert!(update.dependencies.is_none());
    assert!(update.duration_ms.is_none());
}

#[tokio::test]
async fn test_task_create_serialization() {
    use crate::models::task::TaskCreate;

    let task = TaskCreate::new(
        "wf_001".to_string(),
        "Test task".to_string(),
        "A test task".to_string(),
        3,
    );

    let json = serde_json::to_string(&task).unwrap();
    assert!(json.contains("\"workflow_id\":\"wf_001\""));
    assert!(json.contains("\"name\":\"Test task\""));
    assert!(json.contains("\"priority\":3"));
    assert!(json.contains("\"status\":\"pending\""));
}

/// Verifies that task names containing apostrophes, backslashes, newlines,
/// and other special characters are stored and retrieved without corruption
/// or injection risk.
#[tokio::test]
async fn test_update_task_name_with_special_chars() {
    use crate::models::task::TaskCreate;

    let (state, _db_guard) = crate::test_utils::setup_test_state().await;

    // Create a workflow to satisfy the foreign key relationship
    let workflow_id = uuid::Uuid::new_v4().to_string();
    let wf_json = serde_json::json!({
        "id": workflow_id,
        "name": "Test Workflow",
        "status": "active",
        "agent_id": null,
        "created_at": chrono::Utc::now().to_rfc3339(),
    });
    state
        .db
        .execute_with_params(
            &format!("CREATE workflow:`{}` CONTENT $data", workflow_id),
            vec![("data".to_string(), wf_json)],
        )
        .await
        .expect("Failed to create test workflow");

    // Create the task
    let task_id = uuid::Uuid::new_v4().to_string();
    let task_create = TaskCreate::new(
        workflow_id.clone(),
        "Original name".to_string(),
        "Original description".to_string(),
        3,
    );
    state
        .db
        .create("task", &task_id, task_create)
        .await
        .expect("Failed to create test task");

    // Build an update with a name containing apostrophes, backslashes,
    // newlines and embedded quotes - the characters that broke the old
    // `replace('\'', "''")` approach.
    let tricky_name =
        "It's a \"tricky\" name\nwith backslash \\ and null-like \\0 chars".to_string();
    let tricky_description = "Line one\nLine two's \"quoted\" section \\ end".to_string();
    let tricky_agent = "agent'with\"quotes\\and\nnewline".to_string();

    let updates = TaskUpdate {
        name: Some(tricky_name.clone()),
        description: Some(tricky_description.clone()),
        agent_assigned: Some(tricky_agent.clone()),
        ..TaskUpdate::default()
    };

    // Simulate the SET-clause generation the same way update_task() does,
    // confirming that serde_json::to_string produces valid JSON literals
    // for each field.
    let mut set_parts: Vec<String> = Vec::new();

    let name_json = serde_json::to_string(&updates.name.as_ref().unwrap())
        .expect("name serialization must not fail");
    set_parts.push(format!("name = {}", name_json));

    let desc_json = serde_json::to_string(&updates.description.as_ref().unwrap())
        .expect("description serialization must not fail");
    set_parts.push(format!("description = {}", desc_json));

    let agent_json = serde_json::to_string(&updates.agent_assigned.as_ref().unwrap())
        .expect("agent_assigned serialization must not fail");
    set_parts.push(format!("agent_assigned = {}", agent_json));

    let query = format!("UPDATE task:`{}` SET {}", task_id, set_parts.join(", "));

    state
        .db
        .execute(&query)
        .await
        .expect("UPDATE with special chars must not fail");

    // Retrieve the task and assert values were stored verbatim
    let fetch_query = format!(
        "SELECT meta::id(id) AS id, name, description, agent_assigned FROM task WHERE meta::id(id) = '{}'",
        task_id
    );
    let rows: Vec<serde_json::Value> = state
        .db
        .query(&fetch_query)
        .await
        .expect("SELECT must succeed");

    assert_eq!(rows.len(), 1, "Expected exactly one task row");
    let row = &rows[0];

    assert_eq!(
        row["name"].as_str().unwrap_or(""),
        tricky_name,
        "name with special chars must round-trip correctly"
    );
    assert_eq!(
        row["description"].as_str().unwrap_or(""),
        tricky_description,
        "description with special chars must round-trip correctly"
    );
    assert_eq!(
        row["agent_assigned"].as_str().unwrap_or(""),
        tricky_agent,
        "agent_assigned with special chars must round-trip correctly"
    );
}
