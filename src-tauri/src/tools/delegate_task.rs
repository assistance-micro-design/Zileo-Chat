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

use crate::agents::core::agent::Task;
use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::streaming::SubAgentOperationType;
use crate::models::sub_agent::{
    constants::MAX_SUB_AGENTS, DelegateResult, SubAgentExecutionComplete, SubAgentExecutionCreate,
    SubAgentStatus,
};
use crate::models::Lifecycle;
use crate::tools::context::AgentToolContext;
use crate::tools::sub_agent_executor::SubAgentExecutor;
use crate::tools::utils::sub_agent_description_template;
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

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
    db: Arc<DBClient>,
    /// Agent registry for agent lookup
    registry: Arc<AgentRegistry>,
    /// Agent orchestrator for execution
    orchestrator: Arc<AgentOrchestrator>,
    /// MCP manager for tool routing (optional)
    mcp_manager: Option<Arc<MCPManager>>,
    /// Tauri app handle for event emission (optional, for validation)
    app_handle: Option<AppHandle>,
    /// Cancellation token for graceful shutdown (OPT-SA-7)
    cancellation_token: Option<CancellationToken>,
    /// Current agent ID (parent agent)
    current_agent_id: String,
    /// Workflow ID
    workflow_id: String,
    /// Whether this tool is for the primary agent (true) or a sub-agent (false)
    is_primary_agent: bool,
    /// Tracked active delegations for this workflow
    active_delegations: Arc<RwLock<Vec<ActiveDelegation>>>,
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
    /// # Cancellation Token (OPT-SA-7)
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
            active_delegations: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Delegates a task to an existing permanent agent.
    ///
    /// # Arguments
    /// * `agent_id` - ID of the agent to delegate to
    /// * `prompt` - Complete prompt for the agent (only input it receives)
    #[instrument(skip(self), fields(
        current_agent_id = %self.current_agent_id,
        workflow_id = %self.workflow_id,
        target_agent_id = %agent_id
    ))]
    async fn delegate(&self, agent_id: &str, prompt: &str) -> ToolResult<Value> {
        // 1. Check if this agent is the primary (workflow starter)
        if !self.is_primary_agent {
            return Err(ToolError::PermissionDenied(
                "Only the primary workflow agent can delegate tasks. \
                 Sub-agents cannot delegate to other agents."
                    .to_string(),
            ));
        }

        // 2. Check sub-agent limit (shared with spawned agents)
        let current_count = self.active_delegations.read().await.len();
        if current_count >= MAX_SUB_AGENTS {
            return Err(ToolError::ValidationFailed(format!(
                "Maximum {} sub-agent operations reached. Cannot delegate more. \
                 Current active delegations: {}",
                MAX_SUB_AGENTS, current_count
            )));
        }

        // 3. Validate inputs
        if agent_id.trim().is_empty() {
            return Err(ToolError::ValidationFailed(
                "Agent ID cannot be empty. Use 'list_agents' to find available agents.".to_string(),
            ));
        }

        if prompt.trim().is_empty() {
            return Err(ToolError::ValidationFailed(
                "Prompt cannot be empty. The prompt is the only input the agent receives."
                    .to_string(),
            ));
        }

        // 4. Cannot delegate to self
        if agent_id == self.current_agent_id {
            return Err(ToolError::ValidationFailed(
                "Cannot delegate to yourself. Choose a different agent.".to_string(),
            ));
        }

        // 5. Look up the target agent
        let target_agent = self.registry.get(agent_id).await.ok_or_else(|| {
            ToolError::NotFound(format!(
                "Agent '{}' not found. Use 'list_agents' to see available agents.",
                agent_id
            ))
        })?;

        // 6. Verify agent is permanent (temporary agents should not be delegated to)
        if !matches!(target_agent.lifecycle(), Lifecycle::Permanent) {
            return Err(ToolError::ValidationFailed(format!(
                "Cannot delegate to temporary agent '{}'. \
                 Only permanent agents can receive delegations.",
                agent_id
            )));
        }

        let agent_name = target_agent.config().name.clone();

        // 6b. Optionally validate MCP server names configured for this agent
        // (This is informational - delegation uses the agent's existing config)
        let mcp_servers_info = target_agent.mcp_servers();
        if !mcp_servers_info.is_empty() {
            if let Some(ref mcp_mgr) = self.mcp_manager {
                if let Err(invalid) = mcp_mgr.validate_server_names(&mcp_servers_info).await {
                    warn!(
                        agent_id = %agent_id,
                        invalid_servers = ?invalid,
                        "Delegated agent has unknown MCP servers configured"
                    );
                }
            }
        }

        // 7. Request human-in-the-loop validation
        let validation_helper = ValidationHelper::new(self.db.clone(), self.app_handle.clone());
        let details = ValidationHelper::delegate_details(agent_id, &agent_name, prompt);
        let risk_level = ValidationHelper::determine_risk_level(&SubAgentOperationType::Delegate);

        validation_helper
            .request_validation(
                &self.workflow_id,
                SubAgentOperationType::Delegate,
                &format!("Delegate task to agent '{}'", agent_name),
                details,
                risk_level,
            )
            .await?;

        info!(
            agent_id = %agent_id,
            agent_name = %agent_name,
            agent_lifecycle = ?target_agent.lifecycle(),
            has_mcp_manager = self.mcp_manager.is_some(),
            "Delegating task to agent"
        );

        // 7. Create execution record ID
        let execution_id = Uuid::new_v4().to_string();

        // 8. Create execution record in database (status: running)
        // Note: DelegateTaskTool is a top-level execution, so parent_execution_id = None
        let mut execution_create = SubAgentExecutionCreate::with_parent(
            self.workflow_id.clone(),
            self.current_agent_id.clone(),
            agent_id.to_string(),
            agent_name.clone(),
            prompt.to_string(),
            None, // OPT-SA-11: No parent for top-level delegations
        );
        // Set status to running (new() defaults to pending)
        execution_create.status = "running".to_string();

        // Use db.create() which handles serialization correctly (avoids SDK enum issues)
        self.db
            .create("sub_agent_execution", &execution_id, execution_create)
            .await
            .map_err(|e| {
                ToolError::DatabaseError(format!("Failed to create execution record: {}", e))
            })?;

        // OPT-SA-11: Log execution creation with tracing ID
        debug!(
            execution_id = %execution_id,
            agent_id = %agent_id,
            workflow_id = %self.workflow_id,
            "Created delegation execution record"
        );

        // 9. Track active delegation
        let delegation = ActiveDelegation {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.clone(),
            task_description: prompt.to_string(),
            status: SubAgentStatus::Running,
            execution_id: execution_id.clone(),
        };
        self.active_delegations.write().await.push(delegation);

        // 9b. Create executor for unified event emission (OPT-SA-4)
        // OPT-SA-7: Use with_cancellation for graceful shutdown support
        let executor = SubAgentExecutor::with_cancellation(
            self.db.clone(),
            self.orchestrator.clone(),
            self.mcp_manager.clone(),
            self.app_handle.clone(),
            self.workflow_id.clone(),
            self.current_agent_id.clone(),
            self.cancellation_token.clone(),
        );

        // 9c. Emit sub_agent_start event via unified executor
        executor.emit_start_event(agent_id, &agent_name, prompt);

        // 10. Create task for agent
        let task = Task {
            id: format!("delegate_{}", Uuid::new_v4()),
            description: prompt.to_string(),
            context: serde_json::json!({
                "workflow_id": self.workflow_id,
                "delegator_agent_id": self.current_agent_id,
                "is_delegation": true
            }),
        };

        // 11. Execute via unified executor with retry and heartbeat monitoring (OPT-SA-1, OPT-SA-10)
        let exec_result = executor.execute_with_retry(agent_id, task, None).await;

        // 12. Emit sub_agent_complete or sub_agent_error event via unified executor (OPT-SA-4)
        executor.emit_complete_event(agent_id, &agent_name, &exec_result);

        // Extract values for subsequent processing
        let report = exec_result.report.clone();
        let metrics = exec_result.metrics.clone();
        let success = exec_result.success;
        let error_message = exec_result.error_message.clone();

        // 13. Update execution record
        let completion = if success {
            SubAgentExecutionComplete::success(
                metrics.duration_ms,
                Some(metrics.tokens_input),
                Some(metrics.tokens_output),
                report.clone(),
            )
        } else {
            SubAgentExecutionComplete::error(
                metrics.duration_ms,
                error_message
                    .clone()
                    .unwrap_or_else(|| "Unknown error".to_string()),
            )
        };

        let update_query = format!(
            "UPDATE sub_agent_execution:`{}` SET \
             status = '{}', \
             duration_ms = {}, \
             tokens_input = {}, \
             tokens_output = {}, \
             result_summary = {}, \
             error_message = {}, \
             completed_at = time::now()",
            execution_id,
            completion.status,
            completion.duration_ms,
            completion.tokens_input.unwrap_or(0),
            completion.tokens_output.unwrap_or(0),
            completion
                .result_summary
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_else(|_| "null".to_string()))
                .unwrap_or_else(|| "null".to_string()),
            completion
                .error_message
                .as_ref()
                .map(|s| serde_json::to_string(s).unwrap_or_else(|_| "null".to_string()))
                .unwrap_or_else(|| "null".to_string()),
        );

        if let Err(e) = self.db.execute(&update_query).await {
            warn!(
                execution_id = %execution_id,
                error = %e,
                "Failed to update execution record"
            );
        }

        // 14. Update active delegations status
        {
            let mut delegations = self.active_delegations.write().await;
            if let Some(d) = delegations.iter_mut().find(|d| d.agent_id == agent_id) {
                d.status = if success {
                    SubAgentStatus::Completed
                } else {
                    SubAgentStatus::Error
                };
            }
        }

        // OPT-SA-11: Include execution_id for hierarchical tracing
        info!(
            agent_id = %agent_id,
            execution_id = %execution_id,
            workflow_id = %self.workflow_id,
            success = success,
            duration_ms = metrics.duration_ms,
            "Delegation completed"
        );

        // 15. Return result
        let result = DelegateResult {
            success,
            agent_id: agent_id.to_string(),
            report,
            metrics,
        };

        serde_json::to_value(&result)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to serialize result: {}", e)))
    }

    /// Lists available agents for delegation.
    #[instrument(skip(self), fields(workflow_id = %self.workflow_id))]
    async fn list_agents(&self) -> ToolResult<Value> {
        let agent_ids = self.registry.list().await;

        // Filter to only permanent agents, exclude self
        let mut available: Vec<Value> = Vec::new();

        for id in agent_ids {
            // Skip self
            if id == self.current_agent_id {
                continue;
            }

            // Get agent and check if permanent
            if let Some(agent) = self.registry.get(&id).await {
                if matches!(agent.lifecycle(), Lifecycle::Permanent) {
                    let config = agent.config();
                    available.push(serde_json::json!({
                        "id": id,
                        "name": config.name,
                        "lifecycle": "permanent",
                        "tools": config.tools,
                        "mcp_servers": config.mcp_servers,
                        "capabilities": agent.capabilities()
                    }));
                }
            }
        }

        let current_delegations = self.active_delegations.read().await.len();
        let remaining_slots = MAX_SUB_AGENTS.saturating_sub(current_delegations);

        debug!(
            available_count = available.len(),
            current_delegations = current_delegations,
            remaining_slots = remaining_slots,
            "Listed available agents"
        );

        Ok(serde_json::json!({
            "success": true,
            "count": available.len(),
            "agents": available,
            "current_delegations": current_delegations,
            "remaining_slots": remaining_slots,
            "max_allowed": MAX_SUB_AGENTS
        }))
    }
}

