// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for MemoryTool Phase 4 Integration.
//!
//! These tests verify that MemoryTool works correctly when integrated
//! with the agent system, ToolFactory, and AppState.

use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;

// Note: Integration tests run as separate crates, so we need to import from the main crate
// The crate name is "zileo_chat" as defined in Cargo.toml (hyphens become underscores)

/// Helper to create test database path
fn create_test_db_path() -> (tempfile::TempDir, String) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("integration_test_db");
    let path_str = db_path.to_str().unwrap().to_string();
    (temp_dir, path_str)
}

#[cfg(test)]
mod tool_factory_tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_factory_creates_memory_tool() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::ToolFactory;

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Create embedding service reference (None = no embedding service)
        let embedding_service = Arc::new(RwLock::new(None));
        let factory = ToolFactory::new(db, embedding_service);

        // Create MemoryTool via factory
        let result = factory
            .create_tool(
                "MemoryTool",
                Some("wf_integration_test".to_string()),
                "integration_test_agent".to_string(),
                None,
            )
            .await;

        assert!(result.is_ok(), "Should create MemoryTool");

        let tool = result.unwrap();
        assert_eq!(tool.definition().id, "MemoryTool");
        assert_eq!(tool.definition().name, "Memory Manager");
        assert!(!tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_tool_factory_creates_todo_tool() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::ToolFactory;

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let embedding_service = Arc::new(RwLock::new(None));
        let factory = ToolFactory::new(db, embedding_service);

        // Create TodoTool via factory
        let result = factory
            .create_tool(
                "TodoTool",
                Some("wf_integration_test".to_string()),
                "integration_test_agent".to_string(),
                None,
            )
            .await;

        assert!(result.is_ok(), "Should create TodoTool");

        let tool = result.unwrap();
        assert_eq!(tool.definition().id, "TodoTool");
    }

    #[tokio::test]
    async fn test_tool_factory_batch_creation() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::ToolFactory;

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let embedding_service = Arc::new(RwLock::new(None));
        let factory = ToolFactory::new(db, embedding_service);

        let tool_names = vec![
            "MemoryTool".to_string(),
            "TodoTool".to_string(),
            "UnknownTool".to_string(), // Should be skipped
        ];

        let tools = factory
            .create_tools(
                &tool_names,
                Some("wf_batch".to_string()),
                "batch_agent".to_string(),
                None, // app_handle not needed in tests
            )
            .await;

        // Should create 2 valid tools, skip 1 invalid
        assert_eq!(tools.len(), 2);
    }
}

#[cfg(test)]
mod memory_tool_operations_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_memory_tool_add_and_list() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Create tool without embedding service (text-only mode)
        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        // Add a memory
        let add_result = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "Integration test memory content"
            }))
            .await;

        assert!(add_result.is_ok(), "Add operation should succeed");
        let add_response = add_result.unwrap();
        assert_eq!(add_response["success"], true);
        assert!(add_response["memory_id"].is_string());
        assert_eq!(add_response["embedding_generated"], false); // No embedding service
                                                                // knowledge auto-scopes to general (null workflow_id)
        assert!(add_response["workflow_id"].is_null());

        // List memories
        let list_result = tool.execute(json!({"operation": "list"})).await;

        assert!(list_result.is_ok(), "List operation should succeed");
        let list_response = list_result.unwrap();
        assert_eq!(list_response["success"], true);
        assert!(list_response["count"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn test_memory_tool_auto_scoping() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_auto_scope".to_string()),
            "test_agent".to_string(),
        );

        // Add user_pref (auto-scoped to general)
        let result = tool
            .execute(json!({
                "operation": "add",
                "type": "user_pref",
                "content": "User prefers dark mode"
            }))
            .await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        // user_pref should have null workflow_id (general)
        assert!(response["workflow_id"].is_null());

        // Add context (auto-scoped to workflow)
        let result = tool
            .execute(json!({
                "operation": "add",
                "type": "context",
                "content": "Working on database migration"
            }))
            .await;
        assert!(result.is_ok(), "Add context failed: {:?}", result.err());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        // context should have workflow_id set
        assert_eq!(response["workflow_id"], "wf_auto_scope");
    }

    #[tokio::test]
    async fn test_memory_tool_text_search_fallback() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Add some memories
        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "Rust programming language"
            }))
            .await
            .expect("Add failed");

        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "Python scripting"
            }))
            .await
            .expect("Add failed");

        // Search (will use text fallback since no embedding service)
        let result = tool
            .execute(json!({
                "operation": "search",
                "query": "Rust",
                "limit": 5
            }))
            .await;

        assert!(result.is_ok(), "Search should succeed");
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["search_type"], "text"); // Fallback to text search
    }

    #[tokio::test]
    async fn test_memory_tool_validation() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Test invalid operation
        let result = tool.execute(json!({"operation": "invalid_op"})).await;
        assert!(result.is_err(), "Invalid operation should fail");

        // Test missing required fields
        let result = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge"
                // Missing content
            }))
            .await;
        assert!(result.is_err(), "Missing content should fail");

        // Test invalid memory type
        let result = tool
            .execute(json!({
                "operation": "add",
                "type": "invalid_type",
                "content": "test"
            }))
            .await;
        assert!(result.is_err(), "Invalid type should fail");
    }
}

