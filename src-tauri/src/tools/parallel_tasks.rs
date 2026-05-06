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

//! ParallelTasksTool - Parallel batch execution across multiple agents
//!
//! This tool allows a primary agent to execute multiple tasks in parallel
//! across different agents. It uses `tokio::task::JoinSet` for efficient
//! concurrent execution with per-task control and cancellation support.
//!
//! # Sub-Agent Hierarchy Rules
//!
//! - Only the primary workflow agent can use this tool
//! - Sub-agents CANNOT use parallel execution (single level only)
//! - Maximum `MAX_PARALLEL_TASKS_PER_BATCH` (3) agents in one batch
//! - Total of `MAX_SUB_AGENTS` (15) sub-agent operations per workflow (cumulative)
//! - Each agent only receives its prompt, no shared context/memory/state
//!
//! # Communication Pattern: "Prompt In, Report Out"
//!
//! ```text
//! Primary Agent --> [prompt1, prompt2, prompt3] --> [Agent1, Agent2, Agent3]
//! [Agent1, Agent2, Agent3] --> [report1, report2, report3] --> Primary Agent
//! ```
//!
//! # Performance Benefits
//!
//! - All tasks execute concurrently using `tokio::task::JoinSet`
//! - Total time is approximately the slowest agent, not sum of all
//! - Per-task control allows for future cancellation support
//! - Ideal for independent analyses that can run in parallel

use crate::agents::core::agent::Task;
use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::sub_agent::constants::{MAX_PARALLEL_TASKS_PER_BATCH, MAX_SUB_AGENTS};
use crate::tools::context::AgentToolContext;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::sub_agent_executor::SubAgentExecutor;
use crate::tools::task_bridge::extract_task_ids;
use crate::tools::utils::resolve_agent_ref;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, LazyLock};
use tauri::AppHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, instrument};

/// Cached tool definition (built once, cloned per call).
static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "ParallelTasksTool".to_string(),
    name: "Parallel Tasks".to_string(),
    summary: "Execute multiple independent tasks in parallel across agents".to_string(),
    description: ToolDescriptionBuilder::new(
        "Executes multiple tasks in parallel across different agents.",
    )
    .use_when(&[
        "You need to run multiple independent analyses simultaneously",
        "Tasks don't depend on each other and can run concurrently",
    ])
    .do_not_use(&[
        "Tasks depend on each other's results (use sequential delegation instead)",
        "You have only one task (use DelegateTaskTool instead)",
    ])
    .operations(&[(
        "execute_batch",
        // Keep this string in sync with `MAX_PARALLEL_TASKS_PER_BATCH`
        // (test_definition_text_matches_batch_constant guards it).
        "Run multiple tasks in parallel (max 3 per batch). Required: tasks (array of {agent_name or agent_id, prompt, optional task_ids})",
    )])
    .note(
        "Note: Each agent receives its prompt + any assigned tasks in context. \
         Use TodoTool to create tasks first, then pass their IDs via task_ids per task.",
    )
    .examples(&[
        serde_json::json!({
            "operation": "execute_batch",
            "tasks": [
                {"agent_name": "DB Agent", "prompt": "Analyze performance..."},
                {"agent_name": "Security Agent", "prompt": "Review API security..."}
            ]
        }),
    ])
    .primary_agent_constraint(MAX_SUB_AGENTS)
    .build(),

    input_schema: parallel_tasks_input_schema(),

    output_schema: serde_json::json!({
        "type": "object",
        "properties": {
            "success": {"type": "boolean"},
            "completed": {"type": "integer"},
            "failed": {"type": "integer"},
            "results": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "agent_id": {"type": "string"},
                        "success": {"type": "boolean"},
                        "report": {"type": "string"},
                        "error": {"type": "string"},
                        "metrics": {
                            "type": "object",
                            "properties": {
                                "duration_ms": {"type": "integer"},
                                "tokens_input": {"type": "integer"},
                                "tokens_output": {"type": "integer"}
                            }
                        }
                    }
                }
            },
            "aggregated_report": {"type": "string"}
        }
    }),

    requires_confirmation: false,
}
});

/// Task specification for parallel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelTaskSpec {
    /// Resolved agent ID (UUID) for this task
    pub agent_id: String,
    /// Resolved agent display name
    pub agent_name: String,
    /// Complete prompt for the agent
    pub prompt: String,
    /// Optional task IDs to assign to this agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_ids: Option<Vec<String>>,
}

/// Prepared execution context containing all resources needed for parallel execution.
/// Used to pass data between helper functions during execute_batch().
pub(crate) struct PreparedExecution {
    /// Unified executor for event emission and DB updates
    pub(crate) executor: SubAgentExecutor,
    /// Unique identifier for this batch
    pub(crate) batch_id: String,
    /// Execution IDs for each task (in order)
    pub(crate) execution_ids: Vec<String>,
    /// Tasks prepared for orchestrator execution
    pub(crate) orchestrator_tasks: Vec<(String, Task)>,
}

/// Validates a parallel batch size against `MAX_PARALLEL_TASKS_PER_BATCH`.
///
/// Pure function for testability. Used by both `ParallelTasksTool::validate_input`
/// (input-schema gate) and `ParallelTasksTool::validate_tasks`
/// (post-resolution gate inside `parallel_tasks_execution`).
pub(crate) fn validate_batch_size(len: usize) -> ToolResult<()> {
    if len == 0 {
        return Err(ToolError::InvalidInput(
            "'tasks' array cannot be empty".to_string(),
        ));
    }
    if len > MAX_PARALLEL_TASKS_PER_BATCH {
        return Err(ToolError::ValidationFailed(format!(
            "Maximum {} parallel tasks allowed per batch. Received {}.",
            MAX_PARALLEL_TASKS_PER_BATCH, len
        )));
    }
    Ok(())
}

