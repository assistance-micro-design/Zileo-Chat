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
//! across different agents. It uses the orchestrator's `execute_parallel()`
//! method for efficient concurrent execution.
//!
//! # Sub-Agent Hierarchy Rules
//!
//! - Only the primary workflow agent can use this tool
//! - Sub-agents CANNOT use parallel execution (single level only)
//! - Maximum 3 agents in a batch (enforced in validation)
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
//! - All tasks execute concurrently using `futures::join_all`
//! - Total time is approximately the slowest agent, not sum of all
//! - Ideal for independent analyses that can run in parallel

use crate::agents::core::agent::Task;
use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::streaming::{events, StreamChunk, SubAgentOperationType, SubAgentStreamMetrics};
use crate::models::sub_agent::{
    constants::MAX_SUB_AGENTS, ParallelBatchResult, ParallelTaskResult, SubAgentExecutionComplete,
    SubAgentExecutionCreate, SubAgentMetrics,
};
use crate::tools::context::AgentToolContext;
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Task specification for parallel execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParallelTaskSpec {
    /// Agent ID to execute this task
    pub agent_id: String,
    /// Complete prompt for the agent
    pub prompt: String,
}

/// Tool for parallel batch execution across multiple agents.
///
/// This tool enables efficient concurrent execution of multiple tasks
/// across different agents. All tasks run in parallel using
/// `futures::join_all`, making the total execution time approximately
/// equal to the slowest individual task.
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
    db: Arc<DBClient>,
    /// Agent registry for agent lookup
    registry: Arc<AgentRegistry>,
    /// Agent orchestrator for execution
    orchestrator: Arc<AgentOrchestrator>,
    /// MCP manager for tool routing (optional)
    mcp_manager: Option<Arc<MCPManager>>,
    /// Tauri app handle for event emission (optional, for validation)
    app_handle: Option<AppHandle>,
    /// Current agent ID (parent agent)
    current_agent_id: String,
    /// Workflow ID
    workflow_id: String,
    /// Whether this tool is for the primary agent (true) or a sub-agent (false)
    is_primary_agent: bool,
}