#[cfg(test)]
mod agent_config_tests {
    #[test]
    fn test_agent_config_with_memory_tool() {
        use zileo_chat::models::{AgentConfig, LLMConfig, Lifecycle};

        let config = AgentConfig {
            id: "memory_agent".to_string(),
            name: "Memory Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
            },
            tools: vec!["MemoryTool".to_string(), "TodoTool".to_string()],
            mcp_servers: vec![],
            system_prompt: "You are an agent with memory capabilities.".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        };

        assert!(config.has_valid_tools());
        assert!(config.validate_tools().is_empty());
        assert!(config.tools.contains(&"MemoryTool".to_string()));
    }
}

/// Additional integration tests for complete CRUD operations and workflow isolation.
#[cfg(test)]
mod memory_crud_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_memory_get_by_id() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        // Add a memory
        let add_result = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "Test memory for retrieval"
            }))
            .await
            .expect("Add should succeed");

        let memory_id = add_result["memory_id"].as_str().unwrap();

        // Get the memory by ID
        let get_result = tool
            .execute(json!({
                "operation": "get",
                "memory_id": memory_id
            }))
            .await;

        assert!(get_result.is_ok(), "Get operation should succeed");
        let response = get_result.unwrap();
        assert_eq!(response["success"], true);
        assert!(response["memory"].is_object());
        assert_eq!(response["memory"]["content"], "Test memory for retrieval");
    }

    #[tokio::test]
    async fn test_memory_get_nonexistent() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Try to get a nonexistent memory
        let result = tool
            .execute(json!({
                "operation": "get",
                "memory_id": "nonexistent_id"
            }))
            .await;

        assert!(result.is_err(), "Get nonexistent should fail");
    }

    #[tokio::test]
    async fn test_memory_delete() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        // Add a memory
        let add_result = tool
            .execute(json!({
                "operation": "add",
                "type": "context",
                "content": "Memory to be deleted"
            }))
            .await
            .expect("Add should succeed");

        let memory_id = add_result["memory_id"].as_str().unwrap();

        // Delete the memory
        let delete_result = tool
            .execute(json!({
                "operation": "delete",
                "memory_id": memory_id
            }))
            .await;

        assert!(delete_result.is_ok(), "Delete should succeed");
        let response = delete_result.unwrap();
        assert_eq!(response["success"], true);

        // Verify it's deleted by trying to get it
        let get_result = tool
            .execute(json!({
                "operation": "get",
                "memory_id": memory_id
            }))
            .await;

        assert!(get_result.is_err(), "Get deleted memory should fail");
    }

    #[tokio::test]
    async fn test_memory_clear_by_type() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        // Add multiple memories of different types
        for i in 0..3 {
            let _ = tool
                .execute(json!({
                    "operation": "add",
                    "type": "context",
                    "content": format!("Context memory {}", i)
                }))
                .await
                .expect("Add should succeed");
        }

        for i in 0..2 {
            let _ = tool
                .execute(json!({
                    "operation": "add",
                    "type": "knowledge",
                    "content": format!("Knowledge memory {}", i)
                }))
                .await
                .expect("Add should succeed");
        }

        // Clear context memories
        let clear_result = tool
            .execute(json!({
                "operation": "clear_by_type",
                "type": "context"
            }))
            .await;

        assert!(clear_result.is_ok(), "Clear by type should succeed");
        let response = clear_result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["type"], "context");

        // Verify only knowledge memories remain
        let list_result = tool.execute(json!({"operation": "list"})).await.unwrap();
        assert_eq!(list_result["count"], 2);
    }

    #[tokio::test]
    async fn test_memory_list_with_type_filter() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Add memories of different types
        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "user_pref",
                "content": "User prefers dark mode"
            }))
            .await;

        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "decision",
                "content": "Decided to use Rust for backend"
            }))
            .await;

        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "decision",
                "content": "Decided to use SurrealDB"
            }))
            .await;

        // List only decision memories
        let list_result = tool
            .execute(json!({
                "operation": "list",
                "type_filter": "decision"
            }))
            .await;

        assert!(list_result.is_ok());
        let response = list_result.unwrap();
        assert_eq!(response["count"], 2);
    }

    #[tokio::test]
    async fn test_memory_list_with_limit() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Add 5 memories
        for i in 0..5 {
            let _ = tool
                .execute(json!({
                    "operation": "add",
                    "type": "knowledge",
                    "content": format!("Knowledge {}", i)
                }))
                .await;
        }

        // List with limit 2
        let list_result = tool
            .execute(json!({
                "operation": "list",
                "limit": 2
            }))
            .await;

        assert!(list_result.is_ok());
        let response = list_result.unwrap();
        let memories = response["memories"].as_array().unwrap();
        assert_eq!(memories.len(), 2);
    }
}

