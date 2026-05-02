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

//! Execution record persistence and event emission for sub-agents.
//!
//! Handles creation/update of execution records in SurrealDB and
//! streaming events to the frontend.

use serde_json::Value;
use tauri::Emitter;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::models::streaming::{events, StreamChunk, SubAgentOperationType, SubAgentStreamMetrics};
use crate::models::sub_agent::SubAgentExecutionCreate;
use crate::tools::utils::safe_truncate;
use crate::tools::validation_helper::ValidationHelper;
use crate::tools::{ToolError, ToolResult};

use super::ExecutionResult;
use super::SubAgentExecutor;

impl SubAgentExecutor {
    /// Requests human-in-the-loop validation.
    ///
    /// # Arguments
    /// * `operation_type` - Type of sub-agent operation
    /// * `description` - Human-readable operation description
    /// * `details` - Additional operation details (JSON)
    ///
    /// # Returns
    /// * `Ok(())` - If approved (or validation skipped)
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_validation(
        &self,
        operation_type: SubAgentOperationType,
        description: &str,
        details: Value,
    ) -> ToolResult<()> {
        let validation_helper = ValidationHelper::new(self.db.clone(), self.app_handle.clone());
        let risk_level = ValidationHelper::determine_risk_level(&operation_type);

        validation_helper
            .request_validation(
                &self.workflow_id,
                operation_type,
                description,
                details,
                risk_level,
            )
            .await
    }

    /// Creates an execution record in the database.
    ///
    /// # Arguments
    /// * `child_agent_id` - Sub-agent ID
    /// * `child_agent_name` - Sub-agent name
    /// * `prompt` - Task prompt
    ///
    /// # Returns
    /// * `Ok(String)` - Execution record ID
    /// * `Err(ToolError)` - Database error
    pub async fn create_execution_record(
        &self,
        child_agent_id: &str,
        child_agent_name: &str,
        prompt: &str,
    ) -> ToolResult<String> {
        self.create_execution_record_with_parent(child_agent_id, child_agent_name, prompt, None)
            .await
    }

    /// Creates an execution record with optional parent execution ID for hierarchical tracing.
    ///
    /// # Arguments
    /// * `child_agent_id` - Sub-agent ID
    /// * `child_agent_name` - Sub-agent name
    /// * `prompt` - Task prompt
    /// * `parent_execution_id` - Optional parent execution ID for correlation tracing
    ///
    /// # Returns
    /// * `Ok(String)` - Execution record ID
    /// * `Err(ToolError)` - Database error
    pub async fn create_execution_record_with_parent(
        &self,
        child_agent_id: &str,
        child_agent_name: &str,
        prompt: &str,
        parent_execution_id: Option<String>,
    ) -> ToolResult<String> {
        let execution_id = Uuid::new_v4().to_string();

        let mut execution_create = SubAgentExecutionCreate::with_parent(
            self.workflow_id.clone(),
            self.parent_agent_id.clone(),
            child_agent_id.to_string(),
            child_agent_name.to_string(),
            prompt.to_string(),
            parent_execution_id.clone(),
        );
        execution_create.status = "running".to_string();

        self.db
            .create("sub_agent_execution", &execution_id, execution_create)
            .await
            .map_err(|e| {
                ToolError::DatabaseError(format!("Failed to create execution record: {}", e))
            })?;

        if let Some(ref parent_id) = parent_execution_id {
            debug!(
                execution_id = %execution_id,
                parent_execution_id = %parent_id,
                child_agent_id = %child_agent_id,
                workflow_id = %self.workflow_id,
                "Created sub-agent execution record with parent correlation"
            );
        } else {
            debug!(
                execution_id = %execution_id,
                child_agent_id = %child_agent_id,
                workflow_id = %self.workflow_id,
                "Created sub-agent execution record"
            );
        }

        Ok(execution_id)
    }

