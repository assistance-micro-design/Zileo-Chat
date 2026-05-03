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

//! DelegateTaskTool - Task delegation to existing agents
//!
//! This tool allows a primary agent to delegate tasks to existing permanent agents.
//! Unlike SpawnAgentTool which creates temporary sub-agents, DelegateTaskTool
//! uses existing agents from the registry with their pre-configured tools and settings.
//!
//! # Sub-Agent Hierarchy Rules
//!
//! - Only the primary workflow agent can use this tool
//! - Sub-agents CANNOT delegate to other agents (single level only)
//! - Maximum 3 delegations per workflow (shared count with spawned agents)
//! - Delegated agents only receive the prompt, no shared context/memory/state
//!
//! # Communication Pattern: "Prompt In, Report Out"
//!
//! ```text
//! Primary Agent --> [prompt string] --> Delegated Agent
//! Delegated Agent --> [markdown report + metrics] --> Primary Agent
//! ```
//!
//! # Difference from SpawnAgentTool
//!
//! | Aspect | SpawnAgentTool | DelegateTaskTool |
//! |--------|----------------|------------------|
//! | Agent | Creates temporary | Uses existing permanent |
//! | Config | Can override | Uses agent's config |
//! | Cleanup | Auto-cleanup | No cleanup needed |
//! | Use case | Custom tasks | Specialized agents |

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::sub_agent::constants::MAX_SUB_AGENTS;
use crate::models::sub_agent::SubAgentStatus;
use crate::tools::context::AgentToolContext;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::task_bridge::extract_task_ids;
use crate::tools::utils::resolve_agent_ref;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

/// Cached tool definition (built once, cloned per call).
static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "DelegateTaskTool".to_string(),
    name: "Delegate Task".to_string(),
    summary: "Delegate a task to an existing permanent LLM agent".to_string(),
    description: ToolDescriptionBuilder::new(
        "Delegates tasks to existing permanent LLM agents.",
    )
    .use_when(&[
        "You need a specialized permanent agent to handle a task",
        "The task benefits from an agent's pre-configured expertise and tools",
    ])
    .do_not_use(&[
        "You need custom tools or configuration (use SpawnAgentTool instead)",
        "Simple single-step tasks that don't need agent expertise",
    ])
    .operations(&[
        (
            "delegate",
            "Execute task via permanent agent (requires agent_name or agent_id + prompt, optional task_ids)",
        ),
        ("list_agents", "Show available agents for delegation"),
    ])
    .note(
        "WARNING: LLM AGENTS ONLY - NOT MCP SERVERS:\n\
         - Use agent_name (preferred) or agent_id (UUID) to identify the target agent\n\
         - DO NOT use MCP server IDs here (e.g., \"mcp-xxx-7tj9p\")\n\
         - For MCP tools, call them DIRECTLY: server_id:tool_name",
    )
    .note(
        "Note: Delegated agents receive the prompt + any assigned tasks in context. \
         Use TodoTool to create tasks first, then pass their IDs via task_ids.",
    )
    .examples(&[
        serde_json::json!({
            "operation": "delegate",
            "agent_name": "Database Agent",
            "prompt": "Analyze the users table..."
        }),
        serde_json::json!({
            "operation": "delegate",
            "agent_name": "DB Agent",
            "prompt": "Complete these tasks",
            "task_ids": ["task_1", "task_2"]
        }),
    ])
    .primary_agent_constraint(MAX_SUB_AGENTS)
    .build(),

    input_schema: delegate_task_input_schema(),

    output_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "success": {"type": "boolean"},
            "agent_id": {"type": "string"},
            "report": {"type": "string"},
            "metrics": {
                "type": "object",
                "properties": {
                    "duration_ms": {"type": "integer"},
                    "tokens_input": {"type": "integer"},
                    "tokens_output": {"type": "integer"}
                }
            },
            "count": {"type": "integer"},
            "agents": {"type": "array"},
            "remaining_slots": {"type": "integer"}
        }
    }),

    requires_confirmation: false,
}
});

/// Tracked delegation for this workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveDelegation {
    /// Delegated agent ID
    pub agent_id: String,
    /// Agent name
    pub agent_name: String,
    /// Task description sent to agent
    pub task_description: String,
    /// Current status
    pub status: SubAgentStatus,
    /// Execution record ID in database
    pub execution_id: String,
}

/// Validates delegate operation parameters: requires prompt + (agent_id OR agent_name).
///
/// Pure function for testability. Used by `DelegateTaskTool::validate_input`.
fn validate_delegate_operation(input: &Value) -> ToolResult<()> {
    let has_agent_id = input
        .get("agent_id")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty());
    let has_agent_name = input
        .get("agent_name")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty());

    if !has_agent_id && !has_agent_name {
        return Err(ToolError::InvalidInput(
            "Missing 'agent_id' or 'agent_name' for delegate operation. \
             Provide at least one. Use 'list_agents' to find available agents."
                .to_string(),
        ));
    }
    if input.get("prompt").is_none() {
        return Err(ToolError::InvalidInput(
            "Missing 'prompt' for delegate operation. The prompt is the only input \
             the agent receives - include all necessary context."
                .to_string(),
        ));
    }

    // task_ids is optional, but if present must be a non-empty array
    if let Some(task_ids) = input.get("task_ids") {
        if let Some(arr) = task_ids.as_array() {
            if arr.is_empty() {
                return Err(ToolError::InvalidInput(
                    "task_ids array cannot be empty if provided".to_string(),
                ));
            }
        }
    }

    Ok(())
}

