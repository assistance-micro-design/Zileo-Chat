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

//! Execution logic for SpawnAgentTool.
//!
//! Contains spawn, list_children, and terminate operations.

use super::spawn_agent::{
    SpawnAgentTool, SpawnParams, SpawnedChild, DEFAULT_SUB_AGENT_SYSTEM_PROMPT,
};
use crate::agents::core::agent::Task;
use crate::agents::LLMAgent;
use crate::db::DBClient;
use crate::models::streaming::SubAgentOperationType;
use crate::models::sub_agent::{constants::MAX_SUB_AGENTS, SubAgentSpawnResult, SubAgentStatus};
use crate::models::{AgentConfig, LLMConfig, Lifecycle};
use crate::tools::constants::sub_agent::TASK_DESC_TRUNCATE_CHARS;
use crate::tools::factory::ToolFactory;
use crate::tools::sub_agent_executor::SubAgentExecutor;
use crate::tools::utils::safe_truncate;
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{ToolError, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Lightweight model info for sub-agent config resolution.
///
/// When a sub-agent overrides provider/model, we query the DB to get
/// the target model's actual capabilities instead of inheriting from parent.
#[derive(Debug, Deserialize, Serialize)]
struct ModelLookup {
    is_reasoning: bool,
    context_window: usize,
    max_output_tokens: usize,
    temperature_default: f64,
}

/// Queries the DB for model capabilities by api_name and provider.
///
/// Returns `None` if the model is not found (falls back to parent config).
async fn lookup_model_config(db: &DBClient, api_name: &str, provider: &str) -> Option<ModelLookup> {
    let query = "SELECT is_reasoning, context_window, max_output_tokens, temperature_default \
                 FROM llm_model WHERE api_name = $api_name AND provider = $provider";
    let result: Vec<ModelLookup> = db
        .query_with_params(
            query,
            vec![
                ("api_name".to_string(), serde_json::json!(api_name)),
                ("provider".to_string(), serde_json::json!(provider)),
            ],
        )
        .await
        .ok()?;
    result.into_iter().next()
}

impl SpawnAgentTool {
    pub(crate) async fn spawn(
        &self,
        name: &str,
        prompt: &str,
        params: SpawnParams<'_>,
    ) -> ToolResult<Value> {
        // Check if this agent is the primary (workflow starter)
        SubAgentExecutor::check_primary_permission(self.is_primary_agent, "spawn sub-agents")?;

        // Check sub-agent limit
        let current_count = self.spawned_children.read().await.len();
        SubAgentExecutor::check_limit(current_count, "spawn")?;

        // Validate inputs
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

        // Validate tool names if provided
        if let Some(ref tool_list) = params.tools {
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

        // Validate MCP server names if provided
        if let Some(ref mcp_servers_list) = params.mcp_servers {
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

        // Request human-in-the-loop validation
        let executor = SubAgentExecutor::with_cancellation(
            self.db.clone(),
            self.orchestrator.clone(),
            self.mcp_manager.clone(),
            self.app_handle.clone(),
            self.workflow_id.clone(),
            self.parent_agent_id.clone(),
            self.cancellation_token.clone(),
        )
        .with_parent_message(self.parent_message_id.clone());

        let details = ValidationHelper::spawn_details(
            name,
            prompt,
            &params.tools.clone().unwrap_or_default(),
            &params.mcp_servers.clone().unwrap_or_default(),
        );

        executor
            .request_validation(
                SubAgentOperationType::Spawn,
                &format!("Spawn sub-agent '{}' to execute task", name),
                details,
            )
            .await?;

        // Get parent agent config for defaults
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

        // Generate sub-agent ID
        let sub_agent_id = SubAgentExecutor::generate_sub_agent_id();

        // Build sub-agent configuration
        let parent_tools = params.tools.unwrap_or_else(|| parent_config.tools.clone());
        let sub_agent_tools: Vec<String> = parent_tools
            .into_iter()
            .filter(|t| !ToolFactory::requires_context(t))
            .collect();

        // When model or provider is overridden, look up the target model's actual
        // capabilities from DB. This ensures is_reasoning, context_window, temperature,
        // and max_tokens match the target model instead of inheriting from parent.
        let has_model_override = params.provider.is_some() || params.model.is_some();

        let sub_provider = params
            .provider
            .unwrap_or(&parent_config.llm.provider)
            .to_string();
        let sub_model = params.model.unwrap_or(&parent_config.llm.model).to_string();
        let model_info = if has_model_override {
            match lookup_model_config(&self.db, &sub_model, &sub_provider).await {
                Some(info) => {
                    debug!(
                        model = %sub_model,
                        provider = %sub_provider,
                        is_reasoning = info.is_reasoning,
                        "Resolved target model config from DB for sub-agent"
                    );
                    Some(info)
                }
                None => {
                    warn!(
                        model = %sub_model,
                        provider = %sub_provider,
                        "Target model not found in DB, inheriting parent config"
                    );
                    None
                }
            }
        } else {
            None
        };

        let sub_agent_config = AgentConfig {
            id: sub_agent_id.clone(),
            name: name.to_string(),
            lifecycle: Lifecycle::Temporary,
            llm: LLMConfig {
                provider: sub_provider,
                model: sub_model,
                temperature: model_info
                    .as_ref()
                    .map(|m| m.temperature_default)
                    .unwrap_or(parent_config.llm.temperature),
                max_tokens: model_info
                    .as_ref()
                    .map(|m| m.max_output_tokens)
                    .unwrap_or(parent_config.llm.max_tokens),
                is_reasoning: model_info
                    .as_ref()
                    .map(|m| m.is_reasoning)
                    .unwrap_or(parent_config.llm.is_reasoning),
                context_window: model_info
                    .as_ref()
                    .map(|m| Some(m.context_window))
                    .unwrap_or(parent_config.llm.context_window),
            },
            tools: sub_agent_tools,
            mcp_servers: params
                .mcp_servers
                .unwrap_or_else(|| parent_config.mcp_servers.clone()),
            // Sub-agents inherit parent's skills
            skills: parent_config.skills.clone(),
            // Sub-agents inherit parent's folders and file confirmation setting
            folders: parent_config.folders.clone(),
            require_file_confirmation: parent_config.require_file_confirmation,
            system_prompt: params
                .system_prompt
                .unwrap_or(DEFAULT_SUB_AGENT_SYSTEM_PROMPT)
                .to_string(),
            // Sub-agents inherit parent's max_tool_iterations and reasoning_effort
            max_tool_iterations: parent_config.max_tool_iterations,
            reasoning_effort: parent_config.reasoning_effort.clone(),
        };

        // Create execution record in database (status: running)
        let execution_id = executor
            .create_execution_record(&sub_agent_id, name, prompt)
            .await?;

        // Include execution_id in creation log for hierarchical tracing
        info!(
            sub_agent_id = %sub_agent_id,
            execution_id = %execution_id,
            workflow_id = %self.workflow_id,
            name = %name,
            tools_count = sub_agent_config.tools.len(),
            mcp_servers_count = sub_agent_config.mcp_servers.len(),
            "Creating sub-agent with execution tracking"
        );

        // Create LLMAgent instance for sub-agent
        let sub_agent = LLMAgent::with_factory(
            sub_agent_config.clone(),
            self.llm_manager.clone(),
            self.tool_factory.clone(),
        );

        // Register in registry
        self.registry
            .register(sub_agent_id.clone(), Arc::new(sub_agent))
            .await;

        // Track spawned child
        let spawned_child = SpawnedChild {
            id: sub_agent_id.clone(),
            name: name.to_string(),
            task_description: prompt.to_string(),
            status: SubAgentStatus::Running,
            execution_id: execution_id.clone(),
        };
        self.spawned_children.write().await.push(spawned_child);

        // Emit sub_agent_start event
        executor.emit_start_event(&sub_agent_id, name, prompt);

        // Create task for sub-agent.
        //
        // Each sub-agent gets a fresh `message_id` (its own logical
        // assistant turn). If this sub-agent itself spawns sub-agents
        // (defensive — currently single-level enforced), its descendants
        // chain through this id (H2 audit 2026-05-02).
        let task = Task {
            id: format!("task_{}", Uuid::new_v4()),
            description: prompt.to_string(),
            context: serde_json::json!({
                "workflow_id": self.workflow_id,
                "parent_agent_id": self.parent_agent_id,
                "is_sub_agent": true,
                "message_id": Uuid::new_v4().to_string(),
            }),
        };

        // Execute sub-agent with retry and heartbeat monitoring
        let exec_result = executor.execute_with_retry(&sub_agent_id, task, None).await;

        // Emit completion or error event
        executor.emit_complete_event(&sub_agent_id, name, &exec_result);

        // Update execution record
        executor
            .update_execution_record(&execution_id, &exec_result)
            .await;

        // Persist sub-agent internal tool executions and reasoning steps
        executor
            .persist_sub_agent_internals(&execution_id, &sub_agent_id, &exec_result)
            .await;

        // Update spawned children status
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

        // Cleanup: unregister sub-agent from registry
        if let Err(e) = self.registry.unregister(&sub_agent_id).await {
            warn!(
                sub_agent_id = %sub_agent_id,
                error = %e,
                "Failed to unregister sub-agent"
            );
        }

        // Include execution_id for hierarchical tracing
        info!(
            sub_agent_id = %sub_agent_id,
            execution_id = %execution_id,
            workflow_id = %self.workflow_id,
            success = exec_result.success,
            duration_ms = exec_result.metrics.duration_ms,
            "Sub-agent execution completed"
        );

        // Return result
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
    pub(crate) async fn list_children(&self) -> ToolResult<Value> {
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
    pub(crate) async fn terminate(&self, child_id: &str) -> ToolResult<Value> {
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
