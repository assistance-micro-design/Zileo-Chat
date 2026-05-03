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
// Consumed by sub-agent tools wired through the lib; not all fields are read
// from the binary path.
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
    /// Cancellation token for graceful shutdown of sub-agent execution
    ///
    /// When provided, sub-agents will monitor this token and abort execution
    /// if cancellation is requested. This enables the user to cancel long-running
    /// workflows and have sub-agents respond immediately.
    pub cancellation_token: Option<CancellationToken>,
    /// Circuit breaker for sub-agent execution resilience
    ///
    /// When provided, sub-agent tools will check the circuit state before execution
    /// and record success/failure after execution. This prevents cascade failures
    /// when the sub-agent system is experiencing issues.
    ///
    /// The circuit breaker is shared across all sub-agent tools in the workflow.
    pub circuit_breaker: Option<Arc<Mutex<SubAgentCircuitBreaker>>>,
    /// Assistant message_id of the agent that owns this context.
    ///
    /// Set on the primary's tool loop to the workflow's pre-allocated
    /// message_id. Sub-agent tools propagate it as `parent_message_id` when
    /// creating `sub_agent_execution` records, so attribution is correct
    /// at CREATE time (replaces the legacy bulk UPDATE in
    /// `persistence_step.rs` — H2 audit 2026-05-02).
    pub current_message_id: Option<String>,
}

// Constructor used by sub-agent tool factories in the lib; lib/bin split.
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
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
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
            circuit_breaker: None, // Default to None for backward compatibility
            current_message_id: None,
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
            current_message_id: None,
        }
    }

    /// Creates an AgentToolContext from AppState with cancellation token support.
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
            current_message_id: None,
        }
    }

    /// Creates an AgentToolContext from AppState with full resilience features.
    ///
    /// This constructor should be used when executing workflows that need both
    /// graceful cancellation and circuit breaker protection for sub-agents.
    ///
    /// # Arguments
    /// * `app_state` - The application state containing all managers
    /// * `mcp_manager` - Optional MCP manager
    /// * `app_handle` - Optional Tauri app handle for event emission
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
    /// * `circuit_breaker` - Optional circuit breaker for execution resilience
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
            current_message_id: None,
        }
    }

    /// Returns a new context with the cancellation token replaced.
    ///
    /// This is used by `LLMAgent::execute_with_mcp` to inject the workflow's
    /// cancellation token into the context before passing it to the tool factory.
    /// This ensures sub-agent tools receive the token and can propagate cancellation.
    ///
    /// # Arguments
    /// * `token` - The cancellation token to set
    ///
    /// # Example
    /// ```ignore
    /// let ctx_with_token = context.with_cancellation_token(token.clone());
    /// // ctx_with_token.cancellation_token is now Some(token)
    /// ```
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancellation_token = Some(token);
        self
    }

    /// Returns a new context with `current_message_id` set.
    ///
    /// Used by `tool_loop::execute_with_tools` to propagate the agent's
    /// pre-allocated assistant message_id from `task.context["message_id"]`
    /// down to sub-agent tools so they can persist `parent_message_id` on
    /// `sub_agent_execution` records at CREATE time (H2 audit 2026-05-02).
    pub fn with_current_message_id(mut self, message_id: String) -> Self {
        self.current_message_id = Some(message_id);
        self
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
#[path = "context_tests.rs"]
mod tests;
