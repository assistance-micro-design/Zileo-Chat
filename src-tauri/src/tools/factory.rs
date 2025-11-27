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
use crate::tools::context::AgentToolContext;
use crate::tools::delegate_task::DelegateTaskTool;
use crate::tools::parallel_tasks::ParallelTasksTool;
use crate::tools::spawn_agent::SpawnAgentTool;
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

            _ => {
                warn!(tool_name = %tool_name, "Unknown tool requested");
                Err(format!(
                    "Unknown tool: '{}'. Available tools: MemoryTool, TodoTool",
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
    ///
    /// This includes both basic tools (MemoryTool, TodoTool) and
    /// sub-agent tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool).
    ///
    /// Note: Sub-agent tools require `AgentToolContext` and should be
    /// created using `create_tool_with_context()`.
    pub fn available_tools() -> Vec<&'static str> {
        vec![
            "MemoryTool",
            "TodoTool",
            "SpawnAgentTool",
            "DelegateTaskTool",
            "ParallelTasksTool",
        ]
    }

    /// Returns list of basic tool names (those not requiring AgentToolContext).
    pub fn basic_tools() -> Vec<&'static str> {
        vec!["MemoryTool", "TodoTool"]
    }

    /// Returns list of sub-agent tool names (those requiring AgentToolContext).
    ///
    /// These tools are only available to the primary workflow agent
    /// and are NOT provided to sub-agents (to prevent chaining).
    pub fn sub_agent_tools() -> Vec<&'static str> {
        vec!["SpawnAgentTool", "DelegateTaskTool", "ParallelTasksTool"]
    }

    /// Checks if a tool name is valid.
    pub fn is_valid_tool(name: &str) -> bool {
        Self::available_tools().contains(&name)
    }

    /// Checks if a tool requires AgentToolContext.
    pub fn requires_context(name: &str) -> bool {
        Self::sub_agent_tools().contains(&name)
    }

    /// Creates a tool instance with AgentToolContext.
    ///
    /// This method is used for tools that need access to the agent system,
    /// such as SpawnAgentTool, DelegateTaskTool, and ParallelTasksTool.
    ///
    /// # Arguments
    /// * `tool_name` - Tool identifier
    /// * `workflow_id` - Workflow ID for scoping
    /// * `agent_id` - Agent ID using the tool
    /// * `context` - AgentToolContext providing system dependencies
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Returns
    /// * `Ok(Arc<dyn Tool>)` - Tool instance ready for use
    /// * `Err(String)` - Error message if tool is unknown or cannot be created
    ///
    /// # Sub-Agent Constraints
    ///
    /// When `is_primary_agent` is `false`, sub-agent tools (SpawnAgentTool,
    /// DelegateTaskTool, ParallelTasksTool) will NOT be created. This enforces
    /// the single-level constraint where sub-agents cannot spawn other sub-agents.
    ///
    /// # Example
    /// ```ignore
    /// let tool = factory.create_tool_with_context(
    ///     "SpawnAgentTool",
    ///     Some("wf_001".into()),
    ///     "primary_agent".into(),
    ///     context,
    ///     true, // is_primary_agent
    /// )?;
    /// ```
    pub fn create_tool_with_context(
        &self,
        tool_name: &str,
        workflow_id: Option<String>,
        agent_id: String,
        context: AgentToolContext,
        is_primary_agent: bool,
    ) -> Result<Arc<dyn Tool>, String> {
        debug!(
            tool_name = %tool_name,
            workflow_id = ?workflow_id,
            agent_id = %agent_id,
            is_primary_agent = is_primary_agent,
            "Creating tool instance with context"
        );

        // Check if this is a sub-agent tool and enforce constraints
        if Self::requires_context(tool_name) && !is_primary_agent {
            warn!(
                tool_name = %tool_name,
                agent_id = %agent_id,
                "Sub-agent attempted to access sub-agent tool - denied"
            );
            return Err(format!(
                "Tool '{}' is only available to the primary workflow agent. \
                 Sub-agents cannot spawn other sub-agents or delegate tasks.",
                tool_name
            ));
        }

        match tool_name {
            // Basic tools (delegate to create_tool)
            "MemoryTool" | "TodoTool" => self.create_tool(tool_name, workflow_id, agent_id),

            // Sub-agent tools (require context)
            "SpawnAgentTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = SpawnAgentTool::new(
                    self.db.clone(),
                    context,
                    agent_id,
                    wf_id,
                    is_primary_agent,
                );
                info!("SpawnAgentTool instance created");
                Ok(Arc::new(tool))
            }

            "DelegateTaskTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = DelegateTaskTool::new(
                    self.db.clone(),
                    context,
                    agent_id,
                    wf_id,
                    is_primary_agent,
                );
                info!("DelegateTaskTool instance created");
                Ok(Arc::new(tool))
            }

            "ParallelTasksTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = ParallelTasksTool::new(
                    self.db.clone(),
                    context,
                    agent_id,
                    wf_id,
                    is_primary_agent,
                );
                info!("ParallelTasksTool instance created");
                Ok(Arc::new(tool))
            }

            _ => {
                warn!(tool_name = %tool_name, "Unknown tool requested");
                Err(format!(
                    "Unknown tool: '{}'. Available tools: {:?}",
                    tool_name,
                    Self::available_tools()
                ))
            }
        }
    }

    /// Creates multiple tools, handling both basic and context-aware tools.
    ///
    /// # Arguments
    /// * `tool_names` - List of tool identifiers
    /// * `workflow_id` - Optional workflow ID for scoping
    /// * `agent_id` - Agent ID using the tools
    /// * `context` - Optional AgentToolContext for sub-agent tools
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Returns
    /// Vector of successfully created tools. Failed tools are logged but skipped.
    pub fn create_tools_with_context(
        &self,
        tool_names: &[String],
        workflow_id: Option<String>,
        agent_id: String,
        context: Option<AgentToolContext>,
        is_primary_agent: bool,
    ) -> Vec<Arc<dyn Tool>> {
        let mut tools = Vec::new();

        for name in tool_names {
            let result = if Self::requires_context(name) {
                if let Some(ctx) = &context {
                    self.create_tool_with_context(
                        name,
                        workflow_id.clone(),
                        agent_id.clone(),
                        ctx.clone(),
                        is_primary_agent,
                    )
                } else {
                    warn!(
                        tool_name = %name,
                        "Sub-agent tool requested without context - skipping"
                    );
                    Err("AgentToolContext required for sub-agent tools".to_string())
                }
            } else {
                self.create_tool(name, workflow_id.clone(), agent_id.clone())
            };

            match result {
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
            "Tool batch creation with context completed"
        );

        tools
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
        assert!(tools.contains(&"SpawnAgentTool"));
        assert!(tools.contains(&"DelegateTaskTool"));
        assert!(tools.contains(&"ParallelTasksTool"));
        assert_eq!(tools.len(), 5);
    }

    #[test]
    fn test_basic_tools() {
        let tools = ToolFactory::basic_tools();
        assert!(tools.contains(&"MemoryTool"));
        assert!(tools.contains(&"TodoTool"));
        assert!(!tools.contains(&"SpawnAgentTool"));
        assert_eq!(tools.len(), 2);
    }

    #[test]
    fn test_sub_agent_tools() {
        let tools = ToolFactory::sub_agent_tools();
        assert!(tools.contains(&"SpawnAgentTool"));
        assert!(tools.contains(&"DelegateTaskTool"));
        assert!(tools.contains(&"ParallelTasksTool"));
        assert!(!tools.contains(&"MemoryTool"));
        assert_eq!(tools.len(), 3);
    }

    #[test]
    fn test_is_valid_tool() {
        assert!(ToolFactory::is_valid_tool("MemoryTool"));
        assert!(ToolFactory::is_valid_tool("TodoTool"));
        assert!(ToolFactory::is_valid_tool("SpawnAgentTool"));
        assert!(!ToolFactory::is_valid_tool("InvalidTool"));
        assert!(!ToolFactory::is_valid_tool("memory_tool"));
    }

    #[test]
    fn test_requires_context() {
        assert!(!ToolFactory::requires_context("MemoryTool"));
        assert!(!ToolFactory::requires_context("TodoTool"));
        assert!(ToolFactory::requires_context("SpawnAgentTool"));
        assert!(ToolFactory::requires_context("DelegateTaskTool"));
        assert!(ToolFactory::requires_context("ParallelTasksTool"));
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
    async fn test_factory_without_embedding() {
        let factory = create_test_factory().await;

        // MemoryTool should still work without embedding service
        let result = factory.create_tool("MemoryTool", None, "test_agent".to_string());
        assert!(result.is_ok());
    }
}
