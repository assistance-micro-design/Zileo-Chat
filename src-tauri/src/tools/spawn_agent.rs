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

//! SpawnAgentTool - Dynamic sub-agent creation and execution
//!
//! This tool allows a primary agent to spawn temporary sub-agents for parallel
//! or sequential task execution. Sub-agents receive only a prompt and execute
//! autonomously, returning a markdown report to the primary agent.
//!
//! # Sub-Agent Hierarchy Rules
//!
//! - Only the primary workflow agent can use this tool
//! - Sub-agents CANNOT spawn other sub-agents (single level only)
//! - Maximum 3 sub-agents per workflow
//! - Sub-agents only receive the prompt, no shared context/memory/state
//!
//! # Communication Pattern: "Prompt In, Report Out"
//!
//! ```text
//! Primary Agent --> [prompt string] --> Sub-Agent
//! Sub-Agent --> [markdown report + metrics] --> Primary Agent
//! ```

use crate::agents::core::agent::Task;
use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::agents::LLMAgent;
use crate::db::DBClient;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::models::streaming::SubAgentOperationType;
use crate::models::sub_agent::{constants::MAX_SUB_AGENTS, SubAgentSpawnResult, SubAgentStatus};
use crate::models::{AgentConfig, LLMConfig, Lifecycle};
use crate::tools::{
    constants::sub_agent::TASK_DESC_TRUNCATE_CHARS,
    context::AgentToolContext,
    sub_agent_executor::SubAgentExecutor,
    validation_helper::{safe_truncate, ValidationHelper},
    Tool, ToolDefinition, ToolError, ToolFactory, ToolResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Default system prompt for sub-agents when none is provided
const DEFAULT_SUB_AGENT_SYSTEM_PROMPT: &str = r#"You are a specialized sub-agent executing a specific task.

Your task is provided in the user message. Execute it thoroughly and return a detailed markdown report.

Guidelines:
- Focus only on the task described in the prompt
- Use available tools as needed to complete the task
- Return a structured markdown report with your findings
- Include a summary section at the top
- Be thorough but concise"#;

/// Tracked spawned child for this workflow
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnedChild {
    /// Child agent ID
    pub id: String,
    /// Child agent name
    pub name: String,
    /// Task description sent to child
    pub task_description: String,
    /// Current status
    pub status: SubAgentStatus,
    /// Execution record ID in database
    pub execution_id: String,
}

/// Tool for spawning temporary sub-agents.
///
/// This tool allows the primary workflow agent to create temporary sub-agents
/// that execute tasks and return reports. The sub-agents are automatically
/// cleaned up after execution.
///
/// # Operations
///
/// - `spawn`: Create and execute a temporary sub-agent
/// - `list_children`: List currently spawned sub-agents
/// - `terminate`: Force-stop a spawned sub-agent
///
/// # Constraints
///
/// - Only available to the primary workflow agent
/// - Maximum 3 sub-agents per workflow
/// - Sub-agents cannot spawn other sub-agents
pub struct SpawnAgentTool {
    /// Database client for persistence
    db: Arc<DBClient>,
    /// Agent registry for agent management
    registry: Arc<AgentRegistry>,
    /// Agent orchestrator for execution
    orchestrator: Arc<AgentOrchestrator>,
    /// LLM provider manager
    llm_manager: Arc<ProviderManager>,
    /// MCP manager for tool routing (optional)
    mcp_manager: Option<Arc<MCPManager>>,
    /// Tool factory for creating tools for sub-agents
    tool_factory: Arc<ToolFactory>,
    /// Tauri app handle for event emission (optional, for validation)
    app_handle: Option<AppHandle>,
    /// Cancellation token for graceful shutdown (OPT-SA-7)
    cancellation_token: Option<CancellationToken>,
    /// Parent agent ID
    parent_agent_id: String,
    /// Workflow ID
    workflow_id: String,
    /// Whether this tool is for the primary agent (true) or a sub-agent (false)
    is_primary_agent: bool,
    /// Tracked spawned children for this workflow
    spawned_children: Arc<RwLock<Vec<SpawnedChild>>>,
}

impl SpawnAgentTool {
    /// Creates a new SpawnAgentTool.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `context` - Agent tool context with system dependencies (includes cancellation token)
    /// * `parent_agent_id` - ID of the parent agent using this tool
    /// * `workflow_id` - Workflow ID for scoping
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Cancellation Token (OPT-SA-7)
    ///
    /// The cancellation token is extracted from the `AgentToolContext`. If provided,
    /// sub-agents spawned by this tool will monitor the token and abort execution
    /// when cancellation is requested.
    ///
    /// # Example
    /// ```ignore
    /// let tool = SpawnAgentTool::new(
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
        parent_agent_id: String,
        workflow_id: String,
        is_primary_agent: bool,
    ) -> Self {
        Self {
            db,
            registry: context.registry,
            orchestrator: context.orchestrator,
            llm_manager: context.llm_manager,
            mcp_manager: context.mcp_manager,
            tool_factory: context.tool_factory,
            app_handle: context.app_handle,
            cancellation_token: context.cancellation_token,
            parent_agent_id,
            workflow_id,
            is_primary_agent,
            spawned_children: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Spawns a temporary sub-agent and executes a task.
    ///
    /// # Arguments
    /// * `name` - Name for the sub-agent
    /// * `prompt` - Complete prompt for the sub-agent (only input it receives)
    /// * `system_prompt` - Optional custom system prompt
    /// * `tools` - Optional tools list (defaults to parent's tools without sub-agent tools)
    /// * `mcp_servers` - Optional MCP servers (defaults to parent's)
    /// * `provider` - Optional LLM provider (defaults to parent's)
    /// * `model` - Optional model ID (defaults to parent's)
    #[allow(clippy::too_many_arguments)]
    #[instrument(skip(self), fields(
        parent_agent_id = %self.parent_agent_id,
        workflow_id = %self.workflow_id,
        sub_agent_name = %name
    ))]
    async fn spawn(
        &self,
        name: &str,
        prompt: &str,
        system_prompt: Option<&str>,
        tools: Option<Vec<String>>,
        mcp_servers: Option<Vec<String>>,
        provider: Option<&str>,
        model: Option<&str>,
    ) -> ToolResult<Value> {
        // 1. Check if this agent is the primary (workflow starter)
        SubAgentExecutor::check_primary_permission(self.is_primary_agent, "spawn sub-agents")?;

        // 2. Check sub-agent limit
        let current_count = self.spawned_children.read().await.len();
        SubAgentExecutor::check_limit(current_count, "spawn")?;

        // 3. Validate inputs
        if name.trim().is_empty() {
            return Err(ToolError::ValidationFailed(
                "Sub-agent name cannot be empty".to_string(),
            ));
        }

        if prompt.trim().is_empty() {
            return Err(ToolError::ValidationFailed(
                "Prompt cannot be empty. The prompt is the only input the sub-agent receives."
                    .to_string(),
            ));
        }

        // 4. Validate tool names if provided
        if let Some(ref tool_list) = tools {
            let invalid_tools: Vec<&String> = tool_list
                .iter()
                .filter(|t| !ToolFactory::is_valid_tool(t))
                .collect();

            if !invalid_tools.is_empty() {
                let available = ToolFactory::basic_tools().join(", ");
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid tool(s) specified: {:?}. Available tools for sub-agents: {}. \
                     Note: Sub-agents cannot use SpawnAgentTool, DelegateTaskTool, or ParallelTasksTool.",
                    invalid_tools, available
                )));
            }

            // Also reject sub-agent tools even if they pass is_valid_tool()
            let sub_agent_tools = ToolFactory::sub_agent_tools();
            let forbidden_tools: Vec<&String> = tool_list
                .iter()
                .filter(|t| sub_agent_tools.contains(&t.as_str()))
                .collect();

            if !forbidden_tools.is_empty() {
                return Err(ToolError::ValidationFailed(format!(
                    "Sub-agents cannot use sub-agent tools: {:?}. These tools are only available to the primary workflow agent.",
                    forbidden_tools
                )));
            }
        }

        // 4b. Validate MCP server names if provided
        if let Some(ref mcp_servers_list) = mcp_servers {
            if !mcp_servers_list.is_empty() {
                if let Some(ref mcp_mgr) = self.mcp_manager {
                    if let Err(invalid) = mcp_mgr.validate_server_names(mcp_servers_list).await {
                        return Err(ToolError::ValidationFailed(format!(
                            "Unknown MCP server(s): {:?}. Available servers: {:?}",
                            invalid,
                            mcp_mgr.server_names().await
                        )));
                    }
                }
            }
        }

        // 5. Request human-in-the-loop validation
        // OPT-SA-7: Create executor with cancellation token for graceful shutdown
        let executor = SubAgentExecutor::with_cancellation(
            self.db.clone(),
            self.orchestrator.clone(),
            self.mcp_manager.clone(),
            self.app_handle.clone(),
            self.workflow_id.clone(),
            self.parent_agent_id.clone(),
            self.cancellation_token.clone(),
        );

        let details = ValidationHelper::spawn_details(
            name,
            prompt,
            &tools.clone().unwrap_or_default(),
            &mcp_servers.clone().unwrap_or_default(),
        );

        executor
            .request_validation(
                SubAgentOperationType::Spawn,
                &format!("Spawn sub-agent '{}' to execute task", name),
                details,
            )
            .await?;

        // 6. Get parent agent config for defaults
        let parent_config = self
            .registry
            .get(&self.parent_agent_id)
            .await
            .map(|agent| agent.config().clone())
            .ok_or_else(|| {
                ToolError::DependencyError(format!(
                    "Parent agent '{}' not found in registry",
                    self.parent_agent_id
                ))
            })?;

        // 7. Generate sub-agent ID
        let sub_agent_id = SubAgentExecutor::generate_sub_agent_id();

        // 9. Build sub-agent configuration
        // Filter out sub-agent tools from available tools (sub-agents cannot spawn others)
        let parent_tools = tools.unwrap_or_else(|| parent_config.tools.clone());
        let sub_agent_tools: Vec<String> = parent_tools
            .into_iter()
            .filter(|t| !ToolFactory::requires_context(t))
            .collect();

        let sub_agent_config = AgentConfig {
            id: sub_agent_id.clone(),
            name: name.to_string(),
            lifecycle: Lifecycle::Temporary,
            llm: LLMConfig {
                provider: provider.unwrap_or(&parent_config.llm.provider).to_string(),
                model: model.unwrap_or(&parent_config.llm.model).to_string(),
                temperature: parent_config.llm.temperature,
                max_tokens: parent_config.llm.max_tokens,
            },
            tools: sub_agent_tools,
            mcp_servers: mcp_servers.unwrap_or_else(|| parent_config.mcp_servers.clone()),
            system_prompt: system_prompt
                .unwrap_or(DEFAULT_SUB_AGENT_SYSTEM_PROMPT)
                .to_string(),
            // Sub-agents inherit parent's max_tool_iterations and enable_thinking
            max_tool_iterations: parent_config.max_tool_iterations,
            enable_thinking: parent_config.enable_thinking,
        };

        info!(
            sub_agent_id = %sub_agent_id,
            name = %name,
            tools_count = sub_agent_config.tools.len(),
            mcp_servers_count = sub_agent_config.mcp_servers.len(),
            "Creating sub-agent"
        );

        // 10. Create execution record in database (status: running)
        let execution_id = executor
            .create_execution_record(&sub_agent_id, name, prompt)
            .await?;

        // 11. Create LLMAgent instance for sub-agent
        let sub_agent = LLMAgent::with_factory(
            sub_agent_config.clone(),
            self.llm_manager.clone(),
            self.tool_factory.clone(),
        );

        // 12. Register in registry
        self.registry
            .register(sub_agent_id.clone(), Arc::new(sub_agent))
            .await;

        // 13. Track spawned child
        let spawned_child = SpawnedChild {
            id: sub_agent_id.clone(),
            name: name.to_string(),
            task_description: prompt.to_string(),
            status: SubAgentStatus::Running,
            execution_id: execution_id.clone(),
        };
        self.spawned_children.write().await.push(spawned_child);

        // 13b. Emit sub_agent_start event
        executor.emit_start_event(&sub_agent_id, name, prompt);

        // 14. Create task for sub-agent
        let task = Task {
            id: format!("task_{}", Uuid::new_v4()),
            description: prompt.to_string(),
            context: serde_json::json!({
                "workflow_id": self.workflow_id,
                "parent_agent_id": self.parent_agent_id,
                "is_sub_agent": true
            }),
        };

        // 15. Execute sub-agent with retry and heartbeat monitoring (OPT-SA-1, OPT-SA-10)
        let exec_result = executor
            .execute_with_retry(&sub_agent_id, task, None)
            .await;

        // 16. Emit completion or error event
        executor.emit_complete_event(&sub_agent_id, name, &exec_result);

        // 17. Update execution record
        executor
            .update_execution_record(&execution_id, &exec_result)
            .await;

        // 18. Update spawned children status
        {
            let mut children = self.spawned_children.write().await;
            if let Some(child) = children.iter_mut().find(|c| c.id == sub_agent_id) {
                child.status = if exec_result.success {
                    SubAgentStatus::Completed
                } else {
                    SubAgentStatus::Error
                };
            }
        }

        // 19. Cleanup: unregister sub-agent from registry
        if let Err(e) = self.registry.unregister(&sub_agent_id).await {
            warn!(
                sub_agent_id = %sub_agent_id,
                error = %e,
                "Failed to unregister sub-agent"
            );
        }

        info!(
            sub_agent_id = %sub_agent_id,
            success = exec_result.success,
            duration_ms = exec_result.metrics.duration_ms,
            "Sub-agent execution completed"
        );

        // 20. Return result
        let result = SubAgentSpawnResult {
            success: exec_result.success,
            child_id: sub_agent_id,
            report: exec_result.report,
            metrics: exec_result.metrics,
        };

        serde_json::to_value(&result)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to serialize result: {}", e)))
    }

    /// Lists currently spawned sub-agents for this workflow.
    #[instrument(skip(self), fields(workflow_id = %self.workflow_id))]
    async fn list_children(&self) -> ToolResult<Value> {
        let children = self.spawned_children.read().await;

        debug!(count = children.len(), "Listing spawned children");

        Ok(serde_json::json!({
            "success": true,
            "count": children.len(),
            "max_allowed": MAX_SUB_AGENTS,
            "remaining_slots": MAX_SUB_AGENTS.saturating_sub(children.len()),
            "children": children.iter().map(|c| serde_json::json!({
                "id": c.id,
                "name": c.name,
                "status": c.status.to_string(),
                "task_description": safe_truncate(&c.task_description, TASK_DESC_TRUNCATE_CHARS, true)
            })).collect::<Vec<_>>()
        }))
    }

    /// Terminates a spawned sub-agent.
    ///
    /// Note: This only marks the agent as cancelled in tracking.
    /// Actual execution cancellation is not yet implemented.
    #[instrument(skip(self), fields(workflow_id = %self.workflow_id, child_id = %child_id))]
    async fn terminate(&self, child_id: &str) -> ToolResult<Value> {
        // Check if this agent is the primary
        if !self.is_primary_agent {
            return Err(ToolError::PermissionDenied(
                "Only the primary workflow agent can terminate sub-agents.".to_string(),
            ));
        }

        // Find and update child
        let mut children = self.spawned_children.write().await;
        let child = children
            .iter_mut()
            .find(|c| c.id == child_id)
            .ok_or_else(|| {
                ToolError::NotFound(format!(
                    "Sub-agent '{}' not found. Use list_children to see available sub-agents.",
                    child_id
                ))
            })?;

        // Check if already terminal
        if matches!(
            child.status,
            SubAgentStatus::Completed | SubAgentStatus::Error | SubAgentStatus::Cancelled
        ) {
            return Err(ToolError::ValidationFailed(format!(
                "Sub-agent '{}' is already in terminal state: {}",
                child_id, child.status
            )));
        }

        // Mark as cancelled
        child.status = SubAgentStatus::Cancelled;

        // Update database record
        let update_query = format!(
            "UPDATE sub_agent_execution:`{}` SET \
             status = 'cancelled', \
             error_message = 'Terminated by parent agent', \
             completed_at = time::now()",
            child.execution_id
        );

        if let Err(e) = self.db.execute(&update_query).await {
            warn!(
                execution_id = %child.execution_id,
                error = %e,
                "Failed to update execution record for termination"
            );
        }

        // Attempt to unregister from registry
        if let Err(e) = self.registry.unregister(child_id).await {
            debug!(
                child_id = %child_id,
                error = %e,
                "Could not unregister terminated agent (may have already completed)"
            );
        }

        info!(child_id = %child_id, "Sub-agent terminated");

        Ok(serde_json::json!({
            "success": true,
            "child_id": child_id,
            "message": format!("Sub-agent '{}' has been terminated", child_id)
        }))
    }
}

