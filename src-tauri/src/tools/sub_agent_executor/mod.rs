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

//! Common execution logic for sub-agent tools.
//!
//! This module centralizes duplicated code across SpawnAgentTool, DelegateTaskTool,
//! and ParallelTasksTool to provide a unified interface for:
//! - Permission checks (primary agent only)
//! - Limit validation (MAX_SUB_AGENTS)
//! - Execution record lifecycle (create, update)
//! - Event emission (streaming)
//! - Metrics collection
//!
//! # Architecture
//!
//! The module is split into focused submodules:
//! - [`activity_monitor`] - Heartbeat-based inactivity detection
//! - [`execution`] - Execution engine with retry and cancellation
//! - [`records`] - DB record persistence and event emission
//!
//! # Usage
//!
//! ```ignore
//! let executor = SubAgentExecutor::with_cancellation(
//!     db, orchestrator, mcp_manager, app_handle,
//!     workflow_id, parent_agent_id,
//!     Some(cancellation_token),
//! );
//!
//! // Check permissions and limits
//! SubAgentExecutor::check_primary_permission(is_primary, "spawn")?;
//! SubAgentExecutor::check_limit(current_count, "spawn")?;
//!
//! // Create execution record
//! let execution_id = executor.create_execution_record(
//!     &sub_agent_id, "Sub-Agent Name", "Task prompt"
//! ).await?;
//!
//! // Execute with retry and heartbeat monitoring
//! let result = executor.execute_with_retry(&sub_agent_id, task, None).await;
//!
//! // Update record and emit events
//! executor.update_execution_record(&execution_id, &result).await;
//! executor.emit_complete_event(&sub_agent_id, "Sub-Agent Name", &result);
//! ```

pub mod activity_monitor;
mod execution;
mod execution_retry;
mod records;

use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::agents::core::agent::{ReasoningStepData, ToolExecutionData};
use crate::agents::core::orchestrator::AgentOrchestrator;
use crate::db::DBClient;
use crate::mcp::manager::MCPManager;
use crate::models::sub_agent::{constants::MAX_SUB_AGENTS, SubAgentMetrics};
use crate::tools::{ToolError, ToolResult};

// Re-exports for external consumers

/// Result of sub-agent execution with metrics.
#[derive(Debug)]
pub struct ExecutionResult {
    /// Whether execution succeeded
    pub success: bool,
    /// Markdown report from sub-agent
    pub report: String,
    /// Execution metrics
    pub metrics: SubAgentMetrics,
    /// Error message if failed
    pub error_message: Option<String>,
    /// Internal tool executions from sub-agent
    pub tool_executions: Vec<ToolExecutionData>,
    /// Internal reasoning steps from sub-agent
    pub reasoning_steps: Vec<ReasoningStepData>,
}

impl Default for ExecutionResult {
    fn default() -> Self {
        Self {
            success: false,
            report: String::new(),
            metrics: SubAgentMetrics {
                duration_ms: 0,
                tokens_input: 0,
                tokens_output: 0,
                cached_tokens: None,
                cache_write_tokens: None,
                thinking_tokens: None,
                cost_usd: None,
            },
            error_message: None,
            tool_executions: Vec::new(),
            reasoning_steps: Vec::new(),
        }
    }
}

/// Common executor for sub-agent operations.
///
/// Centralizes shared logic across SpawnAgentTool, DelegateTaskTool, and ParallelTasksTool
/// to reduce code duplication and ensure consistent behavior.
///
/// # Cancellation Support
///
/// The executor supports graceful cancellation via `CancellationToken`. When a token
/// is provided and cancelled, execution aborts immediately with a "cancelled" result.
pub struct SubAgentExecutor {
    /// Database client for execution record management
    pub(crate) db: Arc<DBClient>,
    /// Orchestrator for agent execution
    pub(crate) orchestrator: Arc<AgentOrchestrator>,
    /// Optional MCP manager for tool routing
    pub(crate) mcp_manager: Option<Arc<MCPManager>>,
    /// Optional app handle for event emission
    pub(crate) app_handle: Option<tauri::AppHandle>,
    /// Workflow ID for scoping
    pub(crate) workflow_id: String,
    /// Parent agent ID (caller of sub-agent tools)
    pub(crate) parent_agent_id: String,
    /// Optional cancellation token for graceful shutdown
    pub(crate) cancellation_token: Option<CancellationToken>,
    /// Assistant message_id of the spawning agent.
    ///
    /// Persisted on the new `sub_agent_execution` record's `parent_message_id`
    /// at CREATE time (H2 audit 2026-05-02). `None` for callers that do not
    /// have message-level correlation (e.g. legacy tests).
    pub(crate) parent_message_id: Option<String>,
}

