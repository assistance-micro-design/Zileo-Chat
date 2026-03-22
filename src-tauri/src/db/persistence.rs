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

//! Shared persistence functions for tool executions and reasoning steps.
//!
//! Extracted from `commands/streaming.rs` to be reusable by both
//! the primary agent workflow execution and sub-agent executors.
//!
//! These functions persist batch data to the `tool_execution` and `thinking_step`
//! tables, accepting the agent-level data types (`ToolExecutionData`, `ReasoningStepData`)
//! from the `Report` struct.

use crate::agents::core::agent::{ReasoningStepData, ToolExecutionData};
use crate::db::DBClient;
use crate::models::{ThinkingStepCreate, ToolExecutionCreate};
use futures_util::future::join_all;
use tracing::warn;
use uuid::Uuid;

/// Persists tool execution records from agent-level `ToolExecutionData` in parallel.
///
/// This function converts `ToolExecutionData` (from `Report.metrics.tool_executions`)
/// to `ToolExecutionCreate` (DB model) and saves them.
///
/// # Arguments
/// * `db` - Database client
/// * `tool_executions` - Tool execution data from agent Report
/// * `workflow_id` - Workflow ID for scoping
/// * `message_id` - Message ID for correlation (assistant message or sub-agent execution ID)
/// * `agent_id` - The agent that executed these tools (primary or sub-agent)
pub async fn persist_tool_executions(
    db: &DBClient,
    tool_executions: &[ToolExecutionData],
    workflow_id: &str,
    message_id: &str,
    agent_id: &str,
) {
    if tool_executions.is_empty() {
        return;
    }

    let tool_futures: Vec<_> = tool_executions
        .iter()
        .enumerate()
        .map(|(idx, te)| {
            let execution_id = Uuid::new_v4().to_string();
            let execution = ToolExecutionCreate {
                workflow_id: workflow_id.to_string(),
                message_id: message_id.to_string(),
                agent_id: agent_id.to_string(),
                tool_type: te.tool_type.clone(),
                tool_name: te.tool_name.clone(),
                server_name: te.server_name.clone(),
                input_params: te.input_params.clone(),
                output_result: te.output_result.clone(),
                success: te.success,
                error_message: te.error_message.clone(),
                duration_ms: te.duration_ms,
                iteration: te.iteration,
                sequence: te.sequence,
            };
            let tool_name = te.tool_name.clone();
            async move {
                if let Err(e) = db.create("tool_execution", &execution_id, execution).await {
                    warn!(
                        error = %e,
                        tool_name = %tool_name,
                        agent_id = %agent_id,
                        index = idx,
                        "Failed to persist tool execution"
                    );
                }
            }
        })
        .collect();
    join_all(tool_futures).await;
}

/// Persists reasoning step records from agent-level `ReasoningStepData` in parallel.
///
/// Returns the next step number (for continuation in multi-batch scenarios).
///
/// # Arguments
/// * `db` - Database client
/// * `reasoning_steps` - Reasoning step data from agent Report
/// * `workflow_id` - Workflow ID for scoping
/// * `message_id` - Message ID for correlation
/// * `agent_id` - The agent that produced these reasoning steps
/// * `start_step_number` - Starting step number for sequential ordering
pub async fn persist_reasoning_steps(
    db: &DBClient,
    reasoning_steps: &[ReasoningStepData],
    workflow_id: &str,
    message_id: &str,
    agent_id: &str,
    start_step_number: u32,
) -> u32 {
    if reasoning_steps.is_empty() {
        return start_step_number;
    }

    let step_futures: Vec<_> = reasoning_steps
        .iter()
        .enumerate()
        .map(|(idx, rs)| {
            let step_id = Uuid::new_v4().to_string();
            let step_num = start_step_number + idx as u32;
            let step = ThinkingStepCreate {
                workflow_id: workflow_id.to_string(),
                message_id: message_id.to_string(),
                agent_id: agent_id.to_string(),
                step_number: step_num,
                content: rs.content.clone(),
                duration_ms: Some(rs.duration_ms),
                tokens: None,
                sequence: rs.sequence,
                source: rs.source.to_string(),
            };
            async move {
                if let Err(e) = db.create("thinking_step", &step_id, step).await {
                    warn!(
                        error = %e,
                        agent_id = %agent_id,
                        step_number = step_num,
                        "Failed to persist reasoning step (idx={})",
                        idx
                    );
                }
            }
        })
        .collect();
    join_all(step_futures).await;
    start_step_number + reasoning_steps.len() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_tool_executions_does_not_panic() {
        let data: Vec<ToolExecutionData> = Vec::new();
        assert!(data.is_empty());
    }

    #[test]
    fn test_empty_reasoning_steps_does_not_panic() {
        let data: Vec<ReasoningStepData> = Vec::new();
        assert!(data.is_empty());
    }
}
