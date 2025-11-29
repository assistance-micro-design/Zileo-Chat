// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * MCP (Model Context Protocol) type definitions for Zileo-Chat-3.
 *
 * These types are synchronized with Rust backend types in
 * `src-tauri/src/models/mcp.rs` to ensure type safety across IPC.
 *
 * @module types/mcp
 *
 * @example
 * ```typescript
 * import type { MCPServer, MCPServerConfig, MCPTool } from '$types/mcp';
 *
 * const config: MCPServerConfig = {
 *   id: 'serena-server',
 *   name: 'serena',
 *   enabled: true,
 *   command: 'docker',
 *   args: ['run', '-i', 'serena:latest'],
 *   env: { SERENA_PROJECT: '/path/to/project' }
 * };
 * ```
 */

/**
 * MCP deployment methods supported by the application.
 * - docker: Run server in a Docker container (recommended for production)
 * - npx: Run via NPX (Node.js package runner)
 * - uvx: Run via UVX (Python package runner)
 */
export type MCPDeploymentMethod = 'docker' | 'npx' | 'uvx';

/**
 * MCP server status values.
 * - stopped: Server is not running
 * - starting: Server is initializing
 * - running: Server is operational and ready for tool calls
 * - error: Server encountered an error
 * - disconnected: Server process exists but communication failed
 */
export type MCPServerStatus =
	| 'stopped'
	| 'starting'
	| 'running'
	| 'error'
	| 'disconnected';

/**
 * Configuration for an MCP server.
 * Used when creating or updating an MCP server configuration.
 */
export interface MCPServerConfig {
	/** Unique identifier for the server */
	id: string;
	/** Human-readable name for the server */
	name: string;
	/** Whether the server should be started automatically */
	enabled: boolean;
	/** Deployment method (docker, npx, uvx) */
	command: MCPDeploymentMethod;
	/** Command arguments for server startup */
	args: string[];
	/** Environment variables for the server process */
	env: Record<string, string>;
	/** Optional description of the server's purpose */
	description?: string;
}

/**
 * Full MCP server state including runtime information.
 * Extends MCPServerConfig with status and discovered capabilities.
 */
export interface MCPServer extends MCPServerConfig {
	/** Current server status */
	status: MCPServerStatus;
	/** Tools discovered from the server */
	tools: MCPTool[];
	/** Resources discovered from the server */
	resources: MCPResource[];
	/** ISO 8601 timestamp of server creation */
	created_at: string;
	/** ISO 8601 timestamp of last update */
	updated_at: string;
}

/**
 * MCP tool definition as discovered from a server.
 */
export interface MCPTool {
	/** Tool name (used in tool calls) */
	name: string;
	/** Human-readable description of the tool */
	description: string;
	/** JSON Schema describing the tool's input parameters */
	input_schema: Record<string, unknown>;
}

/**
 * MCP resource definition as discovered from a server.
 */
export interface MCPResource {
	/** Resource URI identifier */
	uri: string;
	/** Human-readable name for the resource */
	name: string;
	/** Optional description of the resource */
	description?: string;
	/** MIME type of the resource content */
	mime_type?: string;
}

/**
 * Result from testing an MCP server connection.
 */
export interface MCPTestResult {
	/** Whether the connection test succeeded */
	success: boolean;
	/** Human-readable status message */
	message: string;
	/** Tools discovered during the test */
	tools: MCPTool[];
	/** Resources discovered during the test */
	resources: MCPResource[];
	/** Time taken for the test in milliseconds */
	latency_ms: number;
}

/**
 * Request to call an MCP tool.
 */
export interface MCPToolCallRequest {
	/** Name of the server to call */
	server_name: string;
	/** Name of the tool to invoke */
	tool_name: string;
	/** Arguments to pass to the tool */
	arguments: Record<string, unknown>;
}

/**
 * Result from an MCP tool call.
 */
export interface MCPToolCallResult {
	/** Whether the tool call succeeded */
	success: boolean;
	/** Tool response content */
	content: unknown;
	/** Error message if the call failed */
	error?: string;
	/** Time taken for the call in milliseconds */
	duration_ms: number;
}

/**
 * Default values for MCP server configuration.
 */
export const MCP_DEFAULTS = {
	/** Default timeout for server operations (ms) */
	TIMEOUT_MS: 30000,
	/** Default maximum retries for failed operations */
	MAX_RETRIES: 3,
	/** Default deployment method */
	DEPLOYMENT_METHOD: 'docker' as MCPDeploymentMethod
} as const;
