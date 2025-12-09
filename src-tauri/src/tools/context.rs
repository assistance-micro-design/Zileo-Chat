// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

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
use crate::tools::sub_agent_circuit_breaker::SubAgentCircuitBreaker;
use crate::tools::ToolFactory;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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
    /// Cancellation token for graceful shutdown of sub-agent execution (OPT-SA-7)
    ///
    /// When provided, sub-agents will monitor this token and abort execution
    /// if cancellation is requested. This enables the user to cancel long-running
    /// workflows and have sub-agents respond immediately.
    pub cancellation_token: Option<CancellationToken>,
    /// Circuit breaker for sub-agent execution resilience (OPT-SA-8)
    ///
    /// When provided, sub-agent tools will check the circuit state before execution
    /// and record success/failure after execution. This prevents cascade failures
    /// when the sub-agent system is experiencing issues.
    ///
    /// The circuit breaker is shared across all sub-agent tools in the workflow.
    pub circuit_breaker: Option<Arc<Mutex<SubAgentCircuitBreaker>>>,
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
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown (OPT-SA-7)
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
    ///     Some(cancellation_token),
    /// );
    /// ```
    pub fn new(
        registry: Arc<AgentRegistry>,
        orchestrator: Arc<AgentOrchestrator>,
        llm_manager: Arc<ProviderManager>,
        mcp_manager: Option<Arc<MCPManager>>,
        tool_factory: Arc<ToolFactory>,
        app_handle: Option<AppHandle>,
        cancellation_token: Option<CancellationToken>,
    ) -> Self {
        Self {
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory,
            app_handle,
            cancellation_token,
            circuit_breaker: None, // OPT-SA-8: Default to None for backward compatibility
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
            cancellation_token: None, // Use from_app_state_with_cancellation for token support
            circuit_breaker: None, // Use from_app_state_with_resilience for full resilience support
        }
    }

    /// Creates an AgentToolContext from AppState with cancellation token support (OPT-SA-7).
    ///
    /// This constructor should be used when executing workflows that need graceful
    /// cancellation support for sub-agents.
    ///
    /// # Arguments
    /// * `app_state` - The application state containing all managers
    /// * `mcp_manager` - Optional MCP manager
    /// * `app_handle` - Optional Tauri app handle for event emission
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
    ///
    /// # Example
    /// ```ignore
    /// // In execute_workflow_streaming
    /// let token = state.create_cancellation_token(&workflow_id).await;
    /// let context = AgentToolContext::from_app_state_with_cancellation(
    ///     &state,
    ///     Some(state.mcp_manager.clone()),
    ///     Some(app_handle),
    ///     Some(token),
    /// );
    /// ```
    pub fn from_app_state_with_cancellation(
        app_state: &AppState,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<AppHandle>,
        cancellation_token: Option<CancellationToken>,
    ) -> Self {
        Self {
            registry: app_state.registry.clone(),
            orchestrator: app_state.orchestrator.clone(),
            llm_manager: app_state.llm_manager.clone(),
            mcp_manager: mcp_manager.or_else(|| Some(app_state.mcp_manager.clone())),
            tool_factory: app_state.tool_factory.clone(),
            app_handle,
            cancellation_token,
            circuit_breaker: None, // Use from_app_state_with_resilience for circuit breaker
        }
    }

    /// Creates an AgentToolContext from AppState with full resilience features (OPT-SA-7, OPT-SA-8).
    ///
    /// This constructor should be used when executing workflows that need both
    /// graceful cancellation and circuit breaker protection for sub-agents.
    ///
    /// # Arguments
    /// * `app_state` - The application state containing all managers
    /// * `mcp_manager` - Optional MCP manager
    /// * `app_handle` - Optional Tauri app handle for event emission
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown (OPT-SA-7)
    /// * `circuit_breaker` - Optional circuit breaker for execution resilience (OPT-SA-8)
    ///
    /// # Example
    /// ```ignore
    /// // In execute_workflow_streaming
    /// let token = state.create_cancellation_token(&workflow_id).await;
    /// let circuit_breaker = Arc::new(Mutex::new(SubAgentCircuitBreaker::with_defaults()));
    /// let context = AgentToolContext::from_app_state_with_resilience(
    ///     &state,
    ///     Some(state.mcp_manager.clone()),
    ///     Some(app_handle),
    ///     Some(token),
    ///     Some(circuit_breaker),
    /// );
    /// ```
    pub fn from_app_state_with_resilience(
        app_state: &AppState,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<AppHandle>,
        cancellation_token: Option<CancellationToken>,
        circuit_breaker: Option<Arc<Mutex<SubAgentCircuitBreaker>>>,
    ) -> Self {
        Self {
            registry: app_state.registry.clone(),
            orchestrator: app_state.orchestrator.clone(),
            llm_manager: app_state.llm_manager.clone(),
            mcp_manager: mcp_manager.or_else(|| Some(app_state.mcp_manager.clone())),
            tool_factory: app_state.tool_factory.clone(),
            app_handle,
            cancellation_token,
            circuit_breaker,
        }
    }

    /// Creates an AgentToolContext with all dependencies from AppState.
    ///
    /// Convenience method that always includes the MCP manager from AppState.
    /// Includes app_handle if available in AppState.
    /// Does NOT include cancellation token or circuit breaker - use from_app_state_with_resilience for that.
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
            None, // cancellation_token
        );

        assert!(Arc::ptr_eq(&context.registry, &registry));
        assert!(Arc::ptr_eq(&context.orchestrator, &orchestrator));
        assert!(Arc::ptr_eq(&context.llm_manager, &llm_manager));
        assert!(context.mcp_manager.is_none());
        assert!(Arc::ptr_eq(&context.tool_factory, &tool_factory));
        assert!(context.app_handle.is_none());
        assert!(context.cancellation_token.is_none());
        assert!(context.circuit_breaker.is_none()); // OPT-SA-8
    }

    #[tokio::test]
    async fn test_context_with_cancellation_token() {
        let state = create_test_state().await;
        let token = CancellationToken::new();

        let context = AgentToolContext::from_app_state_with_cancellation(
            &state,
            Some(state.mcp_manager.clone()),
            None,
            Some(token.clone()),
        );

        // Verify cancellation token is set
        assert!(context.cancellation_token.is_some());

        // Verify token is not cancelled initially
        assert!(!context.cancellation_token.as_ref().unwrap().is_cancelled());

        // Cancel the original token
        token.cancel();

        // Verify the context's token is also cancelled (same token)
        assert!(context.cancellation_token.as_ref().unwrap().is_cancelled());
    }

    #[tokio::test]
    async fn test_context_without_cancellation_token() {
        let state = create_test_state().await;

        // from_app_state_full does not include cancellation token
        let context = AgentToolContext::from_app_state_full(&state);
        assert!(context.cancellation_token.is_none());

        // from_app_state does not include cancellation token
        let context2 = AgentToolContext::from_app_state(&state, None, None);
        assert!(context2.cancellation_token.is_none());
    }

    // =========================================================================
    // OPT-SA-8: Circuit Breaker Tests
    // =========================================================================

    #[tokio::test]
    async fn test_context_with_circuit_breaker() {
        let state = create_test_state().await;
        let token = CancellationToken::new();
        let circuit_breaker = Arc::new(Mutex::new(SubAgentCircuitBreaker::with_defaults()));

        let context = AgentToolContext::from_app_state_with_resilience(
            &state,
            Some(state.mcp_manager.clone()),
            None,
            Some(token.clone()),
            Some(circuit_breaker.clone()),
        );

        // Verify circuit breaker is set
        assert!(context.circuit_breaker.is_some());

        // Verify circuit breaker is closed initially
        let cb = context.circuit_breaker.as_ref().unwrap();
        let guard = cb.lock().await;
        assert_eq!(
            guard.state(),
            crate::tools::sub_agent_circuit_breaker::CircuitState::Closed
        );
        assert_eq!(guard.failure_count(), 0);
    }

    #[tokio::test]
    async fn test_context_circuit_breaker_shared_state() {
        let state = create_test_state().await;
        let circuit_breaker = Arc::new(Mutex::new(SubAgentCircuitBreaker::with_defaults()));

        let context = AgentToolContext::from_app_state_with_resilience(
            &state,
            Some(state.mcp_manager.clone()),
            None,
            None,
            Some(circuit_breaker.clone()),
        );

        // Record failures via the original reference
        {
            let mut guard = circuit_breaker.lock().await;
            guard.record_failure();
            guard.record_failure();
        }

        // Verify context sees the same state
        let cb = context.circuit_breaker.as_ref().unwrap();
        let guard = cb.lock().await;
        assert_eq!(guard.failure_count(), 2);
    }

    #[tokio::test]
    async fn test_context_without_circuit_breaker() {
        let state = create_test_state().await;

        // from_app_state_full does not include circuit breaker
        let context = AgentToolContext::from_app_state_full(&state);
        assert!(context.circuit_breaker.is_none());

        // from_app_state does not include circuit breaker
        let context2 = AgentToolContext::from_app_state(&state, None, None);
        assert!(context2.circuit_breaker.is_none());

        // from_app_state_with_cancellation does not include circuit breaker
        let context3 = AgentToolContext::from_app_state_with_cancellation(
            &state,
            None,
            None,
            Some(CancellationToken::new()),
        );
        assert!(context3.circuit_breaker.is_none());
    }
}