impl SubAgentExecutor {
    /// Creates a new executor with cancellation token support.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `orchestrator` - Agent orchestrator for execution
    /// * `mcp_manager` - Optional MCP manager for tool routing
    /// * `app_handle` - Optional app handle for event emission
    /// * `workflow_id` - Workflow ID for scoping
    /// * `parent_agent_id` - ID of parent agent calling sub-agent tools
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
    pub fn with_cancellation(
        db: Arc<DBClient>,
        orchestrator: Arc<AgentOrchestrator>,
        mcp_manager: Option<Arc<MCPManager>>,
        app_handle: Option<tauri::AppHandle>,
        workflow_id: String,
        parent_agent_id: String,
        cancellation_token: Option<CancellationToken>,
    ) -> Self {
        Self {
            db,
            orchestrator,
            mcp_manager,
            app_handle,
            workflow_id,
            parent_agent_id,
            cancellation_token,
            parent_message_id: None,
        }
    }

    /// Sets the spawning agent's assistant message_id.
    ///
    /// Used downstream when building the `sub_agent_execution` record so
    /// `parent_message_id` is set at CREATE time (replaces the legacy bulk
    /// UPDATE in `persistence_step.rs` — H2 audit 2026-05-02).
    pub fn with_parent_message(mut self, parent_message_id: Option<String>) -> Self {
        self.parent_message_id = parent_message_id;
        self
    }

    /// Checks that the caller is the primary agent.
    ///
    /// Sub-agents cannot use sub-agent tools (single level hierarchy).
    ///
    /// # Arguments
    /// * `is_primary` - Whether the caller is the primary agent
    /// * `operation` - Operation name for error message
    ///
    /// # Returns
    /// * `Ok(())` - If caller is primary
    /// * `Err(ToolError::PermissionDenied)` - If caller is sub-agent
    pub fn check_primary_permission(is_primary: bool, operation: &str) -> ToolResult<()> {
        if !is_primary {
            return Err(ToolError::PermissionDenied(format!(
                "Only the primary workflow agent can {}. Sub-agents cannot use this operation.",
                operation
            )));
        }
        Ok(())
    }

    /// Checks the sub-agent limit.
    ///
    /// Maximum 3 sub-agent operations per workflow.
    ///
    /// # Arguments
    /// * `current_count` - Current number of active sub-agents
    /// * `operation` - Operation name for error message
    ///
    /// # Returns
    /// * `Ok(())` - If under limit
    /// * `Err(ToolError::ValidationFailed)` - If limit exceeded
    pub fn check_limit(current_count: usize, operation: &str) -> ToolResult<()> {
        if current_count >= MAX_SUB_AGENTS {
            return Err(ToolError::ValidationFailed(format!(
                "Maximum {} sub-agent operations reached for {}. Current: {}. Complete existing operations first.",
                MAX_SUB_AGENTS, operation, current_count
            )));
        }
        Ok(())
    }

    /// Generates a unique sub-agent ID.
    ///
    /// # Returns
    /// * `String` - Generated ID with "sub_" prefix
    pub fn generate_sub_agent_id() -> String {
        format!("sub_{}", Uuid::new_v4())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_primary_permission_allowed() {
        let result = SubAgentExecutor::check_primary_permission(true, "spawn");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_primary_permission_denied() {
        let result = SubAgentExecutor::check_primary_permission(false, "spawn");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::PermissionDenied(msg) => {
                assert!(msg.contains("Only the primary"));
                assert!(msg.contains("spawn"));
            }
            _ => panic!("Expected PermissionDenied error"),
        }
    }

    #[test]
    fn test_check_limit_ok() {
        let result = SubAgentExecutor::check_limit(2, "spawn");
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_limit_exceeded() {
        let result = SubAgentExecutor::check_limit(MAX_SUB_AGENTS, "spawn");
        assert!(result.is_err());
        match result.unwrap_err() {
            ToolError::ValidationFailed(msg) => {
                assert!(msg.contains("Maximum"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_generate_sub_agent_id() {
        let id1 = SubAgentExecutor::generate_sub_agent_id();
        let id2 = SubAgentExecutor::generate_sub_agent_id();
        assert!(id1.starts_with("sub_"));
        assert!(id2.starts_with("sub_"));
        assert_ne!(id1, id2);
    }
}
