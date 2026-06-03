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
 * @fileoverview MCP network connectivity settings.
 *
 * Mirrors `src-tauri/src/mcp/network_settings.rs` (`McpNetworkSettings`,
 * `#[serde(rename_all = "camelCase")]`). Update both sides together.
 */

/**
 * Persisted MCP network connectivity settings stored in
 * `settings:mcp_network.config`.
 */
export interface McpNetworkSettings {
	/**
	 * Opt-in to reach MCP HTTP servers on private/LAN ranges (RFC1918, CGNAT
	 * 100.64/10, ULA fc00::/7). Disabled by default (secure-by-default).
	 */
	allowPrivateNetwork: boolean;
}

/** Partial update payload — only provided fields are applied. */
export interface UpdateMcpNetworkSettingsRequest {
	allowPrivateNetwork?: boolean;
}
