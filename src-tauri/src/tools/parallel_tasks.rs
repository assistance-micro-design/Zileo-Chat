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
//! - All tasks execute concurrently using `tokio::task::JoinSet`
//! - Total time is approximately the slowest agent, not sum of all
//! - Per-task control allows for future cancellation support (OPT-SA-7)
//! - Ideal for independent analyses that can run in parallel

use crate::agents::core::agent::Task;
use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::streaming::SubAgentOperationType;
use crate::models::sub_agent::{
    constants::MAX_SUB_AGENTS, ParallelBatchResult, ParallelTaskResult, SubAgentExecutionCreate,
    SubAgentMetrics,
};
use crate::tools::context::AgentToolContext;
use crate::tools::sub_agent_executor::{ExecutionResult, SubAgentExecutor};
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
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

/// Prepared execution context containing all resources needed for parallel execution.
/// Used to pass data between helper functions during execute_batch() (OPT-SA-9).
struct PreparedExecution {
    /// Unified executor for event emission and DB updates
    executor: SubAgentExecutor,
    /// Unique identifier for this batch
    batch_id: String,
    /// Execution IDs for each task (in order)
    execution_ids: Vec<String>,
    /// Tasks prepared for orchestrator execution
    orchestrator_tasks: Vec<(String, Task)>,
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
    /// # Cancellation Token (OPT-SA-7)
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
        }
    }

    // =========================================================================
    // Helper functions for execute_batch() - OPT-SA-9: Reduce Cyclomatic Complexity
    // =========================================================================

    /// Validates task specifications for batch execution.
    ///
    /// Checks:
    /// - Task array is not empty
    /// - Task count does not exceed MAX_SUB_AGENTS
    /// - Each task has a non-empty agent_id
    /// - Each task has a non-empty prompt
    /// - No task delegates to self
    fn validate_tasks(&self, tasks: &[ParallelTaskSpec]) -> ToolResult<()> {
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

        Ok(())
    }

    /// Validates MCP server configurations for each task's agent.
    ///
    /// This is informational only - logs warnings for unknown MCP servers
    /// but does not fail the execution.
    async fn validate_mcp_servers(&self, tasks: &[ParallelTaskSpec]) {
        if let Some(ref mcp_mgr) = self.mcp_manager {
            for task_spec in tasks {
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
    }

    /// Requests human-in-the-loop validation for parallel batch execution.
    ///
    /// Blocks until validation is approved or returns error if rejected.
    async fn request_human_validation(&self, tasks: &[ParallelTaskSpec]) -> ToolResult<()> {
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
            .await
    }

    /// Prepares execution context including DB records and orchestrator tasks.
    ///
    /// Creates:
    /// - SubAgentExecutor for unified event emission
    /// - Execution records in database
    /// - Task objects for orchestrator
    /// - Emits start events for each task
    async fn prepare_execution(&self, tasks: &[ParallelTaskSpec]) -> ToolResult<PreparedExecution> {
        // Create executor for unified event emission (OPT-SA-4)
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

        let batch_id = Uuid::new_v4().to_string();
        let mut orchestrator_tasks: Vec<(String, Task)> = Vec::new();
        let mut execution_ids: Vec<String> = Vec::new();

        for task_spec in tasks {
            let execution_id = Uuid::new_v4().to_string();
            execution_ids.push(execution_id.clone());

            // Create execution record with batch_id as parent for hierarchical tracing (OPT-SA-11)
            let mut execution_create = SubAgentExecutionCreate::with_parent(
                self.workflow_id.clone(),
                self.current_agent_id.clone(),
                task_spec.agent_id.clone(),
                format!("Parallel task for {}", task_spec.agent_id),
                task_spec.prompt.clone(),
                Some(batch_id.clone()), // OPT-SA-11: Link parallel tasks to batch
            );
            execution_create.status = "running".to_string();

            if let Err(e) = self
                .db
                .create("sub_agent_execution", &execution_id, execution_create)
                .await
            {
                warn!(
                    execution_id = %execution_id,
                    batch_id = %batch_id, // OPT-SA-11: Include batch correlation in logs
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

            // Emit sub_agent_start event via unified executor (OPT-SA-4)
            let agent_name = format!("Parallel task for {}", task_spec.agent_id);
            executor.emit_start_event(&task_spec.agent_id, &agent_name, &task_spec.prompt);
        }

        Ok(PreparedExecution {
            executor,
            batch_id,
            execution_ids,
            orchestrator_tasks,
        })
    }

    /// Executes all tasks in parallel using JoinSet (OPT-SA-6).
    ///
    /// Each task is executed with retry and heartbeat monitoring (OPT-SA-1, OPT-SA-10).
    /// Returns results in original task order along with total duration.
    async fn run_parallel_tasks(
        &self,
        orchestrator_tasks: Vec<(String, Task)>,
        task_count: usize,
    ) -> (Vec<ExecutionResult>, u64) {
        let start_time = std::time::Instant::now();
        let mut join_set: JoinSet<(usize, ExecutionResult)> = JoinSet::new();

        // Clone dependencies for each spawn
        for (idx, (agent_id, task)) in orchestrator_tasks.into_iter().enumerate() {
            // Clone all dependencies needed for SubAgentExecutor in spawn
            let db = self.db.clone();
            let orchestrator = self.orchestrator.clone();
            let mcp_manager = self.mcp_manager.clone();
            let app_handle = self.app_handle.clone();
            let workflow_id = self.workflow_id.clone();
            let current_agent_id = self.current_agent_id.clone();
            let cancellation_token = self.cancellation_token.clone();

            join_set.spawn(async move {
                // Create executor for this task with retry support (OPT-SA-10)
                let executor = SubAgentExecutor::with_cancellation(
                    db,
                    orchestrator,
                    mcp_manager,
                    app_handle,
                    workflow_id,
                    current_agent_id,
                    cancellation_token,
                );

                // Execute with retry and heartbeat monitoring
                let result = executor.execute_with_retry(&agent_id, task, None).await;
                (idx, result)
            });
        }

        // Collect results with their indices
        let mut indexed_results: Vec<(usize, ExecutionResult)> = Vec::with_capacity(task_count);
        while let Some(join_result) = join_set.join_next().await {
            match join_result {
                Ok((idx, exec_result)) => indexed_results.push((idx, exec_result)),
                Err(join_error) => {
                    warn!("Task panicked during parallel execution: {}", join_error);
                    indexed_results.push((
                        usize::MAX,
                        ExecutionResult {
                            success: false,
                            report: format!("# Task Panic\n\nTask panicked: {}", join_error),
                            metrics: SubAgentMetrics {
                                duration_ms: 0,
                                tokens_input: 0,
                                tokens_output: 0,
                            },
                            error_message: Some(format!("Task panicked: {}", join_error)),
                        },
                    ));
                }
            }
        }

        // Sort by index to restore original task order
        indexed_results.sort_by_key(|(idx, _)| *idx);
        let results: Vec<ExecutionResult> = indexed_results.into_iter().map(|(_, r)| r).collect();
        let total_duration_ms = start_time.elapsed().as_millis() as u64;

        (results, total_duration_ms)
    }

    /// Processes execution results, updates DB records, and builds aggregated report.
    ///
    /// For each result:
    /// - Updates execution record in database
    /// - Emits completion event
    /// - Builds individual and aggregated reports
    ///
    /// # OPT-SA-10 Update
    /// Now accepts `Vec<ExecutionResult>` directly from `run_parallel_tasks` which uses
    /// `execute_with_retry` for each task with exponential backoff on transient errors.
    async fn process_results(
        &self,
        tasks: &[ParallelTaskSpec],
        results: Vec<ExecutionResult>,
        execution_ids: &[String],
        executor: &SubAgentExecutor,
        batch_id: &str,
        total_duration_ms: u64,
    ) -> ParallelBatchResult {
        let mut parallel_results: Vec<ParallelTaskResult> = Vec::new();
        let mut completed_count = 0;
        let mut failed_count = 0;
        let mut aggregated_reports: Vec<String> = Vec::new();

        for (i, (exec_result, task_spec)) in results.into_iter().zip(tasks.iter()).enumerate() {
            let execution_id = execution_ids.get(i).cloned().unwrap_or_default();

            // Update execution record in database
            executor
                .update_execution_record(&execution_id, &exec_result)
                .await;

            // Emit completion event
            let agent_name = format!("Parallel task for {}", task_spec.agent_id);
            executor.emit_complete_event(&task_spec.agent_id, &agent_name, &exec_result);

            // Build task result
            let task_result = if exec_result.success {
                completed_count += 1;

                aggregated_reports.push(format!(
                    "## Agent: {}\n\n{}\n",
                    task_spec.agent_id, exec_result.report
                ));

                ParallelTaskResult {
                    agent_id: task_spec.agent_id.clone(),
                    success: true,
                    report: Some(exec_result.report),
                    error: None,
                    metrics: Some(exec_result.metrics),
                }
            } else {
                failed_count += 1;
                let error_msg = exec_result.error_message.clone().unwrap_or_default();

                // OPT-SA-11: Include batch_id (parent_execution_id) for hierarchical tracing
                error!(
                    agent_id = %task_spec.agent_id,
                    batch_id = %batch_id,
                    error = %error_msg,
                    "Parallel task failed"
                );

                aggregated_reports.push(format!(
                    "## Agent: {} (ERROR)\n\nExecution failed: {}\n",
                    task_spec.agent_id, error_msg
                ));

                ParallelTaskResult {
                    agent_id: task_spec.agent_id.clone(),
                    success: false,
                    report: None,
                    error: Some(error_msg),
                    metrics: Some(exec_result.metrics),
                }
            };

            parallel_results.push(task_result);
        }

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

        ParallelBatchResult {
            success: failed_count == 0,
            completed: completed_count,
            failed: failed_count,
            results: parallel_results,
            aggregated_report,
        }
    }

    // =========================================================================
    // Main execute_batch() - Refactored for CC~6 (OPT-SA-9)
    // =========================================================================

    /// Executes multiple tasks in parallel.
    ///
    /// # Arguments
    /// * `tasks` - Vector of (agent_id, prompt) pairs
    /// * `wait_all` - Whether to wait for all tasks (currently always true)
    ///
    /// # Refactoring (OPT-SA-9)
    ///
    /// This function has been refactored to reduce cyclomatic complexity from ~20 to ~6.
    /// Logic has been extracted into helper functions:
    /// - `validate_tasks()` - Input validation
    /// - `validate_mcp_servers()` - MCP server validation (informational)
    /// - `request_human_validation()` - Human-in-the-loop approval
    /// - `prepare_execution()` - DB records and task preparation
    /// - `run_parallel_tasks()` - JoinSet parallel execution
    /// - `process_results()` - Result processing and report generation
    #[instrument(skip(self, tasks), fields(
        current_agent_id = %self.current_agent_id,
        workflow_id = %self.workflow_id,
        task_count = tasks.len()
    ))]
    async fn execute_batch(
        &self,
        tasks: Vec<ParallelTaskSpec>,
        _wait_all: bool,
    ) -> ToolResult<Value> {
        // 1. Check primary agent permission
        SubAgentExecutor::check_primary_permission(self.is_primary_agent, "parallel tasks")?;

        // 2. Validate tasks
        self.validate_tasks(&tasks)?;

        // 3. Validate MCP servers (informational only)
        self.validate_mcp_servers(&tasks).await;

        // 4. Request human-in-the-loop validation
        self.request_human_validation(&tasks).await?;

        info!(
            task_count = tasks.len(),
            "Starting parallel batch execution"
        );

        // 5. Prepare execution (DB records, executor, start events)
        let prepared = self.prepare_execution(&tasks).await?;

        // 6. Execute in parallel
        let (results, total_duration_ms) = self
            .run_parallel_tasks(prepared.orchestrator_tasks, tasks.len())
            .await;

        // 7. Process results and build report
        let batch_result = self
            .process_results(
                &tasks,
                results,
                &prepared.execution_ids,
                &prepared.executor,
                &prepared.batch_id,
                total_duration_ms,
            )
            .await;

        // 8. Serialize and return
        serde_json::to_value(&batch_result)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to serialize result: {}", e)))
    }

    // NOTE: update_execution_record() removed as part of OPT-SA-5
    // Now using SubAgentExecutor::update_execution_record() for unified DB updates
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
                        "description": "Operation: 'execute_batch' runs multiple tasks concurrently across different agents"
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