#[async_trait]
impl Tool for DelegateTaskTool {
    fn definition(&self) -> ToolDefinition {
        let tool_specific_desc = r#"Delegates tasks to existing permanent LLM agents.

IMPORTANT: This tool is for LLM AGENTS, NOT for MCP servers!
- agent_id must be an LLM agent ID (e.g., "db_agent", "analytics_agent")
- DO NOT use MCP server IDs here (e.g., "mcp-1764345441545-7tj9p")
- To use MCP tools, call them DIRECTLY with format: server_id:tool_name (see MCP Tools section)

USE THIS TOOL WHEN:
- You need a specialized LLM agent to handle a specific task
- The task requires an agent's configuration and expertise
- Use list_agents first to see available agents

IMPORTANT CONSTRAINTS:
- Can only delegate to permanent agents (not temporary)
- Delegated agents only receive the prompt string - NO shared context/memory/state
- You must include ALL necessary information in the prompt

DIFFERENCE FROM SPAWN:
- Delegate: Uses existing agents with their configuration
- Spawn: Creates temporary agents with custom configuration

COMMUNICATION PATTERN:
- You send: A complete prompt with task, data, and expected report format
- Agent returns: A markdown report with findings and metrics

OPERATIONS:
- delegate: Execute a task via an existing permanent agent
  Required: agent_id, prompt

- list_agents: List available LLM agents for delegation (excludes self and temporary agents)

PROMPT BEST PRACTICES:
1. Be explicit about the task objective
2. Include any data the agent needs (it has no access to your context)
3. Specify the expected report format
4. Set clear constraints if any

EXAMPLE - Delegate database analysis:
{"operation": "delegate", "agent_id": "db_agent", "prompt": "Analyze the users table..."}

EXAMPLE - List available agents:
{"operation": "list_agents"}"#;

        ToolDefinition {
            id: "DelegateTaskTool".to_string(),
            name: "Delegate Task".to_string(),
            description: sub_agent_description_template(tool_specific_desc),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["delegate", "list_agents"],
                        "description": "Operation: 'delegate' executes task via permanent agent, 'list_agents' shows available LLM agents for delegation"
                    },
                    "agent_id": {
                        "type": "string",
                        "description": "Target agent ID (for delegate). Use list_agents to find available agents."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "COMPLETE prompt for the agent. Must include task, any data needed, and expected report format. This is the ONLY input the agent receives."
                    }
                },
                "required": ["operation"]
            }),

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
                let agent_id = input["agent_id"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Missing 'agent_id' for delegate operation. Use 'list_agents' to find available agents.".to_string(),
                    )
                })?;
                let prompt = input["prompt"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Missing 'prompt' for delegate operation. The prompt is the only input the agent receives.".to_string(),
                    )
                })?;

                self.delegate(agent_id, prompt).await
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
                if input.get("agent_id").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'agent_id' for delegate operation. \
                         Use 'list_agents' to find available agents."
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

    fn requires_confirmation(&self) -> bool {
        // Delegation operations do not require confirmation by default
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sub_agent::SubAgentMetrics;

    #[test]
    fn test_tool_definition() {
        let definition = ToolDefinition {
            id: "DelegateTaskTool".to_string(),
            name: "Delegate Task".to_string(),
            description: "Test".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "DelegateTaskTool");
        assert!(!definition.requires_confirmation);
    }

    #[test]
    fn test_active_delegation_serialization() {
        let delegation = ActiveDelegation {
            agent_id: "db_agent".to_string(),
            agent_name: "Database Agent".to_string(),
            task_description: "Analyze schema".to_string(),
            status: SubAgentStatus::Running,
            execution_id: "exec_456".to_string(),
        };

        let json = serde_json::to_string(&delegation).unwrap();
        assert!(json.contains("db_agent"));
        assert!(json.contains("Database Agent"));
        assert!(json.contains("running"));
    }

    #[test]
    fn test_input_validation_delegate() {
        let valid_input = serde_json::json!({
            "operation": "delegate",
            "agent_id": "db_agent",
            "prompt": "Analyze the database schema"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "delegate");
        assert!(valid_input.get("agent_id").is_some());
        assert!(valid_input.get("prompt").is_some());
    }

    #[test]
    fn test_input_validation_list() {
        let valid_input = serde_json::json!({
            "operation": "list_agents"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "list_agents");
    }

    #[test]
    fn test_delegate_result_serialization() {
        let result = DelegateResult {
            success: true,
            agent_id: "db_agent".to_string(),
            report: "# Analysis Complete\n\nFound 3 optimization opportunities.".to_string(),
            metrics: SubAgentMetrics {
                duration_ms: 1500,
                tokens_input: 200,
                tokens_output: 400,
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"agent_id\":\"db_agent\""));
        assert!(json.contains("\"duration_ms\":1500"));
    }

    #[test]
    fn test_max_sub_agents_constant() {
        assert_eq!(MAX_SUB_AGENTS, 3);
    }
}