/// Returns the input schema for DelegateTaskTool.
///
/// Pure function for testability. Used by `DelegateTaskTool::definition`.
fn delegate_task_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["delegate", "list_agents"],
                "description": "Operation: 'delegate' executes task via permanent agent, 'list_agents' shows available LLM agents for delegation"
            },
            "agent_id": {
                "type": "string",
                "description": "Target agent ID (UUID). Use list_agents to find available agents. Either agent_id or agent_name is required."
            },
            "agent_name": {
                "type": "string",
                "description": "Target agent name (case-insensitive). Alternative to agent_id. If both are provided, agent_id takes priority."
            },
            "prompt": {
                "type": "string",
                "description": "COMPLETE prompt for the agent. Must include task, any data needed, and expected report format. This is the ONLY input the agent receives."
            },
            "task_ids": {
                "type": "array",
                "items": {"type": "string"},
                "description": "Optional task IDs to assign to the delegated agent. Tasks will be reassigned (agent_assigned updated) and their content injected into the agent's context. Use TodoTool to create tasks first."
            }
        },
        "required": ["operation"]
    })
}

/// Tool for delegating tasks to existing permanent agents.
///
/// This tool enables the primary workflow agent to delegate tasks to
/// specialized permanent agents. The delegated agent uses its own
/// configuration (tools, MCP servers, system prompt).
///
/// # Operations
///
/// - `delegate`: Execute a task via an existing permanent agent
/// - `list_agents`: List available agents for delegation
///
/// # Constraints
///
/// - Only available to the primary workflow agent
/// - Maximum 3 total sub-operations per workflow (shared with spawn)
/// - Can only delegate to permanent agents
pub struct DelegateTaskTool {
    /// Database client for persistence
    pub(crate) db: Arc<DBClient>,
    /// Agent registry for agent lookup
    pub(crate) registry: Arc<AgentRegistry>,
    /// Agent orchestrator for execution
    pub(crate) orchestrator: Arc<AgentOrchestrator>,
    /// MCP manager for tool routing (optional)
    pub(crate) mcp_manager: Option<Arc<MCPManager>>,
    /// Tauri app handle for event emission (optional, for validation)
    pub(crate) app_handle: Option<AppHandle>,
    /// Cancellation token for graceful shutdown
    pub(crate) cancellation_token: Option<CancellationToken>,
    /// Current agent ID (parent agent)
    pub(crate) current_agent_id: String,
    /// Workflow ID
    pub(crate) workflow_id: String,
    /// Whether this tool is for the primary agent (true) or a sub-agent (false)
    pub(crate) is_primary_agent: bool,
    /// Spawning agent's assistant message_id, propagated to
    /// `sub_agent_execution.parent_message_id` at CREATE time (H2 audit
    /// 2026-05-02). Pulled from `AgentToolContext::current_message_id`.
    pub(crate) parent_message_id: Option<String>,
    /// Tracked active delegations for this workflow
    pub(crate) active_delegations: Arc<RwLock<Vec<ActiveDelegation>>>,
}

impl DelegateTaskTool {
    /// Creates a new DelegateTaskTool.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `context` - Agent tool context with system dependencies (includes cancellation token)
    /// * `current_agent_id` - ID of the agent using this tool
    /// * `workflow_id` - Workflow ID for scoping
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Cancellation Token
    ///
    /// The cancellation token is extracted from the `AgentToolContext`. If provided,
    /// delegated agents will monitor the token and abort execution when cancellation
    /// is requested.
    ///
    /// # Example
    /// ```ignore
    /// let tool = DelegateTaskTool::new(
    ///     db.clone(),
    ///     context, // Contains optional cancellation_token
    ///     "primary_agent".to_string(),
    ///     "wf_001".to_string(),
    ///     true,
    /// );
    /// ```
    pub fn new(
        db: Arc<DBClient>,
        context: AgentToolContext,
        current_agent_id: String,
        workflow_id: String,
        is_primary_agent: bool,
    ) -> Self {
        Self {
            db,
            registry: context.registry,
            orchestrator: context.orchestrator,
            mcp_manager: context.mcp_manager,
            app_handle: context.app_handle,
            cancellation_token: context.cancellation_token,
            current_agent_id,
            workflow_id,
            is_primary_agent,
            parent_message_id: context.current_message_id,
            active_delegations: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Tool for DelegateTaskTool {
    fn id(&self) -> &str {
        "DelegateTaskTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    #[instrument(skip(self, input), fields(workflow_id = %self.workflow_id))]
    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation".to_string()))?;

        debug!(operation = %operation, "Executing DelegateTaskTool");

        match operation {
            "delegate" => {
                let agent_ref = input["agent_id"]
                    .as_str()
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        input["agent_name"]
                            .as_str()
                            .filter(|s| !s.trim().is_empty())
                    })
                    .ok_or_else(|| {
                        ToolError::InvalidInput(
                            "Missing 'agent_id' or 'agent_name' for delegate operation."
                                .to_string(),
                        )
                    })?;
                let prompt = input["prompt"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Missing 'prompt' for delegate operation. The prompt is the only input the agent receives.".to_string(),
                    )
                })?;

                // Resolve via ID or name lookup
                let resolved_id = resolve_agent_ref(&self.registry, agent_ref).await?;
                let task_ids = extract_task_ids(&input);
                self.delegate(&resolved_id, prompt, task_ids).await
            }

            "list_agents" => self.list_agents().await,

            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: delegate, list_agents",
                operation
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing 'operation' field".to_string()))?;

        match operation {
            "delegate" => {
                validate_delegate_operation(input)?;
            }
            "list_agents" => {
                // No required params
            }
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown operation: '{}'. Valid operations: delegate, list_agents",
                    operation
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "delegate_task_tests.rs"]
mod tests;