impl ParallelTasksTool {
    /// Creates a new ParallelTasksTool.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `context` - Agent tool context with system dependencies
    /// * `current_agent_id` - ID of the agent using this tool
    /// * `workflow_id` - Workflow ID for scoping
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Example
    /// ```ignore
    /// let tool = ParallelTasksTool::new(
    ///     db.clone(),
    ///     context,
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
            current_agent_id,
            workflow_id,
            is_primary_agent,
        }
    }

    /// Emits a streaming event to the frontend via Tauri.
    ///
    /// This is a helper method to emit sub-agent lifecycle events.
    /// If no app_handle is available, the event is silently skipped.
    fn emit_event(&self, event_name: &str, chunk: &StreamChunk) {
        if let Some(ref handle) = self.app_handle {
            if let Err(e) = handle.emit(event_name, chunk) {
                warn!(
                    event = %event_name,
                    error = %e,
                    "Failed to emit parallel task event"
                );
            }
        }
    }

    /// Executes multiple tasks in parallel.
    ///
    /// # Arguments
    /// * `tasks` - Vector of (agent_id, prompt) pairs
    /// * `wait_all` - Whether to wait for all tasks (currently always true)
    #[instrument(skip(self, tasks), fields(
        current_agent_id = %self.current_agent_id,
        workflow_id = %self.workflow_id,
        task_count = tasks.len()
    ))]
    async fn execute_batch(
        &self,
        tasks: Vec<ParallelTaskSpec>,
        wait_all: bool,
    ) -> ToolResult<Value> {
        // 1. Check if this agent is the primary (workflow starter)
        if !self.is_primary_agent {
            return Err(ToolError::PermissionDenied(
                "Only the primary workflow agent can execute parallel tasks. \
                 Sub-agents cannot use parallel execution."
                    .to_string(),
            ));
        }

        // 2. Validate task count
        if tasks.is_empty() {
            return Err(ToolError::ValidationFailed(
                "Tasks array cannot be empty. Provide at least one task.".to_string(),
            ));
        }

        if tasks.len() > MAX_SUB_AGENTS {
            return Err(ToolError::ValidationFailed(format!(
                "Maximum {} parallel tasks allowed. Received {}.",
                MAX_SUB_AGENTS,
                tasks.len()
            )));
        }

        // 3. Validate each task
        for (i, task) in tasks.iter().enumerate() {
            if task.agent_id.trim().is_empty() {
                return Err(ToolError::ValidationFailed(format!(
                    "Task {} has empty agent_id. All tasks must specify an agent.",
                    i
                )));
            }
            if task.prompt.trim().is_empty() {
                return Err(ToolError::ValidationFailed(format!(
                    "Task {} for agent '{}' has empty prompt. Each task must have a prompt.",
                    i, task.agent_id
                )));
            }
            if task.agent_id == self.current_agent_id {
                return Err(ToolError::ValidationFailed(format!(
                    "Task {} cannot delegate to self (agent '{}'). Choose different agents.",
                    i, task.agent_id
                )));
            }
        }

        // 3b. Optionally validate MCP server names for each agent
        // (This is informational - agents use their existing configs)
        if let Some(ref mcp_mgr) = self.mcp_manager {
            for task_spec in &tasks {
                if let Some(agent) = self.registry.get(&task_spec.agent_id).await {
                    let mcp_servers = agent.mcp_servers();
                    if !mcp_servers.is_empty() {
                        if let Err(invalid) = mcp_mgr.validate_server_names(&mcp_servers).await {
                            warn!(
                                agent_id = %task_spec.agent_id,
                                invalid_servers = ?invalid,
                                "Parallel task agent has unknown MCP servers configured"
                            );
                        }
                    }
                }
            }
        }

        // 4. Request human-in-the-loop validation (High risk for parallel execution)
        let validation_helper = ValidationHelper::new(self.db.clone(), self.app_handle.clone());
        let task_pairs: Vec<(String, String)> = tasks
            .iter()
            .map(|t| (t.agent_id.clone(), t.prompt.clone()))
            .collect();
        let details = ValidationHelper::parallel_details(&task_pairs);
        let risk_level =
            ValidationHelper::determine_risk_level(&SubAgentOperationType::ParallelBatch);

        validation_helper
            .request_validation(
                &self.workflow_id,
                SubAgentOperationType::ParallelBatch,
                &format!("Execute {} tasks in parallel", tasks.len()),
                details,
                risk_level,
            )
            .await?;

        info!(
            task_count = tasks.len(),
            wait_all = wait_all,
            "Starting parallel batch execution"
        );

        // 4. Create execution records and tasks
        let batch_id = Uuid::new_v4().to_string();
        let mut orchestrator_tasks: Vec<(String, Task)> = Vec::new();
        let mut execution_ids: Vec<String> = Vec::new();

        for task_spec in &tasks {
            let execution_id = Uuid::new_v4().to_string();
            execution_ids.push(execution_id.clone());

            // Create execution record
            let mut execution_create = SubAgentExecutionCreate::new(
                self.workflow_id.clone(),
                self.current_agent_id.clone(),
                task_spec.agent_id.clone(),
                format!("Parallel task for {}", task_spec.agent_id),
                task_spec.prompt.clone(),
            );
            // Set status to running (new() defaults to pending)
            execution_create.status = "running".to_string();

            // Use db.create() which handles serialization correctly (avoids SDK enum issues)
            if let Err(e) = self
                .db
                .create("sub_agent_execution", &execution_id, execution_create)
                .await
            {
                warn!(
                    execution_id = %execution_id,
                    error = %e,
                    "Failed to create execution record"
                );
            }

            // Create Task for orchestrator
            let task = Task {
                id: format!("parallel_{}_{}", batch_id, task_spec.agent_id),
                description: task_spec.prompt.clone(),
                context: serde_json::json!({
                    "workflow_id": self.workflow_id,
                    "parent_agent_id": self.current_agent_id,
                    "batch_id": batch_id,
                    "is_parallel_task": true
                }),
            };

            orchestrator_tasks.push((task_spec.agent_id.clone(), task));

            // Emit sub_agent_start event for this parallel task
            let start_chunk = StreamChunk::sub_agent_start(
                self.workflow_id.clone(),
                task_spec.agent_id.clone(),
                format!("Parallel task for {}", task_spec.agent_id),
                self.current_agent_id.clone(),
                task_spec.prompt.clone(),
            );
            self.emit_event(events::WORKFLOW_STREAM, &start_chunk);
        }

        // 5. Execute all tasks in parallel using orchestrator
        let start_time = std::time::Instant::now();
        let results = self.orchestrator.execute_parallel(orchestrator_tasks).await;
        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        // 6. Process results
        let mut parallel_results: Vec<ParallelTaskResult> = Vec::new();
        let mut completed_count = 0;
        let mut failed_count = 0;
        let mut aggregated_reports: Vec<String> = Vec::new();

        for (i, (result, task_spec)) in results.into_iter().zip(tasks.iter()).enumerate() {
            let execution_id = execution_ids.get(i).cloned().unwrap_or_default();

            let task_result = match result {
                Ok(report) => {
                    completed_count += 1;
                    let metrics = SubAgentMetrics {
                        duration_ms: report.metrics.duration_ms,
                        tokens_input: report.metrics.tokens_input as u64,
                        tokens_output: report.metrics.tokens_output as u64,
                    };

                    // Update execution record
                    let completion = SubAgentExecutionComplete::success(
                        report.metrics.duration_ms,
                        Some(metrics.tokens_input),
                        Some(metrics.tokens_output),
                        report.content.clone(),
                    );

                    self.update_execution_record(&execution_id, &completion)
                        .await;

                    // Emit sub_agent_complete event
                    let complete_chunk = StreamChunk::sub_agent_complete(
                        self.workflow_id.clone(),
                        task_spec.agent_id.clone(),
                        format!("Parallel task for {}", task_spec.agent_id),
                        self.current_agent_id.clone(),
                        report.content.clone(),
                        SubAgentStreamMetrics {
                            duration_ms: metrics.duration_ms,
                            tokens_input: metrics.tokens_input,
                            tokens_output: metrics.tokens_output,
                        },
                    );
                    self.emit_event(events::WORKFLOW_STREAM, &complete_chunk);

                    aggregated_reports.push(format!(
                        "## Agent: {}\n\n{}\n",
                        task_spec.agent_id, report.content
                    ));

                    ParallelTaskResult {
                        agent_id: task_spec.agent_id.clone(),
                        success: true,
                        report: Some(report.content),
                        error: None,
                        metrics: Some(metrics),
                    }
                }
                Err(e) => {
                    failed_count += 1;
                    let error_msg = e.to_string();

                    error!(
                        agent_id = %task_spec.agent_id,
                        error = %error_msg,
                        "Parallel task failed"
                    );

                    // Update execution record
                    let completion = SubAgentExecutionComplete::error(0, error_msg.clone());
                    self.update_execution_record(&execution_id, &completion)
                        .await;

                    // Emit sub_agent_error event
                    let error_chunk = StreamChunk::sub_agent_error(
                        self.workflow_id.clone(),
                        task_spec.agent_id.clone(),
                        format!("Parallel task for {}", task_spec.agent_id),
                        self.current_agent_id.clone(),
                        error_msg.clone(),
                        0,
                    );
                    self.emit_event(events::WORKFLOW_STREAM, &error_chunk);

                    aggregated_reports.push(format!(
                        "## Agent: {} (ERROR)\n\nExecution failed: {}\n",
                        task_spec.agent_id, error_msg
                    ));

                    ParallelTaskResult {
                        agent_id: task_spec.agent_id.clone(),
                        success: false,
                        report: None,
                        error: Some(error_msg),
                        metrics: None,
                    }
                }
            };

            parallel_results.push(task_result);
        }

        // 7. Build aggregated report
        let aggregated_report = format!(
            "# Parallel Execution Report\n\n\
             **Batch ID:** {}\n\
             **Total Duration:** {} ms\n\
             **Completed:** {} / {}\n\
             **Failed:** {}\n\n\
             ---\n\n\
             {}",
            batch_id,
            total_duration_ms,
            completed_count,
            parallel_results.len(),
            failed_count,
            aggregated_reports.join("\n---\n\n")
        );

        info!(
            batch_id = %batch_id,
            completed = completed_count,
            failed = failed_count,
            total_duration_ms = total_duration_ms,
            "Parallel batch execution completed"
        );

        // 8. Return result
        let result = ParallelBatchResult {
            success: failed_count == 0,
            completed: completed_count,
            failed: failed_count,
            results: parallel_results,
            aggregated_report,
        };

        serde_json::to_value(&result)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to serialize result: {}", e)))
    }

    /// Updates an execution record with completion data
    async fn update_execution_record(
        &self,
        execution_id: &str,
        completion: &SubAgentExecutionComplete,
    ) {
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
    }
}

