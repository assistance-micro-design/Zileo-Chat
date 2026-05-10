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
 * @fileoverview Streaming event types for real-time workflow execution.
 *
 * These types are synchronized with Rust backend types (src-tauri/src/models/streaming.rs)
 * to ensure type safety for Tauri event streaming.
 *
 * @module types/streaming
 */

import type { UserQuestionStreamPayload } from './user-question';

/**
 * Type of streaming chunk content.
 *
 * Synchronized with Rust `ChunkType` enum in `src-tauri/src/models/streaming.rs`.
 */
export type ChunkType =
	| 'tool_start'
	| 'reasoning'
	| 'error'
	| 'sub_agent_start'
	| 'sub_agent_complete'
	| 'sub_agent_error'
	| 'task_create'
	| 'task_update'
	| 'task_complete'
	| 'user_question_start'
	| 'user_question_complete'
	| 'thinking_block'
	| 'tool_call_complete'
	| 'response_block'
	| 'iteration_progress';

/**
 * Metrics included in sub-agent complete events.
 *
 * Synchronized with Rust `SubAgentStreamMetrics` in streaming.rs.
 */
export interface SubAgentStreamMetrics {
	/** Execution duration in milliseconds */
	duration_ms: number;
	/** Input tokens consumed */
	tokens_input: number;
	/** Output tokens generated */
	tokens_output: number;
}

/**
 * Streaming chunk emitted during workflow execution.
 *
 * Synchronized with Rust `StreamChunk` in `src-tauri/src/models/streaming.rs`.
 */
export interface StreamChunk {
	/** Associated workflow ID */
	workflow_id: string;
	/** Type of chunk content */
	chunk_type: ChunkType;
	/** Text content (for token/reasoning/error/sub_agent chunks) */
	content?: string;
	/** Tool name (for tool_start/tool_call_complete chunks) */
	tool?: string;
	/** Duration in milliseconds (for tool_call_complete/sub_agent_complete/sub_agent_error/task_complete chunks) */
	duration?: number;
	/** Sub-agent ID (for sub_agent_* chunks) */
	sub_agent_id?: string;
	/** Sub-agent name (for sub_agent_* chunks) */
	sub_agent_name?: string;
	/** Parent agent ID (for sub_agent_* chunks) */
	parent_agent_id?: string;
	/** Sub-agent metrics (for sub_agent_complete chunks) */
	metrics?: SubAgentStreamMetrics;
	/** Progress percentage 0-100 (for sub_agent_complete chunks; always 100) */
	progress?: number;
	/** Task ID (for task_* chunks) */
	task_id?: string;
	/** Task name (for task_* chunks) */
	task_name?: string;
	/** Task status (for task_* chunks) */
	task_status?: 'pending' | 'in_progress' | 'completed' | 'blocked';
	/** Task priority (for task_* chunks) */
	task_priority?: 1 | 2 | 3 | 4 | 5;
	/** Agent name associated with task (for task_* chunks) */
	task_agent_name?: string;
	/** User question payload (for user_question_start chunks) */
	user_question?: UserQuestionStreamPayload;
	/** Question ID (for user_question_complete chunks) */
	question_id?: string;
	/** Tool type: "local" or "mcp" (for tool_call_complete) */
	tool_type?: 'local' | 'mcp';
	/** MCP server name (for tool_call_complete, only for MCP tools) */
	server_name?: string;
	/** Tool input parameters as JSON string (for tool_call_complete) */
	tool_input?: string;
	/** Tool output result as JSON string (for tool_call_complete) */
	tool_output?: string;
	/** Tool execution success/failure (for tool_call_complete) */
	tool_success?: boolean;
	/**
	 * Tool error message when `tool_success === false` (for tool_call_complete).
	 * Mirrors the persisted `ToolExecution.error_message`, so the live UI can
	 * render failures with the same level of detail as the post-reload view.
	 */
	error_message?: string;
	/** Input tokens count (for response_block) */
	tokens_input?: number;
	/** Output tokens count (for response_block) */
	tokens_output?: number;
	/** Cached input tokens count (for response_block) */
	cached_tokens?: number;
	/** Cache-write tokens count (for response_block) */
	cache_write_tokens?: number;
	/** Thinking/reasoning tokens count (for response_block) */
	thinking_tokens?: number;
	/**
	 * Per-iteration cost in USD computed by the backend pricing layer
	 * (for response_block). Frontend never multiplies tokens by prices itself —
	 * this field carries the authoritative number so a backgrounded workflow
	 * can accumulate `partialCostUsd` on its bg execution.
	 */
	cost_usd?: number;
	/**
	 * 1-based iteration index for `iteration_progress` chunks emitted from
	 * inside the tool loop. Useful for diagnostics; the metrics handler keys
	 * off the cumulative tokens, not the iteration number.
	 */
	iteration?: number;
	/**
	 * Input tokens consumed by the LATEST LLM call only (= context window
	 * usage of that single call). Distinct from `tokens_input`, which is the
	 * cumulative sum across iterations. Drives the "/ context_max" gauge so
	 * it tracks the last call rather than the cumulative total.
	 */
	iter_input?: number;
	/**
	 * `true` when the chunk is emitted from a delegated sub-agent rather than
	 * the orchestrator. The frontend skips these chunks for the orchestrator's
	 * metrics bar so a sub-agent's TokenTracker (which resets to 0) cannot
	 * stomp the parent's running totals.
	 */
	is_sub_agent?: boolean;
}

/**
 * Event emitted when workflow execution completes
 */
export interface WorkflowComplete {
	/** Associated workflow ID */
	workflow_id: string;
	/** Final workflow status */
	status: 'completed' | 'error' | 'cancelled';
	/** Error message if status is 'error' */
	error?: string;
}
