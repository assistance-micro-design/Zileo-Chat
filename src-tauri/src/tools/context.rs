// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Agent Tool Context for Sub-Agent System
//!
//! This module provides the `AgentToolContext` struct which carries all dependencies
//! needed by tools that require access to the agent system (SpawnAgentTool,
//! DelegateTaskTool, ParallelTasksTool).
//!
//! # Overview
//!
//! Unlike simple tools (MemoryTool, TodoTool) that only need database access,
//! sub-agent tools require access to:
//! - Agent registry (to look up and register agents)
//! - Agent orchestrator (to execute agents)
//! - LLM provider manager (to create LLM instances for sub-agents)
//! - MCP manager (to share MCP connections)
//! - Tool factory (to create tools for sub-agents)
//!
//! # Usage
//!
//! ```ignore
//! use crate::tools::context::AgentToolContext;
//! use crate::state::AppState;
//!
//! // Create context from app state
//! let context = AgentToolContext::from_app_state(&state, Some(mcp_manager.clone()));
//!
//! // Use context to create sub-agent tools
//! let spawn_tool = SpawnAgentTool::new(db, context, parent_agent_id, workflow_id);
//! ```

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::state::AppState;
use crate::tools::ToolFactory;
use std::sync::Arc;
use tauri::AppHandle;

/// Context providing agent-level dependencies to tools.
///
/// This struct is passed to tools that need access to the broader agent system,
/// particularly for sub-agent operations (spawning, delegation, parallel execution).
///
/// # Thread Safety
///
/// All fields are wrapped in `Arc` for thread-safe sharing across async operations.
/// The context can be cloned cheaply as it only clones Arc references.
///
/// # Sub-Agent Hierarchy Rules
///
/// Tools using this context must enforce the sub-agent constraints:
/// - Maximum 3 sub-agents per workflow
/// - Single level only (sub-agents cannot spawn other sub-agents)
/// - Only the primary workflow agent has access to sub-agent tools
#[allow(dead_code)]
#[derive(Clone)]
pub struct AgentToolContext {
    /// Agent registry for agent lookup and registration
    pub registry: Arc<AgentRegistry>,
    /// Agent orchestrator for executing agents
    pub orchestrator: Arc<AgentOrchestrator>,
    /// LLM provider manager for creating LLM instances
    pub llm_manager: Arc<ProviderManager>,
    /// MCP manager for tool routing (optional)
    pub mcp_manager: Option<Arc<MCPManager>>,
    /// Tool factory for creating tools for sub-agents
    pub tool_factory: Arc<ToolFactory>,
    /// Tauri app handle for emitting events (optional, for validation)
    pub app_handle: Option<AppHandle>,
}

#[allow(dead_code)]
impl AgentToolContext {
    /// Creates a new AgentToolContext with the provided dependencies.
    ///
    /// # Arguments
    /// * `registry` - Agent registry for agent management
    /// * `orchestrator` - Agent orchestrator for execution
    /// * `llm_manager` - LLM provider manager
    /// * `mcp_manager` - Optional MCP manager for tool routing
    /// * `tool_factory` - Factory for creating tools
    /// * `app_handle` - Optional Tauri app handle for event emission
    ///
    /// # Example
    /// ```ignore
    /// let context = AgentToolContext::new(
    ///     registry.clone(),
    ///     orchestrator.clone(),
    ///     llm_manager.clone(),
    ///     Some(mcp_manager.clone()),
    ///     tool_factory.clone(),
    ///     Some(app_handle),
    /// );
    /// ```
    pub fn new(
        registry: Arc<AgentRegistry>,
        orchestrator: Arc<AgentOrchestrator>,
        llm_manager: Arc<ProviderManager>,
        mcp_manager: Option<Arc<MCPManager>>,
        tool_factory: Arc<ToolFactory>,
        app_handle: Option<AppHandle>,
    ) -> Self {
        Self {
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory,
            app_handle,
        }
    }

    /// Creates an AgentToolContext from AppState.
    ///
    /// This is the primary constructor for use in Tauri commands.
    /// It extracts all necessary dependencies from the shared application state.
    ///
    /// # Arguments
    /// * `app_state` - The application state containing all managers
    /// * `mcp_manager` - Optional MCP manager (may be passed separately in some contexts)
    /// * `app_handle` - Optional Tauri app handle for event emission
    ///
    /// # Example
    /// ```ignore
    /// // In a Tauri command
    /// let context = AgentToolContext::from_app_state(&state, Some(state.mcp_manager.clone()), Some(app_handle));
    /// ```
    pub fn from_app_state(
        app_state: &AppState,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<AppHandle>,
    ) -> Self {
        Self {
            registry: app_state.registry.clone(),
            orchestrator: app_state.orchestrator.clone(),
            llm_manager: app_state.llm_manager.clone(),
            mcp_manager: mcp_manager.or_else(|| Some(app_state.mcp_manager.clone())),
            tool_factory: app_state.tool_factory.clone(),
            app_handle,
        }
    }

