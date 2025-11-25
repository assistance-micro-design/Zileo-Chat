// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * MCP store for managing MCP server state in the frontend.
 * Provides reactive state management using pure functions.
 * @module stores/mcp
 */

import { invoke } from '@tauri-apps/api/core';
import type {
	MCPServer,
	MCPServerConfig,
	MCPServerStatus,
	MCPTestResult,
	MCPTool,
	MCPToolCallRequest,
	MCPToolCallResult
} from '$types/mcp';

/**
 * State interface for the MCP store
 */
export interface MCPState {
	/** List of all MCP servers */
	servers: MCPServer[];
	/** Loading state indicator */
	loading: boolean;
	/** Error message if any */
	error: string | null;
	/** Currently testing server ID */
	testingServerId: string | null;
}

/**
 * Creates the initial MCP state
 * @returns Initial MCP state with empty values
 */
export function createInitialMCPState(): MCPState {
	return {
		servers: [],
		loading: false,
		error: null,
		testingServerId: null
	};
}

/**
 * Sets the servers list
 * @param state - Current MCP state
 * @param servers - Array of MCP servers
 * @returns Updated state with new servers
 */
export function setServers(state: MCPState, servers: MCPServer[]): MCPState {
	return {
		...state,
		servers,
		loading: false,
		error: null
	};
}

/**
 * Adds a new MCP server to the state
 * @param state - Current MCP state
 * @param server - MCP server to add
 * @returns Updated state with new server
 */
export function addServer(state: MCPState, server: MCPServer): MCPState {
	const exists = state.servers.some((s) => s.id === server.id);
	if (exists) {
		return updateServer(state, server.id, server);
	}

	return {
		...state,
		servers: [...state.servers, server],
		error: null
	};
}

/**
 * Updates an existing MCP server in the state
 * @param state - Current MCP state
 * @param id - Server ID to update
 * @param updates - Partial server updates
 * @returns Updated state with modified server
 */
export function updateServer(
	state: MCPState,
	id: string,
	updates: Partial<MCPServer>
): MCPState {
	const servers = state.servers.map((s) =>
		s.id === id ? { ...s, ...updates } : s
	);

	return {
		...state,
		servers,
		error: null
	};
}

/**
 * Removes an MCP server from the state
 * @param state - Current MCP state
 * @param id - Server ID to remove
 * @returns Updated state without the server
 */
export function removeServer(state: MCPState, id: string): MCPState {
	return {
		...state,
		servers: state.servers.filter((s) => s.id !== id),
		error: null
	};
}

/**
 * Updates the status of a specific server
 * @param state - Current MCP state
 * @param id - Server ID
 * @param status - New status
 * @returns Updated state with new server status
 */
export function setServerStatus(
	state: MCPState,
	id: string,
	status: MCPServerStatus
): MCPState {
	return updateServer(state, id, { status });
}

/**
 * Updates the tools list for a specific server
 * @param state - Current MCP state
 * @param id - Server ID
 * @param tools - Tools discovered from server
 * @returns Updated state with server tools
 */
export function setServerTools(
	state: MCPState,
	id: string,
	tools: MCPTool[]
): MCPState {
	return updateServer(state, id, { tools });
}

/**
 * Sets the loading state
 * @param state - Current MCP state
 * @param loading - Loading state value
 * @returns Updated state with new loading value
 */
export function setMCPLoading(state: MCPState, loading: boolean): MCPState {
	return {
		...state,
		loading,
		error: loading ? null : state.error
	};
}

/**
 * Sets an error message
 * @param state - Current MCP state
 * @param error - Error message (or null to clear)
 * @returns Updated state with error
 */
export function setMCPError(state: MCPState, error: string | null): MCPState {
	return {
		...state,
		error,
		loading: false
	};
}

/**
 * Sets the testing server ID
 * @param state - Current MCP state
 * @param serverId - Server ID being tested (or null when done)
 * @returns Updated state with testing indicator
 */
export function setTestingServer(
	state: MCPState,
	serverId: string | null
): MCPState {
	return {
		...state,
		testingServerId: serverId
	};
}

// Selectors

/**
 * Gets a server by ID
 * @param state - Current MCP state
 * @param id - Server ID
 * @returns Server or undefined if not found
 */
export function getServerById(
	state: MCPState,
	id: string
): MCPServer | undefined {
	return state.servers.find((s) => s.id === id);
}

/**
 * Gets a server by name
 * @param state - Current MCP state
 * @param name - Server name
 * @returns Server or undefined if not found
 */
export function getServerByName(
	state: MCPState,
	name: string
): MCPServer | undefined {
	return state.servers.find((s) => s.name === name);
}

