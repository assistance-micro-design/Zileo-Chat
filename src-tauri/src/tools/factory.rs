// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

// Allow dead code until Phase 6: Full Agent Integration with tools
#![allow(dead_code)]

//! Tool Factory for dynamic tool instantiation.
//!
//! This module provides a factory pattern for creating tool instances
//! based on their string identifiers from agent configuration.
//!
//! # Supported Tools
//!
//! | Tool ID | Module | Description |
//! |---------|--------|-------------|
//! | `MemoryTool` | [`memory`] | Contextual memory with semantic search |
//! | `TodoTool` | [`todo`] | Task management for workflows |
//! | `SurrealDBTool` | [`db`] | Direct database CRUD operations |
//! | `QueryBuilderTool` | [`db`] | SurrealQL query generation |
//! | `AnalyticsTool` | [`db`] | Aggregations and analytics |
//!
//! # Usage
//!
//! ```ignore
//! use crate::tools::factory::ToolFactory;
//!
//! let factory = ToolFactory::new(
//!     db.clone(),
//!     Some(embedding_service.clone()),
//! );
//!
//! // Create a specific tool for an agent
//! let memory_tool = factory.create_tool(
//!     "MemoryTool",
//!     Some("wf_001".to_string()),
//!     "db_agent".to_string(),
//! )?;
//! ```

use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::tools::{MemoryTool, TodoTool, Tool};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Factory for creating tool instances.
///
/// The factory holds shared dependencies (database, embedding service)
/// and creates tool instances on demand based on their string identifiers.
pub struct ToolFactory {
    /// Database client shared by all tools
    db: Arc<DBClient>,
    /// Embedding service for MemoryTool (optional)
    embedding_service: Option<Arc<EmbeddingService>>,
}

impl ToolFactory {
    /// Creates a new tool factory with dependencies.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `embedding_service` - Optional embedding service for semantic search
    ///
    /// # Example
    /// ```ignore
    /// let factory = ToolFactory::new(db.clone(), Some(embed_svc.clone()));
    /// ```
    pub fn new(db: Arc<DBClient>, embedding_service: Option<Arc<EmbeddingService>>) -> Self {
        info!(
            has_embedding = embedding_service.is_some(),
            "ToolFactory initialized"
        );
        Self {
            db,
            embedding_service,
        }
    }