/// Validates a single parallel task item: requires prompt + (agent_id OR agent_name).
///
/// Pure function for testability. Used by `ParallelTasksTool::validate_input`.
fn validate_parallel_task_item(task: &Value, index: usize) -> ToolResult<()> {
    if !task.is_object() {
        return Err(ToolError::InvalidInput(format!(
            "Task {} must be an object with 'prompt' and either 'agent_id' or 'agent_name'",
            index
        )));
    }

    let has_agent_id = task
        .get("agent_id")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty());
    let has_agent_name = task
        .get("agent_name")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty());

    if !has_agent_id && !has_agent_name {
        return Err(ToolError::InvalidInput(format!(
            "Task {} missing 'agent_id' or 'agent_name'. \
             Provide at least one. Use 'list_agents' to find available agents.",
            index
        )));
    }
    if task.get("prompt").is_none() {
        return Err(ToolError::InvalidInput(format!(
            "Task {} missing 'prompt'",
            index
        )));
    }
    Ok(())
}

/// Returns the input schema for ParallelTasksTool.
///
/// Pure function for testability. Used by `ParallelTasksTool::definition`.
fn parallel_tasks_input_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "operation": {
                "type": "string",
                "enum": ["execute_batch"],
                "description": "Operation: 'execute_batch' runs multiple tasks concurrently across different agents"
            },
            "tasks": {
                "type": "array",
                "maxItems": MAX_PARALLEL_TASKS_PER_BATCH,
                "description": format!("Array of agent-prompt pairs (max {})", MAX_PARALLEL_TASKS_PER_BATCH),
                "items": {
                    "type": "object",
                    "properties": {
                        "agent_id": {
                            "type": "string",
                            "description": "Target agent ID (UUID). Either agent_id or agent_name is required."
                        },
                        "agent_name": {
                            "type": "string",
                            "description": "Target agent name (case-insensitive). Alternative to agent_id. If both provided, agent_id takes priority."
                        },
                        "prompt": {
                            "type": "string",
                            "description": "COMPLETE prompt for this agent"
                        },
                        "task_ids": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Optional task IDs to assign to this agent"
                        }
                    },
                    "required": ["prompt"]
                }
            },
            "wait_all": {
                "type": "boolean",
                "default": true,
                "description": "Wait for all tasks to complete (currently always true)"
            }
        },
        "required": ["operation", "tasks"]
    })
}

/// Tool for parallel batch execution across multiple agents.
///
/// This tool enables efficient concurrent execution of multiple tasks
/// across different agents. All tasks run in parallel using
/// `tokio::task::JoinSet`, providing per-task control and making
/// the total execution time approximately equal to the slowest individual task.
///
/// # Operations
///
/// - `execute_batch`: Run multiple tasks in parallel (max 3 agents)
///
/// # Constraints
///
/// - Only available to the primary workflow agent
/// - Maximum 3 agents per batch
/// - All tasks run concurrently
pub struct ParallelTasksTool {
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
}

impl ParallelTasksTool {
    /// Creates a new ParallelTasksTool.
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
    /// parallel tasks will monitor the token and abort execution when cancellation
    /// is requested.
    ///
    /// # Example
    /// ```ignore
    /// let tool = ParallelTasksTool::new(
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
        }
    }
}

#[async_trait]
impl Tool for ParallelTasksTool {
    fn id(&self) -> &str {
        "ParallelTasksTool"
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

        debug!(operation = %operation, "Executing ParallelTasksTool");

        match operation {
            "execute_batch" => {
                // Parse tasks array
                let tasks_array = input["tasks"].as_array().ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Missing 'tasks' array for execute_batch operation".to_string(),
                    )
                })?;

                let mut tasks: Vec<ParallelTaskSpec> = Vec::new();
                for (i, t) in tasks_array.iter().enumerate() {
                    let agent_ref = t["agent_id"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                        .or_else(|| t["agent_name"].as_str().filter(|s| !s.trim().is_empty()))
                        .ok_or_else(|| {
                            ToolError::InvalidInput(format!(
                                "Task {} missing 'agent_id' or 'agent_name'",
                                i
                            ))
                        })?;
                    let prompt = t["prompt"]
                        .as_str()
                        .ok_or_else(|| {
                            ToolError::InvalidInput(format!("Task {} missing 'prompt'", i))
                        })?
                        .to_string();

                    // Resolve via ID or name lookup
                    let resolved_id = resolve_agent_ref(&self.registry, agent_ref).await?;

                    // Look up real agent name from registry
                    let agent_name = self
                        .registry
                        .get(&resolved_id)
                        .await
                        .map(|a| a.config().name.clone())
                        .unwrap_or_else(|| resolved_id.clone());

                    let task_ids = extract_task_ids(t);

                    tasks.push(ParallelTaskSpec {
                        agent_id: resolved_id,
                        agent_name,
                        prompt,
                        task_ids,
                    });
                }

                let wait_all = input["wait_all"].as_bool().unwrap_or(true);

                self.execute_batch(tasks, wait_all).await
            }

            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: execute_batch",
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
            "execute_batch" => {
                let tasks = input.get("tasks").ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Missing 'tasks' for execute_batch operation".to_string(),
                    )
                })?;

                let tasks_array = tasks.as_array().ok_or_else(|| {
                    ToolError::InvalidInput("'tasks' must be an array".to_string())
                })?;

                validate_batch_size(tasks_array.len())?;

                for (i, task) in tasks_array.iter().enumerate() {
                    validate_parallel_task_item(task, i)?;
                }
            }
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown operation: '{}'. Valid operations: execute_batch",
                    operation
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "parallel_tasks_tests.rs"]
mod tests;