/**
 * Gets all servers with a specific status
 * @param state - Current MCP state
 * @param status - Status to filter by
 * @returns Array of servers with matching status
 */
export function getServersByStatus(
	state: MCPState,
	status: MCPServerStatus
): MCPServer[] {
	return state.servers.filter((s) => s.status === status);
}

/**
 * Gets all running servers
 * @param state - Current MCP state
 * @returns Array of running servers
 */
export function getRunningServers(state: MCPState): MCPServer[] {
	return getServersByStatus(state, 'running');
}

/**
 * Gets all enabled servers
 * @param state - Current MCP state
 * @returns Array of enabled servers
 */
export function getEnabledServers(state: MCPState): MCPServer[] {
	return state.servers.filter((s) => s.enabled);
}

/**
 * Gets total server count
 * @param state - Current MCP state
 * @returns Number of servers
 */
export function getServerCount(state: MCPState): number {
	return state.servers.length;
}

/**
 * Gets running server count
 * @param state - Current MCP state
 * @returns Number of running servers
 */
export function getRunningServerCount(state: MCPState): number {
	return getRunningServers(state).length;
}

/**
 * Gets all available tools from all running servers
 * @param state - Current MCP state
 * @returns Array of tools with server info
 */
export function getAllAvailableTools(
	state: MCPState
): Array<{ server: string; tool: MCPTool }> {
	const tools: Array<{ server: string; tool: MCPTool }> = [];

	for (const server of getRunningServers(state)) {
		for (const tool of server.tools) {
			tools.push({ server: server.name, tool });
		}
	}

	return tools;
}

/**
 * Checks if a server exists
 * @param state - Current MCP state
 * @param id - Server ID to check
 * @returns True if server exists
 */
export function hasServer(state: MCPState, id: string): boolean {
	return state.servers.some((s) => s.id === id);
}

/**
 * Checks if a server name is already taken
 * @param state - Current MCP state
 * @param name - Server name to check
 * @param excludeId - Server ID to exclude from check (for updates)
 * @returns True if name is taken
 */
export function isServerNameTaken(
	state: MCPState,
	name: string,
	excludeId?: string
): boolean {
	return state.servers.some((s) => s.name === name && s.id !== excludeId);
}

// Async actions (Tauri IPC calls)

/**
 * Loads all MCP servers from the backend
 * @returns Promise resolving to array of servers
 */
export async function loadServers(): Promise<MCPServer[]> {
	return invoke<MCPServer[]>('list_mcp_servers');
}

/**
 * Creates a new MCP server
 * @param config - Server configuration
 * @returns Promise resolving to created server
 */
export async function createServer(config: MCPServerConfig): Promise<MCPServer> {
	return invoke<MCPServer>('create_mcp_server', { config });
}

/**
 * Updates an existing MCP server
 * @param id - Server ID to update
 * @param config - New server configuration
 * @returns Promise resolving to updated server
 */
export async function updateServerConfig(
	id: string,
	config: MCPServerConfig
): Promise<MCPServer> {
	return invoke<MCPServer>('update_mcp_server', { id, config });
}

/**
 * Deletes an MCP server
 * @param id - Server ID to delete
 * @returns Promise resolving when complete
 */
export async function deleteServer(id: string): Promise<void> {
	return invoke<void>('delete_mcp_server', { id });
}

/**
 * Tests an MCP server connection
 * @param config - Server configuration to test
 * @returns Promise resolving to test result
 */
export async function testServer(config: MCPServerConfig): Promise<MCPTestResult> {
	return invoke<MCPTestResult>('test_mcp_server', { config });
}

/**
 * Starts an MCP server
 * @param id - Server ID to start
 * @returns Promise resolving to updated server
 */
export async function startServer(id: string): Promise<MCPServer> {
	return invoke<MCPServer>('start_mcp_server', { id });
}

/**
 * Stops an MCP server
 * @param id - Server ID to stop
 * @returns Promise resolving to updated server
 */
export async function stopServer(id: string): Promise<MCPServer> {
	return invoke<MCPServer>('stop_mcp_server', { id });
}

/**
 * Calls a tool on an MCP server
 * @param request - Tool call request
 * @returns Promise resolving to tool call result
 */
export async function callTool(request: MCPToolCallRequest): Promise<MCPToolCallResult> {
	return invoke<MCPToolCallResult>('call_mcp_tool', { request });
}

/**
 * Lists tools available from a specific server
 * @param serverName - Name of the server
 * @returns Promise resolving to array of tools
 */
export async function listServerTools(serverName: string): Promise<MCPTool[]> {
	return invoke<MCPTool[]>('list_mcp_tools', { serverName });
}