    /// Creates a tool instance by name.
    ///
    /// # Arguments
    /// * `tool_name` - Tool identifier (e.g., "MemoryTool", "TodoTool")
    /// * `workflow_id` - Optional workflow ID for scoping
    /// * `agent_id` - Agent ID using the tool
    ///
    /// # Returns
    /// * `Ok(Arc<dyn Tool>)` - Tool instance ready for use
    /// * `Err(String)` - Error message if tool is unknown
    ///
    /// # Supported Tools
    /// * `MemoryTool` - Contextual memory with semantic search
    /// * `TodoTool` - Task management for workflow decomposition
    ///
    /// # Example
    /// ```ignore
    /// let tool = factory.create_tool(
    ///     "MemoryTool",
    ///     Some("wf_001".into()),
    ///     "db_agent".into()
    /// )?;
    /// ```
    pub fn create_tool(
        &self,
        tool_name: &str,
        workflow_id: Option<String>,
        agent_id: String,
    ) -> Result<Arc<dyn Tool>, String> {
        debug!(
            tool_name = %tool_name,
            workflow_id = ?workflow_id,
            agent_id = %agent_id,
            "Creating tool instance"
        );

        match tool_name {
            "MemoryTool" => {
                let tool = MemoryTool::new(
                    self.db.clone(),
                    self.embedding_service.clone(),
                    workflow_id,
                    agent_id,
                );
                info!("MemoryTool instance created");
                Ok(Arc::new(tool))
            }

            "TodoTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = TodoTool::new(self.db.clone(), wf_id, agent_id);
                info!("TodoTool instance created");
                Ok(Arc::new(tool))
            }

            // Database tools are stubs - will be fully implemented in future phases
            "SurrealDBTool" | "QueryBuilderTool" | "AnalyticsTool" => {
                warn!(
                    tool_name = %tool_name,
                    "Database tool requested but not yet fully implemented"
                );
                Err(format!(
                    "Tool '{}' is defined but not yet fully implemented. Available tools: MemoryTool, TodoTool",
                    tool_name
                ))
            }

            _ => {
                warn!(tool_name = %tool_name, "Unknown tool requested");
                Err(format!(
                    "Unknown tool: '{}'. Available tools: MemoryTool, TodoTool, SurrealDBTool, QueryBuilderTool, AnalyticsTool",
                    tool_name
                ))
            }
        }
    }

    /// Creates multiple tools from a list of names.
    ///
    /// # Arguments
    /// * `tool_names` - List of tool identifiers
    /// * `workflow_id` - Optional workflow ID for scoping
    /// * `agent_id` - Agent ID using the tools
    ///
    /// # Returns
    /// Vector of successfully created tools. Failed tools are logged but skipped.
    pub fn create_tools(
        &self,
        tool_names: &[String],
        workflow_id: Option<String>,
        agent_id: String,
    ) -> Vec<Arc<dyn Tool>> {
        let mut tools = Vec::new();

        for name in tool_names {
            match self.create_tool(name, workflow_id.clone(), agent_id.clone()) {
                Ok(tool) => {
                    tools.push(tool);
                }
                Err(e) => {
                    warn!(
                        tool_name = %name,
                        error = %e,
                        "Failed to create tool, skipping"
                    );
                }
            }
        }

        debug!(
            requested = tool_names.len(),
            created = tools.len(),
            "Tool batch creation completed"
        );

        tools
    }

    /// Returns list of available tool names.
    pub fn available_tools() -> Vec<&'static str> {
        vec![
            "MemoryTool",
            "TodoTool",
            "SurrealDBTool",
            "QueryBuilderTool",
            "AnalyticsTool",
        ]
    }

    /// Checks if a tool name is valid.
    pub fn is_valid_tool(name: &str) -> bool {
        Self::available_tools().contains(&name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_factory() -> ToolFactory {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");
        let db = Arc::new(
            DBClient::new(db_path.to_str().unwrap())
                .await
                .expect("Failed to create DB"),
        );
        db.initialize_schema().await.expect("Failed to init schema");
        ToolFactory::new(db, None)
    }

    #[test]
    fn test_available_tools() {
        let tools = ToolFactory::available_tools();
        assert!(tools.contains(&"MemoryTool"));
        assert!(tools.contains(&"TodoTool"));
        assert!(tools.contains(&"SurrealDBTool"));
        assert!(tools.contains(&"QueryBuilderTool"));
        assert!(tools.contains(&"AnalyticsTool"));
    }

    #[test]
    fn test_is_valid_tool() {
        assert!(ToolFactory::is_valid_tool("MemoryTool"));
        assert!(ToolFactory::is_valid_tool("TodoTool"));
        assert!(!ToolFactory::is_valid_tool("InvalidTool"));
        assert!(!ToolFactory::is_valid_tool("memory_tool"));
    }

    #[tokio::test]
    async fn test_create_memory_tool() {
        let factory = create_test_factory().await;

        let result = factory.create_tool(
            "MemoryTool",
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        assert!(result.is_ok());
        let tool = result.unwrap();
        assert_eq!(tool.definition().id, "MemoryTool");
        assert!(!tool.requires_confirmation());
    }

    #[tokio::test]
    async fn test_create_todo_tool() {
        let factory = create_test_factory().await;

        let result = factory.create_tool(
            "TodoTool",
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        assert!(result.is_ok());
        let tool = result.unwrap();
        assert_eq!(tool.definition().id, "TodoTool");
    }

    #[tokio::test]
    async fn test_create_unknown_tool() {
        let factory = create_test_factory().await;

        let result = factory.create_tool("UnknownTool", None, "test_agent".to_string());

        assert!(result.is_err());
        match result {
            Err(msg) => assert!(msg.contains("Unknown tool")),
            Ok(_) => panic!("Expected error for unknown tool"),
        }
    }

    #[tokio::test]
    async fn test_create_tools_batch() {
        let factory = create_test_factory().await;

        let tool_names = vec![
            "MemoryTool".to_string(),
            "TodoTool".to_string(),
            "InvalidTool".to_string(), // Should be skipped
        ];

        let tools = factory.create_tools(
            &tool_names,
            Some("wf_batch".to_string()),
            "batch_agent".to_string(),
        );

        // Should create 2 valid tools, skip 1 invalid
        assert_eq!(tools.len(), 2);
    }

    #[tokio::test]
    async fn test_create_database_tool_stub() {
        let factory = create_test_factory().await;

        let result = factory.create_tool("SurrealDBTool", None, "test_agent".to_string());

        // Database tools are stubs, should return error
        assert!(result.is_err());
        match result {
            Err(msg) => assert!(msg.contains("not yet fully implemented")),
            Ok(_) => panic!("Expected error for stub tool"),
        }
    }

    #[tokio::test]
    async fn test_factory_without_embedding() {
        let factory = create_test_factory().await;

        // MemoryTool should still work without embedding service
        let result = factory.create_tool("MemoryTool", None, "test_agent".to_string());
        assert!(result.is_ok());
    }
}
