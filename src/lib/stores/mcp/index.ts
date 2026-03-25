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
 * MCP store barrel export.
 * @module stores/mcp
 */

export type { MCPState } from './state';
export {
	createInitialMCPState,
	setServers,
	addServer,
	updateServer,
	removeServer,
	setServerStatus,
	setServerTools,
	setMCPLoading,
	setMCPError,
	setTestingServer,
	getServerById,
	getServerByName,
	getServersByStatus,
	getRunningServers,
	getEnabledServers,
	getServerCount,
	getRunningServerCount,
	getAllAvailableTools,
	hasServer,
	isServerNameTaken
} from './state';

export {
	invalidateMCPCache,
	loadServers,
	createServer,
	updateServerConfig,
	deleteServer,
	testServer,
	startServer,
	stopServer,
	callTool,
	listServerTools
} from './actions';
