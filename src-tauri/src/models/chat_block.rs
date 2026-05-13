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

//! Chat block models for unified block-by-block display.
//!
//! This module provides types for representing execution blocks (thinking steps,
//! tool calls, sub-agent completions) in a unified, ordered format for display
//! in the chat interface.
//!
//! Provides block-level persistence and loading for chat display.

use crate::models::sub_agent::SubAgentExecution;
use crate::models::{ThinkingStep, ToolExecution};
use serde::Serialize;
use std::collections::HashMap;

/// Block type indicating what kind of execution block this is.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatBlockType {
    /// Thinking/reasoning step from model or agent flow
    Thinking,
    /// Tool call execution (local or MCP)
    ToolCall,
    /// Sub-agent execution completion
    SubAgent,
}

impl std::fmt::Display for ChatBlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChatBlockType::Thinking => write!(f, "thinking"),
            ChatBlockType::ToolCall => write!(f, "tool_call"),
            ChatBlockType::SubAgent => write!(f, "sub_agent"),
        }
    }
}

/// A unified chat block for display in the chat interface.
///
/// Combines thinking steps and tool executions into a single ordered stream
/// sorted by sequence number for chronological display.
#[derive(Debug, Clone, Serialize)]
pub struct ChatBlock {
    /// Block type (thinking, tool_call, sub_agent)
    pub block_type: ChatBlockType,
    /// Global ordering sequence within the message execution
    pub sequence: u32,
    /// Block-specific data (varies by block_type)
    pub data: serde_json::Value,
}