#[async_trait]
impl Tool for ParallelTasksTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "ParallelTasksTool".to_string(),
            name: "Parallel Tasks".to_string(),
            description: r#"Executes multiple tasks in parallel across different agents.

USE THIS TOOL WHEN:
- You need to run multiple independent analyses simultaneously
- Time is critical and tasks don't depend on each other
- You want to gather information from multiple specialized agents at once

IMPORTANT CONSTRAINTS:
- Maximum 3 tasks per batch
- All tasks execute concurrently (total time ≈ slowest task, not sum)
- Each agent only receives its prompt - NO shared context/memory/state
- You must include ALL necessary information in each prompt

OPERATIONS:
- execute_batch: Run multiple tasks in parallel
  Required: tasks (array of {agent_id, prompt})
  Optional: wait_all (default: true)

PROMPT BEST PRACTICES:
1. Each prompt must be self-contained with all necessary context
2. Specify expected report format in each prompt
3. Keep tasks independent - don't rely on other task results

EXAMPLE - Parallel analysis:
{
  "operation": "execute_batch",
  "tasks": [
    {"agent_id": "db_agent", "prompt": "Analyze database performance. Return: 1) Slow queries, 2) Missing indexes."},
    {"agent_id": "api_agent", "prompt": "Review API endpoints for security issues. Return: 1) Vulnerabilities found, 2) Recommendations."},
    {"agent_id": "ui_agent", "prompt": "Check UI accessibility compliance. Return: 1) WCAG violations, 2) Fixes needed."}
  ]
}