    /// Creates an AgentToolContext with all dependencies from AppState.
    ///
    /// Convenience method that always includes the MCP manager from AppState.
    /// Includes app_handle if available in AppState.
    ///
    /// # Arguments
    /// * `app_state` - The application state containing all managers
    ///
    /// # Example
    /// ```ignore
    /// let context = AgentToolContext::from_app_state_full(&state);
    /// ```
    pub fn from_app_state_full(app_state: &AppState) -> Self {
        // Get app_handle from AppState (uses std::sync::RwLock)
        let app_handle = app_state
            .app_handle
            .read()
            .ok()
            .and_then(|guard| guard.clone());

        Self::from_app_state(app_state, Some(app_state.mcp_manager.clone()), app_handle)
    }

    /// Creates an AgentToolContext with all dependencies from AppState including AppHandle.
    ///
    /// Full constructor that includes app_handle for event emission.
    ///
    /// # Arguments
    /// * `app_state` - The application state containing all managers
    /// * `app_handle` - Tauri app handle for event emission
    ///
    /// # Example
    /// ```ignore
    /// let context = AgentToolContext::from_app_state_with_handle(&state, app_handle);
    /// ```
    pub fn from_app_state_with_handle(app_state: &AppState, app_handle: AppHandle) -> Self {
        Self::from_app_state(
            app_state,
            Some(app_state.mcp_manager.clone()),
            Some(app_handle),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DBClient;
    use tempfile::tempdir;

    async fn create_test_state() -> AppState {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_context_db");
        AppState::new(db_path.to_str().unwrap())
            .await
            .expect("Failed to create AppState")
    }

    #[tokio::test]
    async fn test_context_from_app_state() {
        let state = create_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);

        // Verify all fields are populated
        assert!(context.mcp_manager.is_some());
        // Registry should be the same instance
        assert!(Arc::ptr_eq(&context.registry, &state.registry));
        assert!(Arc::ptr_eq(&context.orchestrator, &state.orchestrator));
        assert!(Arc::ptr_eq(&context.llm_manager, &state.llm_manager));
        assert!(Arc::ptr_eq(&context.tool_factory, &state.tool_factory));
    }

    #[tokio::test]
    async fn test_context_clone() {
        let state = create_test_state().await;
        let context1 = AgentToolContext::from_app_state_full(&state);
        let context2 = context1.clone();

        // Cloned context should share the same Arc instances
        assert!(Arc::ptr_eq(&context1.registry, &context2.registry));
        assert!(Arc::ptr_eq(&context1.orchestrator, &context2.orchestrator));
        assert!(Arc::ptr_eq(&context1.llm_manager, &context2.llm_manager));
    }

    #[tokio::test]
    async fn test_context_without_mcp() {
        let state = create_test_state().await;
        let context = AgentToolContext::from_app_state(&state, None, None);

        // When None is passed, it should still get MCP from state
        assert!(context.mcp_manager.is_some());
        // app_handle should be None
        assert!(context.app_handle.is_none());
    }

    #[tokio::test]
    async fn test_context_new() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_context_new_db");
        let db = Arc::new(
            DBClient::new(db_path.to_str().unwrap())
                .await
                .expect("Failed to create DB"),
        );
        db.initialize_schema().await.expect("Failed to init schema");

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
        let llm_manager = Arc::new(ProviderManager::new());
        let tool_factory = Arc::new(ToolFactory::new(db.clone(), None));

        let context = AgentToolContext::new(
            registry.clone(),
            orchestrator.clone(),
            llm_manager.clone(),
            None,
            tool_factory.clone(),
            None,
        );

        assert!(Arc::ptr_eq(&context.registry, &registry));
        assert!(Arc::ptr_eq(&context.orchestrator, &orchestrator));
        assert!(Arc::ptr_eq(&context.llm_manager, &llm_manager));
        assert!(context.mcp_manager.is_none());
        assert!(Arc::ptr_eq(&context.tool_factory, &tool_factory));
        assert!(context.app_handle.is_none());
    }
}