/// Tests for workflow isolation and scope management.
#[cfg(test)]
mod workflow_isolation_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_workflow_isolation_separate_memories() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Create two tools for different workflows
        let tool_a = MemoryTool::new(
            db.clone(),
            None,
            Some("workflow_A".to_string()),
            "agent_a".to_string(),
        );

        let tool_b = MemoryTool::new(
            db.clone(),
            None,
            Some("workflow_B".to_string()),
            "agent_b".to_string(),
        );

        // Add context memories (auto-scoped to workflow) in workflow A
        let _ = tool_a
            .execute(json!({
                "operation": "add",
                "type": "context",
                "content": "Context from workflow A"
            }))
            .await
            .expect("Add should succeed");

        // Add context memories (auto-scoped to workflow) in workflow B
        let _ = tool_b
            .execute(json!({
                "operation": "add",
                "type": "context",
                "content": "Context from workflow B"
            }))
            .await
            .expect("Add should succeed");

        // List in workflow A (scope=workflow) - should only see A's context
        let list_a = tool_a
            .execute(json!({"operation": "list", "scope": "workflow"}))
            .await
            .unwrap();
        assert_eq!(list_a["count"], 1);
        assert!(list_a["memories"][0]["content"]
            .as_str()
            .unwrap()
            .contains("workflow A"));

        // List in workflow B (scope=workflow) - should only see B's context
        let list_b = tool_b
            .execute(json!({"operation": "list", "scope": "workflow"}))
            .await
            .unwrap();
        assert_eq!(list_b["count"], 1);
        assert!(list_b["memories"][0]["content"]
            .as_str()
            .unwrap()
            .contains("workflow B"));
    }

    #[tokio::test]
    async fn test_general_memories_visible_cross_workflow() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        // Tool A adds a user_pref (auto-scoped to general)
        let tool_a = MemoryTool::new(
            db.clone(),
            None,
            Some("workflow_A".to_string()),
            "agent_a".to_string(),
        );
        let _ = tool_a
            .execute(json!({
                "operation": "add",
                "type": "user_pref",
                "content": "User prefers dark mode"
            }))
            .await
            .expect("Add should succeed");

        // Tool A adds knowledge (auto-scoped to general)
        let _ = tool_a
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "SurrealDB supports HNSW indexing"
            }))
            .await
            .expect("Add should succeed");

        // Tool B from a different workflow can see general memories
        let tool_b = MemoryTool::new(
            db.clone(),
            None,
            Some("workflow_B".to_string()),
            "agent_b".to_string(),
        );

        // List with scope=both (default) should see both general memories
        let list_result = tool_b.execute(json!({"operation": "list"})).await.unwrap();
        assert_eq!(list_result["count"], 2);
    }

    #[tokio::test]
    async fn test_scope_override_on_add() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_override".to_string()),
            "test_agent".to_string(),
        );

        // Decision normally auto-scopes to workflow, but override to general
        let result = tool
            .execute(json!({
                "operation": "add",
                "type": "decision",
                "content": "Global policy: RGPD compliance required",
                "scope": "general"
            }))
            .await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        // Should be stored as general (null workflow_id) despite being a decision
        assert!(response["workflow_id"].is_null());

        // Knowledge normally auto-scopes to general, but override to workflow
        let result = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "Workflow-specific API endpoint documentation",
                "scope": "workflow"
            }))
            .await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        // Should be stored in workflow despite being knowledge
        assert_eq!(response["workflow_id"], "wf_override");
    }
}

/// Tests for search functionality including text fallback.
#[cfg(test)]
mod search_tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_search_text_fallback_filters_by_type() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Add memories
        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "SurrealDB is a multi-model database"
            }))
            .await;

        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "decision",
                "content": "We chose SurrealDB for its vector support"
            }))
            .await;

        // Search with type filter
        let result = tool
            .execute(json!({
                "operation": "search",
                "query": "SurrealDB",
                "type_filter": "knowledge"
            }))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        // Should only find the knowledge memory
        let results = response["results"].as_array().unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn test_search_respects_limit() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Add multiple matching memories
        for i in 0..5 {
            let _ = tool
                .execute(json!({
                    "operation": "add",
                    "type": "knowledge",
                    "content": format!("Rust programming tip {}", i)
                }))
                .await;
        }

        // Search with limit
        let result = tool
            .execute(json!({
                "operation": "search",
                "query": "Rust",
                "limit": 2
            }))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        let results = response["results"].as_array().unwrap();
        assert!(results.len() <= 2);
    }

    #[tokio::test]
    async fn test_search_empty_results() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Add a memory
        let _ = tool
            .execute(json!({
                "operation": "add",
                "type": "knowledge",
                "content": "Rust is a systems programming language"
            }))
            .await;

        // Search for something that doesn't match
        let result = tool
            .execute(json!({
                "operation": "search",
                "query": "JavaScript",
                "limit": 5
            }))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        let results = response["results"].as_array().unwrap();
        assert!(results.is_empty());
    }
}
