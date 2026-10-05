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
 * @fileoverview Typed Tauri API client for the MCP domain.
 *
 * Covers MCP server CRUD, lifecycle (test/start/stop), tool listing and
 * tool calls, plus latency metrics and legacy auth warnings.
 *
 * @module lib/api/mcp
 */

import { invokeCommand } from './internal';
import type {
	LegacyHttpAuthWarning,
	MCPLatencyMetrics,
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
// Server CRUD
// ============================================================================

/** List all configured MCP servers. */
export async function listMcpServers(): Promise<MCPServer[]> {
	return invokeCommand<MCPServer[]>('list_mcp_servers');
}

/** Get a single MCP server. */
export async function getMcpServer(id: string): Promise<MCPServer> {
	return invokeCommand<MCPServer>('get_mcp_server', { id });
}

/** Create an MCP server. */
export async function createMcpServer(
	config: MCPServerConfigWithSecret
): Promise<MCPServerResponse> {
	return invokeCommand<MCPServerResponse>('create_mcp_server', { config });
}

/** Update an MCP server. */
export async function updateMcpServer(
	id: string,
	config: MCPServerConfigWithSecret
): Promise<MCPServerResponse> {
	return invokeCommand<MCPServerResponse>('update_mcp_server', { id, config });
}

/** Delete an MCP server. */
export async function deleteMcpServer(id: string): Promise<void> {
	return invokeCommand<void>('delete_mcp_server', { id });
}

// ============================================================================
// Lifecycle
// ============================================================================

/** Test an MCP server configuration. */
export async function testMcpServer(config: MCPServerConfig): Promise<MCPTestResult> {
	return invokeCommand<MCPTestResult>('test_mcp_server', { config });
}

/** Start an MCP server. */
export async function startMcpServer(id: string): Promise<MCPServer> {
	return invokeCommand<MCPServer>('start_mcp_server', { id });
}

/** Stop a running MCP server. */
export async function stopMcpServer(id: string): Promise<MCPServer> {
	return invokeCommand<MCPServer>('stop_mcp_server', { id });
}

/** List legacy HTTP auth warnings. */
export async function listMcpLegacyHttpAuth(): Promise<LegacyHttpAuthWarning[]> {
	return invokeCommand<LegacyHttpAuthWarning[]>('list_mcp_legacy_http_auth');
}

// ============================================================================
// Tools
// ============================================================================

/** List available tools of a server. */
export async function listMcpTools(serverName: string): Promise<MCPTool[]> {
	return invokeCommand<MCPTool[]>('list_mcp_tools', { serverName });
}

/** Execute a tool on an MCP server. */
export async function callMcpTool(request: MCPToolCallRequest): Promise<MCPToolCallResult> {
	return invokeCommand<MCPToolCallResult>('call_mcp_tool', { request });
}

/** Get latency metrics, optionally for a single server. */
export async function getMcpLatencyMetrics(
	serverName?: string | null
): Promise<MCPLatencyMetrics[]> {
	return invokeCommand<MCPLatencyMetrics[]>('get_mcp_latency_metrics', {
		serverName: serverName
	});
}
