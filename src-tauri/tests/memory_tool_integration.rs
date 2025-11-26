// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for MemoryTool Phase 4 Integration.
//!
//! These tests verify that MemoryTool works correctly when integrated
//! with the agent system, ToolFactory, and AppState.

use std::sync::Arc;
use tempfile::tempdir;

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

        let factory = ToolFactory::new(db, None);

        // Create MemoryTool via factory
        let result = factory.create_tool(
            "MemoryTool",
            Some("wf_integration_test".to_string()),
            "integration_test_agent".to_string(),
        );

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

        let factory = ToolFactory::new(db, None);

        // Create TodoTool via factory
        let result = factory.create_tool(
            "TodoTool",
            Some("wf_integration_test".to_string()),
            "integration_test_agent".to_string(),
        );

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

        let factory = ToolFactory::new(db, None);

        let tool_names = vec![
            "MemoryTool".to_string(),
            "TodoTool".to_string(),
            "UnknownTool".to_string(), // Should be skipped
        ];

        let tools = factory.create_tools(
            &tool_names,
            Some("wf_batch".to_string()),
            "batch_agent".to_string(),
        );

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

        // List memories
        let list_result = tool.execute(json!({"operation": "list"})).await;

        assert!(list_result.is_ok(), "List operation should succeed");
        let list_response = list_result.unwrap();
        assert_eq!(list_response["success"], true);
        assert!(list_response["count"].as_u64().unwrap() >= 1);
    }

    #[tokio::test]
    async fn test_memory_tool_scope_switching() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::{MemoryTool, Tool};

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(db, None, None, "test_agent".to_string());

        // Activate workflow scope
        let result = tool
            .execute(json!({
                "operation": "activate_workflow",
                "workflow_id": "wf_scope_test"
            }))
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["scope"], "workflow");
        assert_eq!(response["workflow_id"], "wf_scope_test");

        // Switch to general mode
        let result = tool.execute(json!({"operation": "activate_general"})).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert_eq!(response["scope"], "general");
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
        };

        assert!(config.has_valid_tools());
        assert!(config.validate_tools().is_empty());
        assert!(config.tools.contains(&"MemoryTool".to_string()));
    }
}