RESULT FORMAT:
- success: true if ALL tasks succeeded
- completed: number of successful tasks
- failed: number of failed tasks
- results: array with each task's result (report or error)
- aggregated_report: combined markdown report from all tasks"#.to_string(),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["execute_batch"],
                        "description": "The operation to perform"
                    },
                    "tasks": {
                        "type": "array",
                        "maxItems": 3,
                        "description": "Array of agent-prompt pairs (max 3)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "agent_id": {
                                    "type": "string",
                                    "description": "Target agent ID"
                                },
                                "prompt": {
                                    "type": "string",
                                    "description": "COMPLETE prompt for this agent"
                                }
                            },
                            "required": ["agent_id", "prompt"]
                        }
                    },
                    "wait_all": {
                        "type": "boolean",
                        "default": true,
                        "description": "Wait for all tasks to complete (currently always true)"
                    }
                },
                "required": ["operation", "tasks"]
            }),

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
                for t in tasks_array {
                    let agent_id = t["agent_id"]
                        .as_str()
                        .ok_or_else(|| {
                            ToolError::InvalidInput("Task missing 'agent_id'".to_string())
                        })?
                        .to_string();
                    let prompt = t["prompt"]
                        .as_str()
                        .ok_or_else(|| {
                            ToolError::InvalidInput("Task missing 'prompt'".to_string())
                        })?
                        .to_string();
                    tasks.push(ParallelTaskSpec { agent_id, prompt });
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

                if !tasks.is_array() {
                    return Err(ToolError::InvalidInput(
                        "'tasks' must be an array".to_string(),
                    ));
                }

                let tasks_array = tasks.as_array().unwrap();

                if tasks_array.is_empty() {
                    return Err(ToolError::InvalidInput(
                        "'tasks' array cannot be empty".to_string(),
                    ));
                }

                if tasks_array.len() > MAX_SUB_AGENTS {
                    return Err(ToolError::ValidationFailed(format!(
                        "Maximum {} tasks allowed. Received {}.",
                        MAX_SUB_AGENTS,
                        tasks_array.len()
                    )));
                }

                for (i, task) in tasks_array.iter().enumerate() {
                    if !task.is_object() {
                        return Err(ToolError::InvalidInput(format!(
                            "Task {} must be an object with 'agent_id' and 'prompt'",
                            i
                        )));
                    }
                    if task.get("agent_id").is_none() {
                        return Err(ToolError::InvalidInput(format!(
                            "Task {} missing 'agent_id'",
                            i
                        )));
                    }
                    if task.get("prompt").is_none() {
                        return Err(ToolError::InvalidInput(format!(
                            "Task {} missing 'prompt'",
                            i
                        )));
                    }
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

    fn requires_confirmation(&self) -> bool {
        // Parallel execution does not require confirmation by default
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let definition = ToolDefinition {
            id: "ParallelTasksTool".to_string(),
            name: "Parallel Tasks".to_string(),
            description: "Test".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "ParallelTasksTool");
        assert!(!definition.requires_confirmation);
    }

    #[test]
    fn test_parallel_task_spec_serialization() {
        let spec = ParallelTaskSpec {
            agent_id: "db_agent".to_string(),
            prompt: "Analyze schema".to_string(),
        };

        let json = serde_json::to_string(&spec).unwrap();
        assert!(json.contains("db_agent"));
        assert!(json.contains("Analyze schema"));

        let deserialized: ParallelTaskSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.agent_id, "db_agent");
    }

    #[test]
    fn test_input_validation_execute_batch() {
        let valid_input = serde_json::json!({
            "operation": "execute_batch",
            "tasks": [
                {"agent_id": "db_agent", "prompt": "Analyze database"},
                {"agent_id": "api_agent", "prompt": "Check API security"}
            ]
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "execute_batch");
        assert!(valid_input["tasks"].is_array());
        assert_eq!(valid_input["tasks"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_input_validation_too_many_tasks() {
        let invalid_input = serde_json::json!({
            "operation": "execute_batch",
            "tasks": [
                {"agent_id": "a1", "prompt": "p1"},
                {"agent_id": "a2", "prompt": "p2"},
                {"agent_id": "a3", "prompt": "p3"},
                {"agent_id": "a4", "prompt": "p4"} // 4 tasks - exceeds limit
            ]
        });

        let tasks_len = invalid_input["tasks"].as_array().unwrap().len();
        assert!(tasks_len > MAX_SUB_AGENTS);
    }

    #[test]
    fn test_parallel_batch_result_serialization() {
        let result = ParallelBatchResult {
            success: true,
            completed: 2,
            failed: 0,
            results: vec![
                ParallelTaskResult {
                    agent_id: "agent_1".to_string(),
                    success: true,
                    report: Some("Report 1".to_string()),
                    error: None,
                    metrics: Some(SubAgentMetrics {
                        duration_ms: 1000,
                        tokens_input: 100,
                        tokens_output: 200,
                    }),
                },
                ParallelTaskResult {
                    agent_id: "agent_2".to_string(),
                    success: true,
                    report: Some("Report 2".to_string()),
                    error: None,
                    metrics: Some(SubAgentMetrics {
                        duration_ms: 1500,
                        tokens_input: 150,
                        tokens_output: 250,
                    }),
                },
            ],
            aggregated_report: "# Combined Report".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"completed\":2"));
        assert!(json.contains("\"failed\":0"));
        assert!(json.contains("agent_1"));
        assert!(json.contains("agent_2"));
    }

    #[test]
    fn test_parallel_task_result_with_error() {
        let result = ParallelTaskResult {
            agent_id: "failed_agent".to_string(),
            success: false,
            report: None,
            error: Some("Connection timeout".to_string()),
            metrics: None,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("Connection timeout"));
        assert!(json.contains("\"report\":null"));
    }

    #[test]
    fn test_max_sub_agents_constant() {
        assert_eq!(MAX_SUB_AGENTS, 3);
    }
}
