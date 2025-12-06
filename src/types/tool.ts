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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Tool execution types for persistence and display.
 *
 * These types are synchronized with Rust backend types:
 * - src-tauri/src/models/tool_execution.rs (ToolExecution, ToolExecutionCreate)
 * - src-tauri/src/models/workflow.rs (WorkflowToolExecution)
 *
 * Phase 3: Tool Execution Persistence
 *
 * @module types/tool
 */

/**
 * Tool type indicating execution context
 */
export type ToolType = 'local' | 'mcp';

/**
 * Tool execution record from database (matches Rust ToolExecution)
 */
export interface ToolExecution {
	/** Unique identifier (UUID) */
	id: string;
	/** Associated workflow ID */
	workflow_id: string;
	/** Associated message ID */
	message_id: string;
	/** Agent ID that executed the tool */
	agent_id: string;
	/** Tool type (local or mcp) */
	tool_type: ToolType;
	/** Tool name (e.g., "MemoryTool", "find_symbol") */
	tool_name: string;
	/** MCP server name (only for MCP tools) */
	server_name?: string;
	/** Input parameters as JSON */
	input_params: Record<string, unknown>;
	/** Output result as JSON */
	output_result: Record<string, unknown>;
	/** Whether execution was successful */
	success: boolean;
	/** Error message if failed */
	error_message?: string;
	/** Duration in milliseconds */
	duration_ms: number;
	/** Iteration number in tool loop (0-indexed) */
	iteration: number;
	/** Timestamp when recorded */
	created_at: string;
}

/**
 * Tool execution data from workflow result (IPC-friendly version)
 * Matches Rust WorkflowToolExecution
 */
export interface WorkflowToolExecution {
	/** Tool type (local or mcp) */
	tool_type: string;
	/** Tool name */
	tool_name: string;
	/** MCP server name (only for MCP tools) */
	server_name?: string;
	/** Input parameters as JSON */
	input_params: Record<string, unknown>;
	/** Output result as JSON */
	output_result: Record<string, unknown>;
	/** Whether execution was successful */
	success: boolean;
	/** Error message if failed */
	error_message?: string;
	/** Duration in milliseconds */
	duration_ms: number;
	/** Iteration number in tool loop */
	iteration: number;
}

/**
 * Tool execution status for UI display
 */
export type ToolExecutionStatus = 'pending' | 'running' | 'completed' | 'error';

/**
 * Tool execution for real-time display (used in streaming)
 */
export interface ActiveToolExecution {
	/** Tool name or identifier */
	name: string;
	/** Tool type */
	type: ToolType;
	/** MCP server name if applicable */
	serverName?: string;
	/** Current execution status */
	status: ToolExecutionStatus;
	/** Timestamp when execution started */
	startedAt: number;
	/** Duration in milliseconds (when completed) */
	duration?: number;
	/** Error message if failed */
	error?: string;
	/** Iteration number */
	iteration: number;
}

/**
 * Creates a ToolExecution from WorkflowToolExecution with additional context
 *
 * @param wte - Workflow tool execution from result
 * @param context - Additional context (workflow_id, message_id, agent_id)
 * @returns Tool execution record suitable for display
 */
export function createToolExecutionFromWorkflow(
	wte: WorkflowToolExecution,
	context: {
		id: string;
		workflow_id: string;
		message_id: string;
		agent_id: string;
		created_at: string;
	}
): ToolExecution {
	return {
		id: context.id,
		workflow_id: context.workflow_id,
		message_id: context.message_id,
		agent_id: context.agent_id,
		tool_type: wte.tool_type as ToolType,
		tool_name: wte.tool_name,
		server_name: wte.server_name,
		input_params: wte.input_params,
		output_result: wte.output_result,
		success: wte.success,
		error_message: wte.error_message,
		duration_ms: wte.duration_ms,
		iteration: wte.iteration,
		created_at: context.created_at
	};
}

/**
 * Formats tool execution duration for display
 *
 * @param durationMs - Duration in milliseconds
 * @returns Formatted duration string (e.g., "150ms", "1.5s")
 */
export function formatToolDuration(durationMs: number): string {
	if (durationMs < 1000) {
		return `${durationMs}ms`;
	}
	return `${(durationMs / 1000).toFixed(1)}s`;
}

/**
 * Gets display name for tool type
 *
 * @param toolType - Tool type
 * @returns Display name
 */
export function getToolTypeDisplay(toolType: ToolType): string {
	return toolType === 'local' ? 'Local' : 'MCP';
}

/**
 * Gets full tool identifier for display
 *
 * @param execution - Tool execution record
 * @returns Full identifier (e.g., "MemoryTool", "serena:find_symbol")
 */
export function getToolIdentifier(execution: ToolExecution | WorkflowToolExecution): string {
	if (execution.tool_type === 'mcp' && execution.server_name) {
		return `${execution.server_name}:${execution.tool_name}`;
	}
	return execution.tool_name;
}
