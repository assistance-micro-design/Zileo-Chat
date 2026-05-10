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
 * @fileoverview Tool execution types for persistence and display.
 *
 * These types are synchronized with Rust backend types:
 * - src-tauri/src/models/tool_execution.rs (ToolExecution, ToolExecutionCreate)
 * - src-tauri/src/models/workflow.rs (WorkflowToolExecution)
 *
 * Tool Execution Persistence
 *
 * @module types/tool
 */

/**
 * Tool type indicating execution context
 */
type ToolType = 'local' | 'mcp';

/**
 * Common input parameters for tool execution.
 *
 * Tools may have additional specific parameters beyond these common fields.
 * Maps to serde_json::Value in Rust for flexibility.
 */
interface ToolInputParams {
	/** Operation type (e.g., "add", "get", "update", "delete", "list", "search") */
	operation?: string;
	/** Content or data to process */
	content?: string;
	/** Entity ID for get/update/delete operations */
	id?: string;
	/** Name or identifier for create operations */
	name?: string;
	/** Allow additional tool-specific fields */
	[key: string]: unknown;
}

/**
 * Common output result structure for tool execution.
 *
 * Tools may return additional specific fields beyond these common ones.
 * Maps to serde_json::Value in Rust for flexibility.
 */
interface ToolOutputResult {
	/** Whether the operation was successful */
	success?: boolean;
	/** Created or retrieved entity ID */
	id?: string;
	/** Whether an entity was found (for search operations) */
	found?: boolean;
	/** Line number (for code-related tools) */
	line?: number;
	/** Error message if operation failed */
	error?: string;
	/** Allow additional tool-specific fields */
	[key: string]: unknown;
}

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
	input_params: ToolInputParams;
	/** Output result as JSON (null when tool execution failed) */
	output_result: ToolOutputResult | null;
	/** Whether execution was successful */
	success: boolean;
	/** Error message if failed */
	error_message?: string;
	/** Duration in milliseconds */
	duration_ms: number;
	/** Iteration number in tool loop (0-indexed) */
	iteration: number;
	/** Global ordering sequence within execution (for interleaving with thinking steps) */
	sequence: number;
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
	input_params: ToolInputParams;
	/** Output result as JSON (null when tool execution failed) */
	output_result: ToolOutputResult | null;
	/** Whether execution was successful */
	success: boolean;
	/** Error message if failed */
	error_message?: string;
	/** Duration in milliseconds */
	duration_ms: number;
	/** Iteration number in tool loop */
	iteration: number;
}
