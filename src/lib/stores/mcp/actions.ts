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
 * MCP async actions - Tauri IPC calls with caching.
 * @module stores/mcp/actions
 */

import { invoke } from '@tauri-apps/api/core';
import type {
	MCPServer,
	MCPServerConfig,
	MCPServerResponse,
	MCPTestResult,
	MCPTool,
	MCPToolCallRequest,
	MCPToolCallResult
} from '$types/mcp';

// ============================================================================
// Cache Management
// ============================================================================

interface MCPServerCache {
	servers: MCPServer[] | null;
	timestamp: number;
}

let mcpCache: MCPServerCache = { servers: null, timestamp: 0 };
const MCP_CACHE_TTL = 30000; // 30 seconds

/**
 * Invalidates the MCP servers cache.
 * Call this after any mutation (create/update/delete server).
 */
export function invalidateMCPCache(): void {
	mcpCache = { servers: null, timestamp: 0 };
}

// ============================================================================
// Async Actions (Tauri IPC calls)
// ============================================================================

/**
 * Loads all MCP servers from the backend.
 * Uses cache with 30s TTL to avoid duplicate API calls.
 * @param forceRefresh - Force reload ignoring cache
 * @returns Promise resolving to array of servers
 */
export async function loadServers(forceRefresh = false): Promise<MCPServer[]> {
	const now = Date.now();
	if (!forceRefresh && mcpCache.servers && (now - mcpCache.timestamp) < MCP_CACHE_TTL) {
		return mcpCache.servers;
	}

	const servers = await invoke<MCPServer[]>('list_mcp_servers');

	mcpCache = {
		servers,
		timestamp: now
	};

	return servers;
}

/**
 * Creates a new MCP server
 * @param config - Server configuration
 * @returns Promise resolving to server response with optional warning
 */
export async function createServer(config: MCPServerConfig): Promise<MCPServerResponse> {
	const response = await invoke<MCPServerResponse>('create_mcp_server', { config });
	invalidateMCPCache();
	return response;
}

/**
 * Updates an existing MCP server
 * @param id - Server ID to update
 * @param config - New server configuration
 * @returns Promise resolving to server response with optional warning
 */
export async function updateServerConfig(
	id: string,
	config: MCPServerConfig
): Promise<MCPServerResponse> {
	const response = await invoke<MCPServerResponse>('update_mcp_server', { id, config });
	invalidateMCPCache();
	return response;
}

/**
 * Deletes an MCP server
 * @param id - Server ID to delete
 * @returns Promise resolving when complete
 */
export async function deleteServer(id: string): Promise<void> {
	await invoke<void>('delete_mcp_server', { id });
	invalidateMCPCache();
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
