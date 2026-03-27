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

//! Execution logic for ParallelTasksTool.
//!
//! Contains task validation, preparation, parallel execution, and result collection.

use super::parallel_tasks::{ParallelTaskSpec, ParallelTasksTool, PreparedExecution};
use crate::agents::core::agent::Task;
use crate::models::streaming::SubAgentOperationType;
use crate::models::sub_agent::{
    constants::MAX_SUB_AGENTS, ParallelBatchResult, ParallelTaskResult, SubAgentExecutionCreate,
    SubAgentMetrics,
};
use crate::tools::sub_agent_executor::{ExecutionResult, SubAgentExecutor};
use crate::tools::task_bridge::resolve_and_reassign_tasks;
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{ToolError, ToolResult};
use serde_json::Value;
use tokio::task::JoinSet;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

impl ParallelTasksTool {
    /// Validates task specifications for batch execution.
    ///
    /// Checks:
    /// - Task array is not empty
    /// - Task count does not exceed MAX_SUB_AGENTS
    /// - Each task has a non-empty resolved agent_id (already resolved from ID or name)
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
                    i, task.agent_name
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
            .map(|t| (t.agent_name.clone(), t.prompt.clone()))
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
        // Create executor for unified event emission
        // Use with_cancellation for graceful shutdown support
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

            // Create execution record with batch_id as parent for hierarchical tracing
            let mut execution_create = SubAgentExecutionCreate::with_parent(
                self.workflow_id.clone(),
                self.current_agent_id.clone(),
                task_spec.agent_id.clone(),
                task_spec.agent_name.clone(),
                task_spec.prompt.clone(),
                Some(batch_id.clone()), // Link parallel tasks to batch
            );
            execution_create.status = "running".to_string();

            if let Err(e) = self
                .db
                .create("sub_agent_execution", &execution_id, execution_create)
                .await
            {
                warn!(
                    execution_id = %execution_id,
                    batch_id = %batch_id, // Include batch correlation in logs
                    error = %e,
                    "Failed to create execution record"
                );
            }

            // Resolve task_ids if provided for this task
            let mut context = serde_json::json!({
                "workflow_id": self.workflow_id,
                "parent_agent_id": self.current_agent_id,
                "batch_id": batch_id,
                "is_parallel_task": true
            });

            if let Some(ref ids) = task_spec.task_ids {
                let assigned = resolve_and_reassign_tasks(
                    &self.db,
                    ids,
                    &self.workflow_id,
                    &task_spec.agent_id,
                )
                .await?;
                context["assigned_tasks"] = serde_json::json!(assigned);
            }

            // Create Task for orchestrator
            let task = Task {
                id: format!("parallel_{}_{}", batch_id, task_spec.agent_id),
                description: task_spec.prompt.clone(),
                context,
            };

            orchestrator_tasks.push((task_spec.agent_id.clone(), task));

            // Emit sub_agent_start event via unified executor
            executor.emit_start_event(
                &task_spec.agent_id,
                &task_spec.agent_name,
                &task_spec.prompt,
            );
        }

        Ok(PreparedExecution {
            executor,
            batch_id,
            execution_ids,
            orchestrator_tasks,
        })
    }

    /// Executes all tasks in parallel using JoinSet.
    ///
    /// Each task is executed with retry and heartbeat monitoring.
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
                // Create executor for this task with retry support
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
                            tool_executions: Vec::new(),
                            reasoning_steps: Vec::new(),
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

            // Persist sub-agent internal tool executions and reasoning steps
            executor
                .persist_sub_agent_internals(&execution_id, &task_spec.agent_id, &exec_result)
                .await;

            // Emit completion event with resolved agent name
            executor.emit_complete_event(&task_spec.agent_id, &task_spec.agent_name, &exec_result);

            // Build task result
            let task_result = if exec_result.success {
                completed_count += 1;

                aggregated_reports.push(format!(
                    "## Agent: {}\n\n{}\n",
                    task_spec.agent_name, exec_result.report
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

                // Include batch_id (parent_execution_id) for hierarchical tracing
                error!(
                    agent_id = %task_spec.agent_id,
                    batch_id = %batch_id,
                    error = %error_msg,
                    "Parallel task failed"
                );

                aggregated_reports.push(format!(
                    "## Agent: {} (ERROR)\n\nExecution failed: {}\n",
                    task_spec.agent_name, error_msg
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

    /// Executes multiple tasks in parallel.
    ///
    /// # Arguments
    /// * `tasks` - Vector of task specifications (agent + prompt pairs)
    /// * `wait_all` - Whether to wait for all tasks (currently always true)
    #[instrument(skip(self, tasks), fields(
        current_agent_id = %self.current_agent_id,
        workflow_id = %self.workflow_id,
        task_count = tasks.len()
    ))]
    pub(crate) async fn execute_batch(
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
}