    /// Updates the execution record with the result.
    ///
    /// Persists cache_tokens, cache_write_tokens, thinking_tokens and the
    /// per-sub-agent cost (computed with the SUB-AGENT's own pricing — not
    /// the parent's), in addition to the basic status / tokens / summary.
    ///
    /// # Arguments
    /// * `execution_id` - Execution record ID
    /// * `result` - Execution result with success, report, metrics
    pub async fn update_execution_record(&self, execution_id: &str, result: &ExecutionResult) {
        let status = if result.success { "completed" } else { "error" };
        let result_summary = safe_truncate(&result.report, 5000, true);

        let update_query = format!(
            "UPDATE sub_agent_execution:`{}` SET \
             status = $status, \
             duration_ms = $duration_ms, \
             tokens_input = $tokens_input, \
             tokens_output = $tokens_output, \
             cached_tokens = $cached_tokens, \
             cache_write_tokens = $cache_write_tokens, \
             thinking_tokens = $thinking_tokens, \
             cost_usd = $cost_usd, \
             result_summary = $result_summary, \
             error_message = $error_message, \
             completed_at = time::now()",
            execution_id,
        );

        let error_message_value = result
            .error_message
            .as_ref()
            .map(|s| serde_json::Value::String(s.clone()))
            .unwrap_or(serde_json::Value::Null);

        // Wrap optional u64/f64 as JSON null when absent.
        let opt_u64 = |v: Option<u64>| match v {
            Some(n) => serde_json::json!(n),
            None => serde_json::Value::Null,
        };
        let opt_f64 = |v: Option<f64>| match v {
            Some(n) => serde_json::json!(n),
            None => serde_json::Value::Null,
        };

        if let Err(e) = self
            .db
            .execute_with_params(
                &update_query,
                vec![
                    ("status".to_string(), serde_json::json!(status)),
                    (
                        "duration_ms".to_string(),
                        serde_json::json!(result.metrics.duration_ms),
                    ),
                    (
                        "tokens_input".to_string(),
                        serde_json::json!(result.metrics.tokens_input),
                    ),
                    (
                        "tokens_output".to_string(),
                        serde_json::json!(result.metrics.tokens_output),
                    ),
                    (
                        "cached_tokens".to_string(),
                        opt_u64(result.metrics.cached_tokens),
                    ),
                    (
                        "cache_write_tokens".to_string(),
                        opt_u64(result.metrics.cache_write_tokens),
                    ),
                    (
                        "thinking_tokens".to_string(),
                        opt_u64(result.metrics.thinking_tokens),
                    ),
                    ("cost_usd".to_string(), opt_f64(result.metrics.cost_usd)),
                    (
                        "result_summary".to_string(),
                        serde_json::Value::String(result_summary),
                    ),
                    ("error_message".to_string(), error_message_value),
                ],
            )
            .await
        {
            warn!(
                execution_id = %execution_id,
                error = %e,
                "Failed to update execution record"
            );
        }
    }

    /// Persists internal tool executions and reasoning steps from a sub-agent.
    ///
    /// This captures data that was previously silently dropped in `execute_with_heartbeat_timeout`.
    /// The data is saved to the same `tool_execution` and `thinking_step` tables as primary agent
    /// data, but with the sub-agent's ID as `agent_id` for attribution.
    ///
    /// # Arguments
    /// * `execution_id` - Sub-agent execution record ID (used as message_id for correlation)
    /// * `agent_id` - Sub-agent ID
    /// * `result` - Execution result containing tool_executions and reasoning_steps
    pub async fn persist_sub_agent_internals(
        &self,
        execution_id: &str,
        agent_id: &str,
        result: &ExecutionResult,
    ) {
        if result.tool_executions.is_empty() && result.reasoning_steps.is_empty() {
            return;
        }

        info!(
            agent_id = %agent_id,
            tool_executions = result.tool_executions.len(),
            reasoning_steps = result.reasoning_steps.len(),
            "Persisting sub-agent internal data"
        );

        crate::db::persist_tool_executions(
            &self.db,
            &result.tool_executions,
            &self.workflow_id,
            execution_id,
            agent_id,
        )
        .await;

        crate::db::persist_reasoning_steps(
            &self.db,
            &result.reasoning_steps,
            &self.workflow_id,
            execution_id,
            agent_id,
            0,
        )
        .await;
    }

    /// Emits a streaming event.
    ///
    /// # Arguments
    /// * `event_name` - Event name (e.g., events::WORKFLOW_STREAM)
    /// * `chunk` - Stream chunk to emit
    pub fn emit_event(&self, event_name: &str, chunk: &StreamChunk) {
        if let Some(ref handle) = self.app_handle {
            if let Err(e) = handle.emit(event_name, chunk) {
                warn!(
                    event = %event_name,
                    error = %e,
                    "Failed to emit sub-agent event"
                );
            }
        }
    }

    /// Emits execution start event.
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID
    /// * `agent_name` - Agent name
    /// * `prompt` - Task prompt
    pub fn emit_start_event(&self, agent_id: &str, agent_name: &str, prompt: &str) {
        let chunk = StreamChunk::sub_agent_start(
            self.workflow_id.clone(),
            agent_id.to_string(),
            agent_name.to_string(),
            self.parent_agent_id.clone(),
            prompt.to_string(),
        );
        self.emit_event(events::WORKFLOW_STREAM, &chunk);
    }

    /// Emits execution complete event.
    ///
    /// # Arguments
    /// * `agent_id` - Agent ID
    /// * `agent_name` - Agent name
    /// * `result` - Execution result
    pub fn emit_complete_event(&self, agent_id: &str, agent_name: &str, result: &ExecutionResult) {
        if result.success {
            let chunk = StreamChunk::sub_agent_complete(
                self.workflow_id.clone(),
                agent_id.to_string(),
                agent_name.to_string(),
                self.parent_agent_id.clone(),
                result.report.clone(),
                SubAgentStreamMetrics {
                    duration_ms: result.metrics.duration_ms,
                    tokens_input: result.metrics.tokens_input,
                    tokens_output: result.metrics.tokens_output,
                },
            );
            self.emit_event(events::WORKFLOW_STREAM, &chunk);
        } else {
            let chunk = StreamChunk::sub_agent_error(
                self.workflow_id.clone(),
                agent_id.to_string(),
                agent_name.to_string(),
                self.parent_agent_id.clone(),
                result.error_message.clone().unwrap_or_default(),
                result.metrics.duration_ms,
            );
            self.emit_event(events::WORKFLOW_STREAM, &chunk);
        }
    }
}
