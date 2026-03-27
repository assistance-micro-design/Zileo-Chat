use super::tool::TodoTool;
use crate::db::DBClient;
use crate::tools::{Tool, ToolDefinition, ToolError};
use std::sync::Arc;

#[test]
fn test_tool_definition() {
    let definition = ToolDefinition {
        id: "TodoTool".to_string(),
        name: "Todo Task Manager".to_string(),
        summary: "Manage a structured task list for complex workflows".to_string(),
        description: "Todo task manager tool for tests".to_string(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        requires_confirmation: false,
    };

    assert_eq!(definition.id, "TodoTool");
    assert!(!definition.requires_confirmation);
}

#[test]
fn test_input_validation_create() {
    let valid_input = serde_json::json!({
        "operation": "create",
        "name": "Test task",
        "description": "A test task",
        "priority": 2
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "create");
    assert!(valid_input.get("name").is_some());
}

#[test]
fn test_input_validation_update_status() {
    let valid_input = serde_json::json!({
        "operation": "update_status",
        "task_id": "task_001",
        "status": "in_progress"
    });

    assert!(valid_input.is_object());
    assert!(valid_input.get("task_id").is_some());
    assert!(valid_input.get("status").is_some());
}

#[test]
fn test_input_validation_list() {
    let valid_input = serde_json::json!({
        "operation": "list",
        "status_filter": "pending"
    });

    assert!(valid_input.is_object());
    assert_eq!(valid_input["operation"], "list");
}

#[test]
fn test_priority_values() {
    for p in 1..=5u8 {
        assert!((1..=5).contains(&p));
    }

    assert!(!(1..=5).contains(&0u8));
    assert!(!(1..=5).contains(&6u8));
}

#[test]
fn test_valid_statuses() {
    let valid_statuses = ["pending", "in_progress", "completed", "blocked"];

    assert!(valid_statuses.contains(&"pending"));
    assert!(valid_statuses.contains(&"in_progress"));
    assert!(valid_statuses.contains(&"completed"));
    assert!(valid_statuses.contains(&"blocked"));
    assert!(!valid_statuses.contains(&"done"));
    assert!(!valid_statuses.contains(&"started"));
}

// ==================== Integration Tests ====================

mod integration {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_tool() -> (TodoTool, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_todo_db");
        let db_path_str = db_path.to_str().unwrap().to_string();

        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = TodoTool::new(db, "wf_test".to_string(), "test_agent".to_string(), None);

        (tool, temp_dir)
    }

    #[tokio::test]
    async fn test_create_task_integration() {
        let (tool, _temp) = create_test_tool().await;

        let input = serde_json::json!({
            "operation": "create",
            "name": "Integration test task",
            "description": "Testing task creation with real DB",
            "priority": 2
        });

        let result = tool.execute(input).await;
        assert!(result.is_ok(), "Create task should succeed: {:?}", result);

        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert!(response["task_id"].is_string());
        assert!(!response["task_id"].as_str().unwrap().is_empty());
        assert!(response["message"]
            .as_str()
            .unwrap()
            .contains("created successfully"));
    }

    #[tokio::test]
    async fn test_update_status_integration() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Task to update",
            "description": "Will update status",
            "priority": 3
        });

        let create_result = tool
            .execute(create_input)
            .await
            .expect("Create should work");
        let task_id = create_result["task_id"].as_str().unwrap();

        let update_input = serde_json::json!({
            "operation": "update_status",
            "task_id": task_id,
            "status": "in_progress"
        });

        let update_result = tool.execute(update_input).await;
        assert!(
            update_result.is_ok(),
            "Update status should succeed: {:?}",
            update_result
        );

        let response = update_result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["task_id"], task_id);
        assert_eq!(response["new_status"], "in_progress");
    }

    #[tokio::test]
    async fn test_list_tasks_integration() {
        let (tool, _temp) = create_test_tool().await;

        for i in 1..=3 {
            let input = serde_json::json!({
                "operation": "create",
                "name": format!("List test task {}", i),
                "description": "For list testing",
                "priority": i
            });
            tool.execute(input).await.expect("Create should work");
        }

        let list_input = serde_json::json!({
            "operation": "list"
        });

        let list_result = tool.execute(list_input).await;
        assert!(
            list_result.is_ok(),
            "List tasks should succeed: {:?}",
            list_result
        );

        let response = list_result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["count"], 3);
        assert!(response["tasks"].is_array());
        assert_eq!(response["tasks"].as_array().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn test_list_tasks_with_filter_integration() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Pending task",
            "description": "Stays pending",
            "priority": 1
        });
        tool.execute(create_input)
            .await
            .expect("Create should work");

        let create_input2 = serde_json::json!({
            "operation": "create",
            "name": "In progress task",
            "description": "Will be in progress",
            "priority": 2
        });
        let result = tool
            .execute(create_input2)
            .await
            .expect("Create should work");
        let task_id = result["task_id"].as_str().unwrap();

        let update_input = serde_json::json!({
            "operation": "update_status",
            "task_id": task_id,
            "status": "in_progress"
        });
        tool.execute(update_input)
            .await
            .expect("Update should work");

        let list_pending = serde_json::json!({
            "operation": "list",
            "status_filter": "pending"
        });

        let result = tool.execute(list_pending).await.expect("List should work");
        assert_eq!(result["count"], 1);

        let list_in_progress = serde_json::json!({
            "operation": "list",
            "status_filter": "in_progress"
        });

        let result = tool
            .execute(list_in_progress)
            .await
            .expect("List should work");
        assert_eq!(result["count"], 1);
    }

    #[tokio::test]
    async fn test_complete_task_integration() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Task to complete",
            "description": "Will be completed",
            "priority": 1
        });

        let create_result = tool
            .execute(create_input)
            .await
            .expect("Create should work");
        let task_id = create_result["task_id"].as_str().unwrap();

        let complete_input = serde_json::json!({
            "operation": "complete",
            "task_id": task_id,
            "duration_ms": 5000
        });

        let complete_result = tool.execute(complete_input).await;
        assert!(
            complete_result.is_ok(),
            "Complete task should succeed: {:?}",
            complete_result
        );

        let response = complete_result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["task_id"], task_id);
        assert_eq!(response["status"], "completed");
        assert_eq!(response["duration_ms"], 5000);
    }

    #[tokio::test]
    async fn test_complete_task_without_duration_integration() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Task to complete no duration",
            "description": "Completed without duration",
            "priority": 2
        });

        let create_result = tool
            .execute(create_input)
            .await
            .expect("Create should work");
        let task_id = create_result["task_id"].as_str().unwrap();

        let complete_input = serde_json::json!({
            "operation": "complete",
            "task_id": task_id
        });

        let complete_result = tool.execute(complete_input).await;
        assert!(
            complete_result.is_ok(),
            "Complete task should succeed: {:?}",
            complete_result
        );

        let response = complete_result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["status"], "completed");
        assert!(response["duration_ms"].is_null());
    }

    #[tokio::test]
    async fn test_delete_task_integration() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Task to delete",
            "description": "Will be deleted",
            "priority": 3
        });

        let create_result = tool
            .execute(create_input)
            .await
            .expect("Create should work");
        let task_id = create_result["task_id"].as_str().unwrap();

        let delete_input = serde_json::json!({
            "operation": "delete",
            "task_id": task_id
        });

        let delete_result = tool.execute(delete_input).await;
        assert!(
            delete_result.is_ok(),
            "Delete task should succeed: {:?}",
            delete_result
        );

        let response = delete_result.unwrap();
        assert_eq!(response["success"], true);
        assert!(response["message"]
            .as_str()
            .unwrap()
            .contains("deleted successfully"));

        let get_input = serde_json::json!({
            "operation": "get",
            "task_id": task_id
        });

        let get_result = tool.execute(get_input).await;
        assert!(get_result.is_err(), "Get deleted task should fail");
        match get_result {
            Err(ToolError::NotFound(_)) => {}
            _ => panic!("Expected NotFound error"),
        }
    }

    #[tokio::test]
    async fn test_get_task_not_found() {
        let (tool, _temp) = create_test_tool().await;

        let get_input = serde_json::json!({
            "operation": "get",
            "task_id": "non-existent-task-id-12345"
        });

        let result = tool.execute(get_input).await;
        assert!(result.is_err(), "Get non-existent task should fail");

        match result {
            Err(ToolError::NotFound(msg)) => {
                assert!(msg.contains("non-existent-task-id-12345"));
                assert!(msg.contains("does not exist"));
            }
            other => panic!("Expected NotFound error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_task_success_integration() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Task to retrieve",
            "description": "Testing get operation",
            "priority": 2
        });

        let create_result = tool
            .execute(create_input)
            .await
            .expect("Create should work");
        let task_id = create_result["task_id"].as_str().unwrap();

        let get_input = serde_json::json!({
            "operation": "get",
            "task_id": task_id
        });

        let get_result = tool.execute(get_input).await;
        assert!(
            get_result.is_ok(),
            "Get task should succeed: {:?}",
            get_result
        );

        let response = get_result.unwrap();
        assert_eq!(response["success"], true);
        assert!(response["task"].is_object());
        assert_eq!(response["task"]["name"], "Task to retrieve");
        assert_eq!(response["task"]["status"], "pending");
        assert_eq!(response["task"]["priority"], 2);
    }

    #[tokio::test]
    async fn test_update_status_not_found() {
        let (tool, _temp) = create_test_tool().await;

        let update_input = serde_json::json!({
            "operation": "update_status",
            "task_id": "non-existent-task-456",
            "status": "in_progress"
        });

        let result = tool.execute(update_input).await;
        assert!(result.is_err(), "Update non-existent task should fail");

        match result {
            Err(ToolError::NotFound(msg)) => {
                assert!(msg.contains("non-existent-task-456"));
                assert!(msg.contains("not found"));
            }
            other => panic!("Expected NotFound error, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_complete_task_not_found() {
        let (tool, _temp) = create_test_tool().await;

        let complete_input = serde_json::json!({
            "operation": "complete",
            "task_id": "non-existent-task-789"
        });

        let result = tool.execute(complete_input).await;
        assert!(result.is_err(), "Complete non-existent task should fail");

        match result {
            Err(ToolError::NotFound(msg)) => {
                assert!(msg.contains("non-existent-task-789"));
                assert!(msg.contains("not found"));
            }
            other => panic!("Expected NotFound error, got: {:?}", other),
        }
    }
}

// ==================== SQL Injection Tests ====================

mod sql_injection {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_tool() -> (TodoTool, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_injection_db");
        let db_path_str = db_path.to_str().unwrap().to_string();

        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = TodoTool::new(db, "wf_test".to_string(), "test_agent".to_string(), None);

        (tool, temp_dir)
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_task_id_get() {
        let (tool, _temp) = create_test_tool().await;

        let malicious_input = serde_json::json!({
            "operation": "get",
            "task_id": "'; DROP TABLE task; --"
        });

        let result = tool.execute(malicious_input).await;

        assert!(result.is_err(), "Injection should not succeed");
        match result {
            Err(ToolError::NotFound(_)) => {}
            other => panic!(
                "Expected NotFound error for injection attempt, got: {:?}",
                other
            ),
        }

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "After injection attempt",
            "description": "Table should still exist",
            "priority": 1
        });

        let create_result = tool.execute(create_input).await;
        assert!(
            create_result.is_ok(),
            "Table should still exist after injection attempt"
        );
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_task_id_update() {
        let (tool, _temp) = create_test_tool().await;

        let malicious_input = serde_json::json!({
            "operation": "update_status",
            "task_id": "' OR '1'='1",
            "status": "completed"
        });

        let result = tool.execute(malicious_input).await;

        assert!(result.is_err(), "Injection should not succeed");
        match result {
            Err(ToolError::NotFound(_)) => {}
            other => panic!(
                "Expected NotFound error for injection attempt, got: {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_task_id_complete() {
        let (tool, _temp) = create_test_tool().await;

        let malicious_input = serde_json::json!({
            "operation": "complete",
            "task_id": "1; UPDATE task SET status = 'hacked';"
        });

        let result = tool.execute(malicious_input).await;

        assert!(result.is_err(), "Injection should not succeed");
        match result {
            Err(ToolError::NotFound(_)) => {}
            other => panic!(
                "Expected NotFound error for injection attempt, got: {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_status() {
        let (tool, _temp) = create_test_tool().await;

        let malicious_input = serde_json::json!({
            "operation": "update_status",
            "task_id": "some-task-id",
            "status": "pending' OR '1'='1"
        });

        let result = tool.execute(malicious_input).await;

        assert!(result.is_err(), "Injection should not succeed");
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Invalid"));
            }
            other => panic!(
                "Expected ValidationFailed error for injection attempt, got: {:?}",
                other
            ),
        }
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_status_filter() {
        let (tool, _temp) = create_test_tool().await;

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Legitimate task",
            "description": "For filter test",
            "priority": 2
        });
        tool.execute(create_input)
            .await
            .expect("Create should work");

        let malicious_input = serde_json::json!({
            "operation": "list",
            "status_filter": "pending' OR '1'='1"
        });

        let result = tool.execute(malicious_input).await;

        assert!(result.is_ok(), "Query should succeed but return 0 results");
        let response = result.unwrap();
        assert_eq!(
            response["count"], 0,
            "Injection should not return all tasks"
        );
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_name() {
        let (tool, _temp) = create_test_tool().await;

        let malicious_input = serde_json::json!({
            "operation": "create",
            "name": "Test'; DROP TABLE task; --",
            "description": "Malicious description",
            "priority": 1
        });

        let result = tool.execute(malicious_input).await;

        assert!(
            result.is_ok(),
            "Create should succeed with escaped name: {:?}",
            result
        );

        let list_input = serde_json::json!({
            "operation": "list"
        });
        let list_result = tool.execute(list_input).await;
        assert!(list_result.is_ok(), "Table should still exist");
        assert_eq!(list_result.unwrap()["count"], 1);
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_description() {
        let (tool, _temp) = create_test_tool().await;

        let malicious_input = serde_json::json!({
            "operation": "create",
            "name": "Normal task name",
            "description": "'; DELETE FROM task; SELECT '",
            "priority": 1
        });

        let result = tool.execute(malicious_input).await;

        assert!(result.is_ok(), "Create should succeed: {:?}", result);

        let list_input = serde_json::json!({
            "operation": "list"
        });
        let list_result = tool.execute(list_input).await.unwrap();
        assert_eq!(
            list_result["count"], 1,
            "Task should exist, no deletion occurred"
        );
    }

    #[tokio::test]
    async fn test_sql_injection_prevention_workflow_id() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_wf_injection_db");
        let db_path_str = db_path.to_str().unwrap().to_string();

        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = TodoTool::new(
            db,
            "wf_test' OR '1'='1".to_string(),
            "test_agent".to_string(),
            None,
        );

        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Test with malicious workflow",
            "description": "Should be isolated",
            "priority": 1
        });

        let result = tool.execute(create_input).await;
        assert!(result.is_ok(), "Create should succeed");

        let list_input = serde_json::json!({
            "operation": "list"
        });

        let list_result = tool.execute(list_input).await.unwrap();
        assert_eq!(list_result["count"], 1);
    }
}