/// Merges tool executions, thinking steps, and sub-agent executions into a unified,
/// chronologically ordered list of ChatBlocks.
///
/// Sorting is primarily by `created_at`, with `sequence` as a stable tie-breaker
/// for tool/thinking records that share the same timestamp. Sub-agent records use
/// `created_at` only (they have no shared sequence numbering with tool/thinking).
///
/// After sorting, the final `sequence` field on each ChatBlock is re-assigned as a
/// dense 0..n index so the frontend can use it as a stable rendering key.
///
/// # Arguments
/// * `tool_executions` - Tool execution records for a message
/// * `thinking_steps` - Thinking step records for a message
/// * `sub_agent_executions` - Sub-agent execution records for a message
/// * `agent_name_lookup` - Map agent_id -> agent_name to project the originating
///   agent's display name onto Tool and Thinking blocks. Missing entries leave
///   `agent_name` absent in the projected JSON (frontend falls back on
///   `agent_id`). Pass `&HashMap::new()` when the caller has no registry access.
///
/// # Returns
/// A vector of ChatBlocks ordered chronologically with re-indexed sequence numbers
pub fn merge_into_chat_blocks(
    tool_executions: &[ToolExecution],
    thinking_steps: &[ThinkingStep],
    sub_agent_executions: &[SubAgentExecution],
    agent_name_lookup: &HashMap<String, String>,
) -> Vec<ChatBlock> {
    enum SourceItem<'a> {
        Tool(&'a ToolExecution),
        Thinking(&'a ThinkingStep),
        SubAgent(&'a SubAgentExecution),
    }

    let mut items: Vec<(chrono::DateTime<chrono::Utc>, u32, SourceItem)> = Vec::with_capacity(
        tool_executions.len() + thinking_steps.len() + sub_agent_executions.len(),
    );

    for te in tool_executions {
        items.push((te.created_at, te.sequence, SourceItem::Tool(te)));
    }
    for ts in thinking_steps {
        items.push((ts.created_at, ts.sequence, SourceItem::Thinking(ts)));
    }
    for sa in sub_agent_executions {
        // Sub-agents share no sequence space with tool/thinking; tie-break by 0
        items.push((sa.created_at, 0, SourceItem::SubAgent(sa)));
    }

    items.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

    items
        .into_iter()
        .enumerate()
        .map(|(idx, (_, _, source))| {
            let sequence = idx as u32;
            match source {
                SourceItem::Tool(te) => {
                    // Frontend expects JSON strings for input/output, not nested objects
                    let input_str = serde_json::to_string(&te.input_params).unwrap_or_default();
                    let output_str = serde_json::to_string(&te.output_result).unwrap_or_default();
                    let agent_name = agent_name_lookup.get(&te.agent_id).cloned();

                    let data = serde_json::json!({
                        "tool_name": te.tool_name,
                        "tool_type": te.tool_type,
                        "server_name": te.server_name,
                        "input_params": input_str,
                        "output_result": output_str,
                        "success": te.success,
                        "error_message": te.error_message,
                        "duration_ms": te.duration_ms,
                        "agent_id": te.agent_id,
                        "agent_name": agent_name,
                    });

                    ChatBlock {
                        block_type: ChatBlockType::ToolCall,
                        sequence,
                        data,
                    }
                }
                SourceItem::Thinking(ts) => {
                    let agent_name = agent_name_lookup.get(&ts.agent_id).cloned();
                    let data = serde_json::json!({
                        "content": ts.content,
                        "source": ts.source,
                        "duration_ms": ts.duration_ms,
                        "agent_id": ts.agent_id,
                        "agent_name": agent_name,
                    });

                    ChatBlock {
                        block_type: ChatBlockType::Thinking,
                        sequence,
                        data,
                    }
                }
                SourceItem::SubAgent(sa) => {
                    let status = match sa.status.to_string().as_str() {
                        "completed" => "completed",
                        "error" => "error",
                        _ => "completed",
                    };

                    let data = serde_json::json!({
                        "agent_name": sa.sub_agent_name,
                        "status": status,
                        "duration_ms": sa.duration_ms,
                        "tokens_input": sa.tokens_input,
                        "tokens_output": sa.tokens_output,
                        "report_summary": sa.result_summary,
                    });

                    ChatBlock {
                        block_type: ChatBlockType::SubAgent,
                        sequence,
                        data,
                    }
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::sub_agent::{SubAgentExecution, SubAgentStatus};
    use crate::models::tool_execution::ToolType;
    use crate::models::{ThinkingStep, ToolExecution};
    use chrono::{DateTime, TimeZone, Utc};

    /// Returns a deterministic UTC base time. Tests offset from this value to
    /// produce explicit `created_at` ordering, since the merge sorts by timestamp.
    fn base_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap()
    }

    fn make_tool_execution(
        tool_name: &str,
        sequence: u32,
        success: bool,
        created_at: DateTime<Utc>,
    ) -> ToolExecution {
        ToolExecution {
            id: format!("te_{}", sequence),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_001".to_string(),
            agent_id: "agent_001".to_string(),
            tool_type: ToolType::Local,
            tool_name: tool_name.to_string(),
            server_name: None,
            input_params: serde_json::json!({"query": "test"}),
            output_result: serde_json::json!({"result": "ok"}),
            success,
            error_message: None,
            duration_ms: 100,
            iteration: 0,
            sequence,
            created_at,
        }
    }

    fn make_thinking_step(
        content: &str,
        sequence: u32,
        source: &str,
        created_at: DateTime<Utc>,
    ) -> ThinkingStep {
        ThinkingStep {
            id: format!("ts_{}", sequence),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_001".to_string(),
            agent_id: "agent_001".to_string(),
            step_number: 0,
            content: content.to_string(),
            duration_ms: Some(50),
            tokens: None,
            sequence,
            source: source.to_string(),
            created_at,
        }
    }

    fn make_sub_agent_execution(
        sub_agent_name: &str,
        status: SubAgentStatus,
        created_at: DateTime<Utc>,
    ) -> SubAgentExecution {
        SubAgentExecution {
            id: format!("sa_{}", sub_agent_name),
            workflow_id: "wf_001".to_string(),
            parent_agent_id: "agent_001".to_string(),
            sub_agent_id: format!("agent_{}", sub_agent_name),
            sub_agent_name: sub_agent_name.to_string(),
            task_description: "test task".to_string(),
            status,
            duration_ms: Some(500),
            tokens_input: Some(100),
            tokens_output: Some(50),
            result_summary: Some("done".to_string()),
            error_message: None,
            parent_execution_id: None,
            parent_message_id: None,
            created_at,
            completed_at: None,
        }
    }

    #[test]
    fn test_chat_block_type_serialization() {
        let thinking = ChatBlockType::Thinking;
        let json = serde_json::to_string(&thinking).unwrap();
        assert_eq!(json, "\"thinking\"");

        let tool_call = ChatBlockType::ToolCall;
        let json = serde_json::to_string(&tool_call).unwrap();
        assert_eq!(json, "\"tool_call\"");

        let sub_agent = ChatBlockType::SubAgent;
        let json = serde_json::to_string(&sub_agent).unwrap();
        assert_eq!(json, "\"sub_agent\"");
    }

    #[test]
    fn test_chat_block_type_display() {
        assert_eq!(ChatBlockType::Thinking.to_string(), "thinking");
        assert_eq!(ChatBlockType::ToolCall.to_string(), "tool_call");
        assert_eq!(ChatBlockType::SubAgent.to_string(), "sub_agent");
    }

    #[test]
    fn test_merge_empty_inputs() {
        let blocks = merge_into_chat_blocks(&[], &[], &[], &HashMap::new());
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_merge_only_tool_executions() {
        let t = base_time();
        let tools = vec![
            make_tool_execution("MemoryTool", 1, true, t),
            make_tool_execution("TodoTool", 3, true, t + chrono::Duration::milliseconds(10)),
        ];

        let blocks = merge_into_chat_blocks(&tools, &[], &[], &HashMap::new());

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 1);
        // Original ordering preserved (MemoryTool before TodoTool)
        assert_eq!(blocks[0].data["tool_name"], "MemoryTool");
        assert_eq!(blocks[1].data["tool_name"], "TodoTool");
    }

    #[test]
    fn test_merge_only_thinking_steps() {
        let t = base_time();
        let steps = vec![
            make_thinking_step("Analyzing task...", 0, "agent_flow", t),
            make_thinking_step(
                "Deep reasoning...",
                2,
                "model_thinking",
                t + chrono::Duration::milliseconds(10),
            ),
        ];

        let blocks = merge_into_chat_blocks(&[], &steps, &[], &HashMap::new());

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 1);
        assert_eq!(blocks[0].data["content"], "Analyzing task...");
        assert_eq!(blocks[1].data["content"], "Deep reasoning...");
    }

    #[test]
    fn test_merge_interleaved_blocks() {
        // Stagger created_at so that the chronological order matches the original
        // sequence intent: thinking(1), tool(2), thinking(3), tool(4), thinking(5)
        let t = base_time();
        let ms = |n: i64| t + chrono::Duration::milliseconds(n);
        let tools = vec![
            make_tool_execution("MemoryTool", 2, true, ms(20)),
            make_tool_execution("TodoTool", 4, false, ms(40)),
        ];
        let steps = vec![
            make_thinking_step("Analyzing task...", 1, "agent_flow", ms(10)),
            make_thinking_step("Deep reasoning...", 3, "model_thinking", ms(30)),
            make_thinking_step("Summarizing...", 5, "agent_flow", ms(50)),
        ];

        let blocks = merge_into_chat_blocks(&tools, &steps, &[], &HashMap::new());

        assert_eq!(blocks.len(), 5);
        // Sequences are now dense 0..n indices
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[0].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[1].sequence, 1);
        assert_eq!(blocks[1].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[2].sequence, 2);
        assert_eq!(blocks[2].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[3].sequence, 3);
        assert_eq!(blocks[3].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[4].sequence, 4);
        assert_eq!(blocks[4].block_type, ChatBlockType::Thinking);
    }

    #[test]
    fn test_merge_subagent_interleaved_by_created_at() {
        // Sub-agent inserted between two tool executions chronologically must
        // appear in the middle, not appended at the end.
        let t = base_time();
        let ms = |n: i64| t + chrono::Duration::milliseconds(n);
        let tools = vec![
            make_tool_execution("FirstTool", 1, true, ms(10)),
            make_tool_execution("LastTool", 2, true, ms(30)),
        ];
        let sub_agents = vec![make_sub_agent_execution(
            "Researcher",
            SubAgentStatus::Completed,
            ms(20),
        )];

        let blocks = merge_into_chat_blocks(&tools, &[], &sub_agents, &HashMap::new());

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[0].data["tool_name"], "FirstTool");

        assert_eq!(blocks[1].block_type, ChatBlockType::SubAgent);
        assert_eq!(blocks[1].sequence, 1);
        assert_eq!(blocks[1].data["agent_name"], "Researcher");

        assert_eq!(blocks[2].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[2].sequence, 2);
        assert_eq!(blocks[2].data["tool_name"], "LastTool");
    }

    #[test]
    fn test_merge_same_timestamp_uses_sequence_tiebreaker() {
        // When two records share an exact timestamp, the original `sequence`
        // value breaks the tie deterministically.
        let t = base_time();
        let tools = vec![make_tool_execution("MemoryTool", 5, true, t)];
        let steps = vec![make_thinking_step("Same time step", 2, "agent_flow", t)];

        let blocks = merge_into_chat_blocks(&tools, &steps, &[], &HashMap::new());

        assert_eq!(blocks.len(), 2);
        // Lower original sequence (thinking=2) sorts before higher (tool=5)
        assert_eq!(blocks[0].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[1].block_type, ChatBlockType::ToolCall);
        // Re-indexed densely
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 1);
    }

    #[test]
    fn test_tool_call_block_data_contains_expected_fields() {
        let tools = vec![make_tool_execution("MemoryTool", 1, true, base_time())];
        let blocks = merge_into_chat_blocks(&tools, &[], &[], &HashMap::new());

        assert_eq!(blocks.len(), 1);
        let data = &blocks[0].data;

        assert_eq!(data["tool_name"], "MemoryTool");
        assert_eq!(data["tool_type"], "local");
        assert_eq!(data["success"], true);
        assert_eq!(data["duration_ms"], 100);
        assert!(data["input_params"].is_string());
        assert!(data["output_result"].is_string());
    }

    #[test]
    fn test_tool_call_block_data_with_mcp() {
        let mut te = make_tool_execution("find_symbol", 1, true, base_time());
        te.tool_type = ToolType::Mcp;
        te.server_name = Some("serena".to_string());

        let blocks = merge_into_chat_blocks(&[te], &[], &[], &HashMap::new());

        let data = &blocks[0].data;
        assert_eq!(data["tool_type"], "mcp");
        assert_eq!(data["server_name"], "serena");
    }

    #[test]
    fn test_tool_call_block_data_with_error() {
        let mut te = make_tool_execution("TodoTool", 1, false, base_time());
        te.error_message = Some("Task not found".to_string());

        let blocks = merge_into_chat_blocks(&[te], &[], &[], &HashMap::new());

        let data = &blocks[0].data;
        assert_eq!(data["success"], false);
        assert_eq!(data["error_message"], "Task not found");
    }

    #[test]
    fn test_thinking_block_data_contains_expected_fields() {
        let steps = vec![make_thinking_step(
            "Deep reasoning...",
            1,
            "model_thinking",
            base_time(),
        )];
        let blocks = merge_into_chat_blocks(&[], &steps, &[], &HashMap::new());

        assert_eq!(blocks.len(), 1);
        let data = &blocks[0].data;

        assert_eq!(data["content"], "Deep reasoning...");
        assert_eq!(data["source"], "model_thinking");
        assert_eq!(data["duration_ms"], 50);
    }

    #[test]
    fn test_thinking_block_agent_flow_source() {
        let steps = vec![make_thinking_step(
            "Analyzing...",
            0,
            "agent_flow",
            base_time(),
        )];
        let blocks = merge_into_chat_blocks(&[], &steps, &[], &HashMap::new());

        assert_eq!(blocks[0].data["source"], "agent_flow");
    }

    #[test]
    fn test_chat_block_serialization() {
        let block = ChatBlock {
            block_type: ChatBlockType::Thinking,
            sequence: 1,
            data: serde_json::json!({
                "content": "test",
                "source": "agent_flow",
            }),
        };

        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("\"block_type\":\"thinking\""));
        assert!(json.contains("\"sequence\":1"));
        assert!(json.contains("\"content\":\"test\""));
    }

    #[test]
    fn test_merge_projects_agent_id_for_tool_blocks() {
        // Each ToolCall block must surface its originating agent_id so the
        // frontend can apply the sub-agent visual treatment at replay.
        let tools = vec![make_tool_execution("Tool", 1, true, base_time())];
        let blocks = merge_into_chat_blocks(&tools, &[], &[], &HashMap::new());

        let data = &blocks[0].data;
        assert_eq!(data["agent_id"], "agent_001");
    }

    #[test]
    fn test_merge_projects_agent_id_for_thinking_blocks() {
        let steps = vec![make_thinking_step(
            "Reasoning",
            1,
            "agent_flow",
            base_time(),
        )];
        let blocks = merge_into_chat_blocks(&[], &steps, &[], &HashMap::new());

        let data = &blocks[0].data;
        assert_eq!(data["agent_id"], "agent_001");
    }

    #[test]
    fn test_merge_projects_agent_name_when_lookup_hit() {
        let tools = vec![make_tool_execution("Tool", 1, true, base_time())];
        let steps = vec![make_thinking_step("R", 2, "agent_flow", base_time())];
        let mut lookup = HashMap::new();
        lookup.insert("agent_001".to_string(), "Marie".to_string());

        let blocks = merge_into_chat_blocks(&tools, &steps, &[], &lookup);

        assert_eq!(blocks[0].data["agent_name"], "Marie");
        assert_eq!(blocks[1].data["agent_name"], "Marie");
    }

    #[test]
    fn test_merge_omits_agent_name_when_lookup_miss() {
        // When the lookup map has no entry for the agent_id, the projected
        // agent_name MUST be null so the frontend can fall back on the id.
        let tools = vec![make_tool_execution("Tool", 1, true, base_time())];
        let blocks = merge_into_chat_blocks(&tools, &[], &[], &HashMap::new());

        let data = &blocks[0].data;
        assert!(
            data["agent_name"].is_null(),
            "agent_name must be null when lookup misses, got {:?}",
            data["agent_name"]
        );
        // agent_id is still present so frontend has something to render
        assert_eq!(data["agent_id"], "agent_001");
    }

    #[test]
    fn test_merge_reindexes_sequences_densely() {
        // Original sequences (50, 100) are replaced by dense 0..n indices
        // after sorting by created_at.
        let t = base_time();
        let tools = vec![make_tool_execution(
            "Tool",
            100,
            true,
            t + chrono::Duration::milliseconds(20),
        )];
        let steps = vec![make_thinking_step("Step", 50, "agent_flow", t)];

        let blocks = merge_into_chat_blocks(&tools, &steps, &[], &HashMap::new());

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[1].sequence, 1);
    }
}
