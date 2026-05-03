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

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::models::sub_agent::constants::MAX_SUB_AGENTS;
use crate::models::sub_agent::SubAgentStatus;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{
    context::AgentToolContext, Tool, ToolDefinition, ToolError, ToolFactory, ToolResult,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

/// Cached tool definition (built once, cloned per call).
///
/// `ToolFactory::basic_tools()` is read once at first access. The list is
/// `Vec<&'static str>` so the formatted string is stable across the app's
/// lifetime — providing a stable cache prefix for LLM providers.
static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    let available_tools_str = ToolFactory::basic_tools().join(", ");

    ToolDefinition {
        id: "SpawnAgentTool".to_string(),
        name: "Spawn Sub-Agent".to_string(),
        summary: "Spawn a temporary sub-agent to execute a specialized task".to_string(),
        description: ToolDescriptionBuilder::new(
            "Spawns temporary sub-agents to execute specialized tasks.",
        )
        .use_when(&[
            "You need to parallelize work across multiple specialized tasks",
            "A task requires different tools or context than your current configuration",
        ])
        .do_not_use(&[
            "Simple single-step tasks that don't benefit from delegation",
            "You need shared state or conversation context between agents",
        ])
        .operations(&[
            (
                "spawn",
                "Create and execute a temporary sub-agent (required: name, prompt; optional: system_prompt, tools, mcp_servers, provider, model)",
            ),
            ("list_children", "See spawned sub-agents and remaining slots"),
            ("terminate", "Cancel a running sub-agent"),
        ])
        .note(format!(
            "Available tools for sub-agents: {}\n\
             Note: Sub-agents receive ONLY the prompt string. Include all necessary context.",
            available_tools_str
        ))
        .examples(&[
            serde_json::json!({
                "operation": "spawn",
                "name": "Analyst",
                "prompt": "Analyze the users table schema...",
                "tools": ["MemoryTool"]
            }),
            serde_json::json!({"operation": "list_children"}),
        ])
        .primary_agent_constraint(MAX_SUB_AGENTS)
        .build(),

        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["spawn", "list_children", "terminate"],
                    "description": "Operation: 'spawn' creates temporary sub-agent, 'list_children' shows spawned agents and slots, 'terminate' cancels running sub-agent"
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
});

/// Optional configuration overrides for spawning a sub-agent.
///
/// Groups optional parameters to avoid too-many-arguments on `spawn()`.
#[derive(Debug, Default)]
pub(crate) struct SpawnParams<'a> {
    /// Custom system prompt (overrides default)
    pub system_prompt: Option<&'a str>,
    /// Tools for sub-agent (default: parent's tools without sub-agent tools)
    pub tools: Option<Vec<String>>,
    /// MCP servers (default: parent's)
    pub mcp_servers: Option<Vec<String>>,
    /// LLM provider override
    pub provider: Option<&'a str>,
    /// Model ID override
    pub model: Option<&'a str>,
}

/// Default system prompt for sub-agents when none is provided
pub(crate) const DEFAULT_SUB_AGENT_SYSTEM_PROMPT: &str = r#"You are a specialized sub-agent executing a specific task.

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
    pub(crate) db: Arc<DBClient>,
    /// Agent registry for agent management
    pub(crate) registry: Arc<AgentRegistry>,
    /// Agent orchestrator for execution
    pub(crate) orchestrator: Arc<AgentOrchestrator>,
    /// LLM provider manager
    pub(crate) llm_manager: Arc<ProviderManager>,
    /// MCP manager for tool routing (optional)
    pub(crate) mcp_manager: Option<Arc<MCPManager>>,
    /// Tool factory for creating tools for sub-agents
    pub(crate) tool_factory: Arc<ToolFactory>,
    /// Tauri app handle for event emission (optional, for validation)
    pub(crate) app_handle: Option<AppHandle>,
    /// Cancellation token for graceful shutdown
    pub(crate) cancellation_token: Option<CancellationToken>,
    /// Parent agent ID
    pub(crate) parent_agent_id: String,
    /// Workflow ID
    pub(crate) workflow_id: String,
    /// Whether this tool is for the primary agent (true) or a sub-agent (false)
    pub(crate) is_primary_agent: bool,
    /// Spawning agent's assistant message_id, propagated to
    /// `sub_agent_execution.parent_message_id` at CREATE time (H2 audit
    /// 2026-05-02). Pulled from `AgentToolContext::current_message_id`.
    pub(crate) parent_message_id: Option<String>,
    /// Tracked spawned children for this workflow
    pub(crate) spawned_children: Arc<RwLock<Vec<SpawnedChild>>>,
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
    /// # Cancellation Token
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
            parent_message_id: context.current_message_id,
            spawned_children: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl Tool for SpawnAgentTool {
    fn id(&self) -> &str {
        "SpawnAgentTool"
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

        debug!(operation = %operation, "Executing SpawnAgentTool");

        match operation {
            "spawn" => {
                let name = input["name"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'name' for spawn operation".to_string())
                })?;
                let prompt = input["prompt"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'prompt' for spawn operation".to_string())
                })?;
                let params = SpawnParams {
                    system_prompt: input["system_prompt"].as_str(),
                    tools: input["tools"].as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    }),
                    mcp_servers: input["mcp_servers"].as_array().map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    }),
                    provider: input["provider"].as_str(),
                    model: input["model"].as_str(),
                };

                self.spawn(name, prompt, params).await
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
}

#[cfg(test)]
#[path = "spawn_agent_tests.rs"]
mod tests;