#[async_trait]
impl Tool for SpawnAgentTool {
    fn definition(&self) -> ToolDefinition {
        // Get the list of available tools for documentation
        let available_tools: Vec<&str> = ToolFactory::basic_tools();
        let available_tools_str = available_tools.join(", ");

        ToolDefinition {
            id: "SpawnAgentTool".to_string(),
            name: "Spawn Sub-Agent".to_string(),
            description: format!(
                r#"Spawns temporary sub-agents to execute tasks in parallel or sequence.

USE THIS TOOL WHEN:
- You need to parallelize work across multiple specialized tasks
- A task requires different tools or context than your current configuration
- You want to delegate a specific analysis or research task

IMPORTANT CONSTRAINTS:
- Maximum 3 sub-agents per workflow
- Sub-agents CANNOT spawn other sub-agents (single level only)
- Sub-agents only receive the prompt string - NO shared context/memory/state
- You must include ALL necessary information in the prompt
- Sub-agents are TEMPORARY and are automatically cleaned up after execution
- Sub-agents do NOT appear in the Settings agent list (they are workflow-scoped)

AVAILABLE TOOLS FOR SUB-AGENTS: {available_tools_str}
Note: Sub-agents can only use basic tools listed above, NOT sub-agent tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool).
Do NOT invent or specify tools that are not in this list.

AVAILABLE MCP SERVERS: See the "Available MCP Servers" section in your configuration above.
You can assign any of these running MCP servers to sub-agents using the `mcp_servers` parameter with the server ID.

COMMUNICATION PATTERN:
- You send: A complete prompt with task, data, and expected report format
- Sub-agent returns: A markdown report with findings and metrics

OPERATIONS:
- spawn: Create and execute a temporary sub-agent
  Required: name, prompt
  Optional: system_prompt, tools (from available list above), mcp_servers (from list above), provider, model

- list_children: See your spawned sub-agents and remaining slots

- terminate: Cancel a running sub-agent

PROMPT BEST PRACTICES:
1. Be explicit about the task objective
2. Include any data the sub-agent needs (it has no access to your context)
3. Specify the expected report format
4. Set clear constraints if any

EXAMPLE - Spawn for analysis:
{{"operation": "spawn", "name": "CodeAnalyzer", "prompt": "Analyze the database module for security issues. Focus on SQL injection, input validation, and access control. Return a markdown report with: 1) Summary of findings, 2) Detailed issues with severity ratings, 3) Recommended fixes.", "tools": ["MemoryTool"]}}"#,
                available_tools_str = available_tools_str
            ),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["spawn", "list_children", "terminate"],
                        "description": "The operation to perform"
                    },
                    "name": {
                        "type": "string",
                        "description": "Sub-agent name (for spawn)"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "COMPLETE prompt for sub-agent. Must include task, any data needed, and expected report format. This is the ONLY input the sub-agent receives."
                    },
                    "system_prompt": {
                        "type": "string",
                        "description": "Custom system prompt (optional, overrides default)"
                    },
                    "tools": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tools for sub-agent (default: parent's tools without sub-agent tools)"
                    },
                    "mcp_servers": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "MCP servers (default: parent's)"
                    },
                    "provider": {
                        "type": "string",
                        "description": "LLM provider (default: parent's)"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model ID (default: parent's)"
                    },
                    "child_id": {
                        "type": "string",
                        "description": "Child agent ID (for terminate)"
                    }
                },
                "required": ["operation"]
            }),

            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "success": {"type": "boolean"},
                    "child_id": {"type": "string"},
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
                    "children": {"type": "array"},
                    "message": {"type": "string"}
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

        debug!(operation = %operation, "Executing SpawnAgentTool");

        match operation {
            "spawn" => {
                let name = input["name"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'name' for spawn operation".to_string())
                })?;
                let prompt = input["prompt"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'prompt' for spawn operation".to_string())
                })?;
                let system_prompt = input["system_prompt"].as_str();
                let tools: Option<Vec<String>> = input["tools"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
                let mcp_servers: Option<Vec<String>> = input["mcp_servers"].as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });
                let provider = input["provider"].as_str();
                let model = input["model"].as_str();

                self.spawn(
                    name,
                    prompt,
                    system_prompt,
                    tools,
                    mcp_servers,
                    provider,
                    model,
                )
                .await
            }

            "list_children" => self.list_children().await,

            "terminate" => {
                let child_id = input["child_id"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Missing 'child_id' for terminate operation".to_string(),
                    )
                })?;

                self.terminate(child_id).await
            }

            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: spawn, list_children, terminate",
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
            "spawn" => {
                if input.get("name").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'name' for spawn operation".to_string(),
                    ));
                }
                if input.get("prompt").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'prompt' for spawn operation. The prompt is the only input \
                         the sub-agent receives - include all necessary context."
                            .to_string(),
                    ));
                }
            }
            "list_children" => {
                // No required params
            }
            "terminate" => {
                if input.get("child_id").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'child_id' for terminate operation".to_string(),
                    ));
                }
            }
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown operation: '{}'. Valid operations: spawn, list_children, terminate",
                    operation
                )));
            }
        }

        Ok(())
    }

    fn requires_confirmation(&self) -> bool {
        // Sub-agent operations do not require confirmation by default
        // Validation can be added via the validation system if needed
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        // Verify definition has required fields
        let definition = ToolDefinition {
            id: "SpawnAgentTool".to_string(),
            name: "Spawn Sub-Agent".to_string(),
            description: "Test".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "SpawnAgentTool");
        assert!(!definition.requires_confirmation);
    }

    #[test]
    fn test_spawned_child_serialization() {
        let child = SpawnedChild {
            id: "sub_123".to_string(),
            name: "Test Agent".to_string(),
            task_description: "Analyze something".to_string(),
            status: SubAgentStatus::Running,
            execution_id: "exec_456".to_string(),
        };

        let json = serde_json::to_string(&child).unwrap();
        assert!(json.contains("sub_123"));
        assert!(json.contains("Test Agent"));
        assert!(json.contains("running"));
    }

    #[test]
    fn test_input_validation_spawn() {
        let valid_input = serde_json::json!({
            "operation": "spawn",
            "name": "AnalysisAgent",
            "prompt": "Analyze the code for bugs"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "spawn");
        assert!(valid_input.get("name").is_some());
        assert!(valid_input.get("prompt").is_some());
    }

    #[test]
    fn test_input_validation_terminate() {
        let valid_input = serde_json::json!({
            "operation": "terminate",
            "child_id": "sub_abc123"
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("child_id").is_some());
    }

    #[test]
    fn test_input_validation_list() {
        let valid_input = serde_json::json!({
            "operation": "list_children"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "list_children");
    }

    #[test]
    fn test_max_sub_agents_constant() {
        assert_eq!(MAX_SUB_AGENTS, 3);
    }

    #[test]
    fn test_default_system_prompt() {
        // Verify the default system prompt has meaningful content
        assert!(DEFAULT_SUB_AGENT_SYSTEM_PROMPT.len() > 50);
        assert!(DEFAULT_SUB_AGENT_SYSTEM_PROMPT.contains("sub-agent"));
    }
}
