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
	LegacyHttpAuthWarning,
	MCPServer,
	MCPServerConfig,
	MCPServerConfigWithSecret,
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
 * Creates a new MCP server.
 *
 * For HTTP servers using authentication, pass `authType`, `authMetadata` and
 * `authSecret` on the config. The secret is moved to the OS keychain on the
 * backend; it is never persisted to the database.
 *
 * @param config - Server configuration (with optional auth secret)
 * @returns Promise resolving to server response with optional warning
 */
export async function createServer(
	config: MCPServerConfigWithSecret
): Promise<MCPServerResponse> {
	const response = await invoke<MCPServerResponse>('create_mcp_server', { config });
	invalidateMCPCache();
	return response;
}

/**
 * Updates an existing MCP server.
 *
 * Secret rotation rules (handled backend-side):
 * - `authType === 'none'`: any stored secret is removed.
 * - `authSecret` provided: the keychain entry is overwritten.
 * - `authSecret` omitted with active auth: the existing secret is kept.
 *
 * @param id - Server ID to update
 * @param config - New server configuration (with optional auth secret)
 * @returns Promise resolving to server response with optional warning
 */
export async function updateServerConfig(
	id: string,
	config: MCPServerConfigWithSecret
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

/**
 * Lists HTTP MCP servers that still rely on the legacy `API_KEY` / `HEADER_*`
 * environment variables. These are no longer interpreted at runtime since the
 * v0.21 HTTP auth refactor, so the user must migrate them to the new
 * authentication fields to restore the connection.
 *
 * @returns Promise resolving to one warning entry per affected server
 */
export async function listLegacyHttpAuth(): Promise<LegacyHttpAuthWarning[]> {
	return invoke<LegacyHttpAuthWarning[]>('list_mcp_legacy_http_auth');
}
