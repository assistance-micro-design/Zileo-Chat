use super::tool::TodoTool;
use crate::db::DBClient;
use crate::tools::{Tool, ToolError};
use std::sync::Arc;

mod integration {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_tool() -> (TodoTool, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_todo_db");
        let db_path_str = db_path.to_str().unwrap().to_string();

        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "test_agent".to_string(),
            None,
            true,
        );

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
    #[tokio::test]
    async fn test_list_agent_tasks_requires_primary() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_agent_tasks_db");
        let db_path_str = db_path.to_str().unwrap().to_string();
        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Sub-agent (is_primary_agent = false)
        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "sub_agent".to_string(),
            None,
            false,
        );

        let input = serde_json::json!({
            "operation": "list_agent_tasks",
            "agent_id": "other_agent"
        });

        let result = tool.execute(input).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::PermissionDenied(msg)) => {
                assert!(msg.contains("primary agent"));
            }
            other => panic!("Expected PermissionDenied, got: {:?}", other),
        }
    }

    /// Helper to create a minimal agent record in DB for resolve_agent_ref tests.
    async fn create_agent_record(db: &DBClient, agent_id: &str, name: &str) {
        db.execute_with_params(
            &format!(
                "CREATE agent:`{}` SET \
                 name = $name, \
                 lifecycle = 'permanent', \
                 system_prompt = 'test', \
                 tools = [], \
                 mcp_servers = [], \
                 skills = [], \
                 folders = [], \
                 llm = {{ provider: 'ollama', model: 'test', temperature: 0.7, max_tokens: 1000 }}, \
                 max_tool_iterations = 10",
                agent_id
            ),
            vec![("name".to_string(), serde_json::json!(name))],
        )
        .await
        .expect("Create agent record should work");
    }

    #[tokio::test]
    async fn test_list_agent_tasks_primary_succeeds() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_list_agent_db");
        let db_path_str = db_path.to_str().unwrap().to_string();
        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Create agent record so resolve_agent_ref works
        create_agent_record(&db, "test_agent", "Test Agent").await;

        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "test_agent".to_string(),
            None,
            true,
        );

        // Create a task assigned to test_agent
        let create_input = serde_json::json!({
            "operation": "create",
            "name": "Agent task",
            "description": "Assigned to test_agent",
            "priority": 1
        });
        tool.execute(create_input)
            .await
            .expect("Create should work");

        // Lookup by ID
        let input = serde_json::json!({
            "operation": "list_agent_tasks",
            "agent_id": "test_agent"
        });

        let result = tool.execute(input).await;
        assert!(
            result.is_ok(),
            "list_agent_tasks should succeed: {:?}",
            result
        );

        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["agent_id"], "test_agent");
        assert_eq!(response["total"], 1);

        // Lookup by name
        let input_by_name = serde_json::json!({
            "operation": "list_agent_tasks",
            "agent_name": "Test Agent"
        });

        let result_by_name = tool.execute(input_by_name).await;
        assert!(
            result_by_name.is_ok(),
            "list_agent_tasks by name should succeed: {:?}",
            result_by_name
        );

        let response_by_name = result_by_name.unwrap();
        assert_eq!(response_by_name["total"], 1);
    }

    #[tokio::test]
    async fn test_reassign_tasks_requires_primary() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_reassign_perm_db");
        let db_path_str = db_path.to_str().unwrap().to_string();
        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "sub_agent".to_string(),
            None,
            false,
        );

        let input = serde_json::json!({
            "operation": "reassign_tasks",
            "task_ids": ["task_1"],
            "new_agent_id": "other_agent"
        });

        let result = tool.execute(input).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::PermissionDenied(msg)) => {
                assert!(msg.contains("primary agent"));
            }
            other => panic!("Expected PermissionDenied, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reassign_tasks_validates_empty_ids() {
        let (tool, _temp) = create_test_tool().await;

        let input = serde_json::json!({
            "operation": "reassign_tasks",
            "task_ids": [],
            "new_agent_id": "other_agent"
        });

        let result = tool.execute(input).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("empty"));
            }
            other => panic!("Expected InvalidInput, got: {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reassign_tasks_succeeds() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_reassign_db");
        let db_path_str = db_path.to_str().unwrap().to_string();
        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Create agent records so resolve_agent_ref works
        create_agent_record(&db, "test_agent", "Test Agent").await;
        create_agent_record(&db, "new_agent", "New Agent").await;

        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "test_agent".to_string(),
            None,
            true,
        );

        // Create a task
        let create_result = tool
            .execute(serde_json::json!({
                "operation": "create",
                "name": "Task to reassign",
                "priority": 1
            }))
            .await
            .expect("Create should work");
        let task_id = create_result["task_id"].as_str().unwrap().to_string();

        // Reassign by ID
        let input = serde_json::json!({
            "operation": "reassign_tasks",
            "task_ids": [task_id],
            "new_agent_id": "new_agent"
        });

        let result = tool.execute(input).await;
        assert!(result.is_ok(), "reassign should succeed: {:?}", result);

        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["reassigned_count"], 1);
        assert_eq!(response["new_agent_id"], "new_agent");
    }

    #[tokio::test]
    async fn test_subagent_only_sees_own_tasks() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_scoping_db");
        let db_path_str = db_path.to_str().unwrap().to_string();
        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Primary creates tasks assigned to different agents
        let primary = TodoTool::new(
            db.clone(),
            "wf_test".to_string(),
            "primary_agent".to_string(),
            None,
            true,
        );

        primary
            .execute(serde_json::json!({
                "operation": "create",
                "name": "Primary task",
                "priority": 1
            }))
            .await
            .expect("Create should work");

        // Sub-agent creates a task (auto-assigned to sub_agent)
        let sub = TodoTool::new(
            db.clone(),
            "wf_test".to_string(),
            "sub_agent".to_string(),
            None,
            false,
        );

        sub.execute(serde_json::json!({
            "operation": "create",
            "name": "Sub task",
            "priority": 2
        }))
        .await
        .expect("Create should work");

        // Primary sees all tasks
        let primary_list = primary
            .execute(serde_json::json!({"operation": "list"}))
            .await
            .unwrap();
        assert_eq!(primary_list["count"], 2, "Primary should see all tasks");

        // Sub-agent only sees its own task
        let sub_list = sub
            .execute(serde_json::json!({"operation": "list"}))
            .await
            .unwrap();
        assert_eq!(sub_list["count"], 1, "Sub-agent should only see own tasks");

        let task_name = sub_list["tasks"][0]["name"].as_str().unwrap();
        assert_eq!(task_name, "Sub task");
    }
}

mod sql_injection {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_tool() -> (TodoTool, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_injection_db");
        let db_path_str = db_path.to_str().unwrap().to_string();

        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "test_agent".to_string(),
            None,
            true,
        );

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
            true,
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
