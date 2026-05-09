/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */


/**
 * @fileoverview Chat block types for block-by-block execution display.
 *
 * Synchronized with Rust backend types in `src-tauri/src/models/chat_block.rs`.
 * Used for both real-time streaming blocks and persisted blocks loaded from DB.
 *
 * @module types/chat-block
 */

/**
 * Type of chat block in the execution display.
 *
 * Synchronized with Rust `ChatBlockType` enum.
 */
export type ChatBlockType = 'thinking' | 'tool_call' | 'sub_agent';

/**
 * Unified block for execution display.
 *
 * Synchronized with Rust `ChatBlock` struct.
 * Used for real-time display (from stream events) and persisted display (from DB).
 */
export interface ChatBlock {
	/** Block type determining which data shape to expect */
	block_type: ChatBlockType;
	/** Global ordering sequence within the message execution */
	sequence: number;
	/** Block-specific data (varies by block_type) */
	data: ThinkingBlockData | ToolCallBlockData | SubAgentBlockData;
}

/**
 * Data for thinking/reasoning blocks.
 */
export interface ThinkingBlockData {
	/** Thinking content text */
	content: string;
	/** Origin of the thinking: real model output or synthetic agent flow */
	source: 'model_thinking' | 'agent_flow';
}

/**
 * Data for tool call blocks.
 */
export interface ToolCallBlockData {
	/** Tool name */
	tool_name: string;
	/** Tool type (local built-in or MCP remote) */
	tool_type: 'local' | 'mcp';
	/** MCP server name (for MCP tools only) */
	server_name?: string;
	/** Input parameters as JSON string */
	input_params: string;
	/** Output result as JSON string */
	output_result: string;
	/** Whether the tool execution succeeded */
	success: boolean;
	/** Error message if tool failed */
	error_message?: string;
	/** Execution duration in milliseconds */
	duration_ms: number;
}

/**
 * Data for sub-agent blocks.
 */
export interface SubAgentBlockData {
	/** Sub-agent name */
	agent_name: string;
	/** Execution status */
	status: 'completed' | 'error';
	/** Execution duration in milliseconds */
	duration_ms?: number;
	/** Input tokens consumed */
	tokens_input?: number;
	/** Output tokens generated */
	tokens_output?: number;
	/** Summary of the sub-agent report */
	report_summary?: string;
	/** Internal: sub-agent ID for deduplication (not persisted) */
	_sub_agent_id?: string;
}

/**
 * Display data for a single task in the TodoTasksBlock.
 *
 * Used for both real-time display (from stream events) and persisted display (from DB).
 * Real-time data may have fewer fields populated than persisted data.
 */
export interface TodoTaskDisplay {
	/** Task ID */
	id: string;
	/** Task name */
	name: string;
	/** Task description (available in persisted view) */
	description?: string;
	/** Current task status */
	status: 'pending' | 'in_progress' | 'completed' | 'blocked';
	/** Priority level (1=critical, 5=low) */
	priority: number;
	/** Agent or sub-agent name that owns this task */
	agent_name?: string;
	/** Execution duration in milliseconds (for completed tasks) */
	duration_ms?: number;
}
