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

//! Execution logic for DelegateTaskTool.
//!
//! Contains delegate and list_agents operations.

use super::delegate_task::{ActiveDelegation, DelegateTaskTool};
use crate::agents::core::agent::Task;
use crate::models::streaming::SubAgentOperationType;
use crate::models::sub_agent::{
    constants::MAX_SUB_AGENTS, DelegateResult, SubAgentExecutionCreate, SubAgentStatus,
};
use crate::models::Lifecycle;
use crate::tools::sub_agent_executor::SubAgentExecutor;
use crate::tools::task_bridge::resolve_and_reassign_tasks;
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{ToolError, ToolResult};
use serde_json::Value;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

impl DelegateTaskTool {
    pub(crate) async fn delegate(
        &self,
        agent_id: &str,
        prompt: &str,
        task_ids: Option<Vec<String>>,
    ) -> ToolResult<Value> {
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

        // 8. Create execution record ID
        let execution_id = Uuid::new_v4().to_string();

        // 9. Create execution record in database (status: running)
        // Note: DelegateTaskTool is a top-level execution, so parent_execution_id = None
        let mut execution_create = SubAgentExecutionCreate::with_parent(
            self.workflow_id.clone(),
            self.current_agent_id.clone(),
            agent_id.to_string(),
            agent_name.clone(),
            prompt.to_string(),
            None, // No parent for top-level delegations
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

        // Log execution creation with tracing ID
        debug!(
            execution_id = %execution_id,
            agent_id = %agent_id,
            workflow_id = %self.workflow_id,
            "Created delegation execution record"
        );

        // 10. Track active delegation
        let delegation = ActiveDelegation {
            agent_id: agent_id.to_string(),
            agent_name: agent_name.clone(),
            task_description: prompt.to_string(),
            status: SubAgentStatus::Running,
            execution_id: execution_id.clone(),
        };
        self.active_delegations.write().await.push(delegation);

        // 10b. Create executor for unified event emission
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

        // 10c. Emit sub_agent_start event via unified executor
        executor.emit_start_event(agent_id, &agent_name, prompt);

        // 10d. Resolve task_ids if provided
        let assigned_tasks = if let Some(ref ids) = task_ids {
            Some(resolve_and_reassign_tasks(&self.db, ids, &self.workflow_id, agent_id).await?)
        } else {
            None
        };

        // 11. Create task for agent with optional assigned_tasks in context
        let mut context = serde_json::json!({
            "workflow_id": self.workflow_id,
            "delegator_agent_id": self.current_agent_id,
            "is_delegation": true
        });

        if let Some(ref tasks) = assigned_tasks {
            context["assigned_tasks"] = serde_json::json!(tasks);
        }

        let task = Task {
            id: format!("delegate_{}", Uuid::new_v4()),
            description: prompt.to_string(),
            context,
        };

        // 12. Execute via unified executor with retry and heartbeat monitoring
        let exec_result = executor.execute_with_retry(agent_id, task, None).await;

        // 13. Emit sub_agent_complete or sub_agent_error event via unified executor
        executor.emit_complete_event(agent_id, &agent_name, &exec_result);

        // Extract values for subsequent processing
        let report = exec_result.report.clone();
        let metrics = exec_result.metrics.clone();
        let success = exec_result.success;

        // 14. Update execution record
        executor
            .update_execution_record(&execution_id, &exec_result)
            .await;

        // 14b. Persist sub-agent internal tool executions and reasoning steps
        executor
            .persist_sub_agent_internals(&execution_id, agent_id, &exec_result)
            .await;

        // 15. Update active delegations status
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

        // Include execution_id for hierarchical tracing
        info!(
            agent_id = %agent_id,
            execution_id = %execution_id,
            workflow_id = %self.workflow_id,
            success = success,
            duration_ms = metrics.duration_ms,
            "Delegation completed"
        );

        // 16. Return result
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
    pub(crate) async fn list_agents(&self) -> ToolResult<Value> {
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
