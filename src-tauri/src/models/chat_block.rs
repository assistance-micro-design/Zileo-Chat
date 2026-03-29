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
/// ordered list of ChatBlocks.
///
/// All inputs should already be sorted by sequence/time, but the merge re-sorts
/// the combined result to ensure correct interleaving.
///
/// # Arguments
/// * `tool_executions` - Tool execution records for a message
/// * `thinking_steps` - Thinking step records for a message
/// * `sub_agent_executions` - Sub-agent execution records for a message
///
/// # Returns
/// A vector of ChatBlocks sorted by sequence number
pub fn merge_into_chat_blocks(
    tool_executions: &[ToolExecution],
    thinking_steps: &[ThinkingStep],
    sub_agent_executions: &[SubAgentExecution],
) -> Vec<ChatBlock> {
    let mut blocks: Vec<ChatBlock> = Vec::with_capacity(
        tool_executions.len() + thinking_steps.len() + sub_agent_executions.len(),
    );

    // Convert tool executions to ChatBlocks
    for te in tool_executions {
        // Serialize Value fields back to JSON strings (frontend expects strings, not objects)
        let input_str = serde_json::to_string(&te.input_params).unwrap_or_default();
        let output_str = serde_json::to_string(&te.output_result).unwrap_or_default();

        let data = serde_json::json!({
            "tool_name": te.tool_name,
            "tool_type": te.tool_type,
            "server_name": te.server_name,
            "input_params": input_str,
            "output_result": output_str,
            "success": te.success,
            "error_message": te.error_message,
            "duration_ms": te.duration_ms,
        });

        blocks.push(ChatBlock {
            block_type: ChatBlockType::ToolCall,
            sequence: te.sequence,
            data,
        });
    }

    // Convert thinking steps to ChatBlocks
    for ts in thinking_steps {
        let data = serde_json::json!({
            "content": ts.content,
            "source": ts.source,
            "duration_ms": ts.duration_ms,
        });

        blocks.push(ChatBlock {
            block_type: ChatBlockType::Thinking,
            sequence: ts.sequence,
            data,
        });
    }

    // Convert sub-agent executions to ChatBlocks
    // Sub-agent blocks use sequence u32::MAX - index to sort after tool/thinking blocks
    // (they don't have a global_sequence from the tool loop, they run in parallel)
    let max_sequence = blocks.iter().map(|b| b.sequence).max().unwrap_or(0);
    for (idx, sa) in sub_agent_executions.iter().enumerate() {
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

        blocks.push(ChatBlock {
            block_type: ChatBlockType::SubAgent,
            sequence: max_sequence + 1 + idx as u32,
            data,
        });
    }

    // Sort by sequence for correct chronological ordering
    blocks.sort_by_key(|b| b.sequence);

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::tool_execution::ToolType;
    use crate::models::{ThinkingStep, ToolExecution};
    use chrono::Utc;

    fn make_tool_execution(tool_name: &str, sequence: u32, success: bool) -> ToolExecution {
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
            created_at: Utc::now(),
        }
    }

    fn make_thinking_step(content: &str, sequence: u32, source: &str) -> ThinkingStep {
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
            created_at: Utc::now(),
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
        let blocks = merge_into_chat_blocks(&[], &[], &[]);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_merge_only_tool_executions() {
        let tools = vec![
            make_tool_execution("MemoryTool", 1, true),
            make_tool_execution("TodoTool", 3, true),
        ];

        let blocks = merge_into_chat_blocks(&tools, &[], &[]);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[0].sequence, 1);
        assert_eq!(blocks[1].sequence, 3);
    }

    #[test]
    fn test_merge_only_thinking_steps() {
        let steps = vec![
            make_thinking_step("Analyzing task...", 0, "agent_flow"),
            make_thinking_step("Deep reasoning...", 2, "model_thinking"),
        ];

        let blocks = merge_into_chat_blocks(&[], &steps, &[]);

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 2);
    }

    #[test]
    fn test_merge_interleaved_blocks() {
        let tools = vec![
            make_tool_execution("MemoryTool", 2, true),
            make_tool_execution("TodoTool", 4, false),
        ];
        let steps = vec![
            make_thinking_step("Analyzing task...", 1, "agent_flow"),
            make_thinking_step("Deep reasoning...", 3, "model_thinking"),
            make_thinking_step("Summarizing...", 5, "agent_flow"),
        ];

        let blocks = merge_into_chat_blocks(&tools, &steps, &[]);

        assert_eq!(blocks.len(), 5);
        // Verify correct interleaving order
        assert_eq!(blocks[0].sequence, 1);
        assert_eq!(blocks[0].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[1].sequence, 2);
        assert_eq!(blocks[1].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[2].sequence, 3);
        assert_eq!(blocks[2].block_type, ChatBlockType::Thinking);
        assert_eq!(blocks[3].sequence, 4);
        assert_eq!(blocks[3].block_type, ChatBlockType::ToolCall);
        assert_eq!(blocks[4].sequence, 5);
        assert_eq!(blocks[4].block_type, ChatBlockType::Thinking);
    }

    #[test]
    fn test_merge_same_sequence_stable_order() {
        // When sequence is the same (legacy data with sequence=0),
        // both blocks should still be present
        let tools = vec![make_tool_execution("MemoryTool", 0, true)];
        let steps = vec![make_thinking_step("Legacy step", 0, "agent_flow")];

        let blocks = merge_into_chat_blocks(&tools, &steps, &[]);

        assert_eq!(blocks.len(), 2);
        // Both should have sequence 0
        assert_eq!(blocks[0].sequence, 0);
        assert_eq!(blocks[1].sequence, 0);
    }

    #[test]
    fn test_tool_call_block_data_contains_expected_fields() {
        let tools = vec![make_tool_execution("MemoryTool", 1, true)];
        let blocks = merge_into_chat_blocks(&tools, &[], &[]);

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
        let mut te = make_tool_execution("find_symbol", 1, true);
        te.tool_type = ToolType::Mcp;
        te.server_name = Some("serena".to_string());

        let blocks = merge_into_chat_blocks(&[te], &[], &[]);

        let data = &blocks[0].data;
        assert_eq!(data["tool_type"], "mcp");
        assert_eq!(data["server_name"], "serena");
    }

    #[test]
    fn test_tool_call_block_data_with_error() {
        let mut te = make_tool_execution("TodoTool", 1, false);
        te.error_message = Some("Task not found".to_string());

        let blocks = merge_into_chat_blocks(&[te], &[], &[]);

        let data = &blocks[0].data;
        assert_eq!(data["success"], false);
        assert_eq!(data["error_message"], "Task not found");
    }

    #[test]
    fn test_thinking_block_data_contains_expected_fields() {
        let steps = vec![make_thinking_step("Deep reasoning...", 1, "model_thinking")];
        let blocks = merge_into_chat_blocks(&[], &steps, &[]);

        assert_eq!(blocks.len(), 1);
        let data = &blocks[0].data;

        assert_eq!(data["content"], "Deep reasoning...");
        assert_eq!(data["source"], "model_thinking");
        assert_eq!(data["duration_ms"], 50);
    }

    #[test]
    fn test_thinking_block_agent_flow_source() {
        let steps = vec![make_thinking_step("Analyzing...", 0, "agent_flow")];
        let blocks = merge_into_chat_blocks(&[], &steps, &[]);

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
    fn test_merge_large_sequence_numbers() {
        let tools = vec![make_tool_execution("Tool", 100, true)];
        let steps = vec![make_thinking_step("Step", 50, "agent_flow")];

        let blocks = merge_into_chat_blocks(&tools, &steps, &[]);

        assert_eq!(blocks[0].sequence, 50);
        assert_eq!(blocks[1].sequence, 100);
    }
}
