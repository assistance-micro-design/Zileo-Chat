<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

<!--
AgentMcpAllowlist — arms per-(server, tool) MCP auto-approval for UNATTENDED
(detached) runs (R-SEC-4 / R1).

Used by AgentAuthorizations.svelte on the Settings → Validation page (per-agent
authorizations live there, NOT in AgentForm). i18n keys use the
`validation_mcp_allowlist_*` namespace.

Fully CONTROLLED: the arming state is derived from the `value` prop (the current
`mcp_tool_allowlist`), never held as internal seeded $state — so there is no
stale-seed reactivity bug (ERR_SVELTE_012). The parent seeds `value` from
`agent.mcp_tool_allowlist`; this component only emits the rebuilt allowlist on a
user toggle, so an open+save without interaction reproduces the existing
allowlist identically (anti-involuntary-disarm).

Per running server: one checkbox per tool (arming) + ONE checkbox
`allow_in_delegated_runs` (per-server flag: also auto-approve when another
autonomous agent delegates to this one). Entries of STOPPED / absent servers
cannot be edited (their tools are not listable) and are shown read-only —
they are preserved verbatim in the payload so they are never silently disarmed.
-->

<script lang="ts">
	import { i18n } from '$lib/i18n';
	import type { MCPServer } from '$types/mcp';
	import type { McpToolAllowlistEntry } from '$types/agent';
	import {
		buildMcpToolAllowlist,
		preservedMcpAllowlistEntries,
		seedMcpArmingState
	} from './mcp-allowlist.helpers';

	interface Props {
		/** All MCP servers currently running (their tools are listable/armable). */
		runningServers: MCPServer[];
		/** Current allowlist (complete array, incl. preserved stopped entries). */
		value: McpToolAllowlistEntry[];
		/** Emitted with the rebuilt complete allowlist on every user change. */
		onchange: (allowlist: McpToolAllowlistEntry[]) => void;
	}

	let { runningServers, value, onchange }: Props = $props();

	/** Immutable ids of the servers the user can edit (running = enumerable). */
	const enumerableIds = $derived(runningServers.map((s) => s.id));

	/** Existing entries for non-enumerable (stopped/absent) servers — read-only. */
	const preserved = $derived(preservedMcpAllowlistEntries(value, enumerableIds));

	/** Whether `tool` is armed for `serverId` in the current value. */
	function isToolArmed(serverId: string, tool: string): boolean {
		return value.find((e) => e.server_id === serverId)?.tools.includes(tool) ?? false;
	}

	/** Whether `serverId` is flagged to allow its armed tools in delegated runs. */
	function isDelegatedAllowed(serverId: string): boolean {
		return value.find((e) => e.server_id === serverId)?.allow_in_delegated_runs ?? false;
	}

	/** Rebuilds the arming map from the current value (enumerable servers only). */
	function currentArming() {
		return seedMcpArmingState(value, enumerableIds);
	}

	/** Toggles a single tool's armed state and emits the rebuilt allowlist. */
	function toggleTool(serverId: string, tool: string): void {
		const armed = currentArming();
		const cur = armed[serverId] ?? { tools: [], allowInDelegatedRuns: false };
		const tools = cur.tools.includes(tool)
			? cur.tools.filter((t) => t !== tool)
			: [...cur.tools, tool];
		armed[serverId] = { tools, allowInDelegatedRuns: cur.allowInDelegatedRuns };
		onchange(buildMcpToolAllowlist(armed, enumerableIds, value));
	}

	/** Flips the per-server delegation flag and emits the rebuilt allowlist. */
	function toggleDelegated(serverId: string): void {
		const armed = currentArming();
		const cur = armed[serverId] ?? { tools: [], allowInDelegatedRuns: false };
		armed[serverId] = { tools: cur.tools, allowInDelegatedRuns: !cur.allowInDelegatedRuns };
		onchange(buildMcpToolAllowlist(armed, enumerableIds, value));
	}
</script>

<div class="agent-mcp-allowlist">
	<p class="allowlist-warning" role="note">{$i18n('validation_mcp_allowlist_warning')}</p>

	{#if runningServers.length === 0}
		<p class="allowlist-empty">{$i18n('validation_mcp_allowlist_none')}</p>
	{:else}
		{#each runningServers as server (server.id)}
			<div class="server-block">
				<div class="server-header">
					<span class="server-name">{server.name}</span>
				</div>

				{#if server.tools.length === 0}
					<p class="server-no-tools">{$i18n('validation_mcp_allowlist_no_tools')}</p>
				{:else}
					<div class="tool-group">
						{#each server.tools as tool (tool.name)}
							<label class="tool-item">
								<input
									type="checkbox"
									checked={isToolArmed(server.id, tool.name)}
									onchange={() => toggleTool(server.id, tool.name)}
								/>
								<span class="tool-name">{tool.name}</span>
							</label>
						{/each}
					</div>

					<label class="delegated-item">
						<input
							type="checkbox"
							checked={isDelegatedAllowed(server.id)}
							onchange={() => toggleDelegated(server.id)}
						/>
						<div class="delegated-content">
							<span class="delegated-label"
								>{$i18n('validation_mcp_allowlist_allow_delegated')}</span
							>
							<span class="delegated-help"
								>{$i18n('validation_mcp_allowlist_allow_delegated_help')}</span
							>
						</div>
					</label>
				{/if}
			</div>
		{/each}
	{/if}

	{#if preserved.length > 0}
		<div class="preserved-block">
			<p class="preserved-note">{$i18n('validation_mcp_allowlist_stopped_note')}</p>
			<ul class="preserved-list" role="list">
				{#each preserved as entry (entry.server_id)}
					<li class="preserved-item">
						<span class="preserved-id" title={entry.server_id}>{entry.server_id}</span>
						<span class="preserved-tools">{entry.tools.join(', ')}</span>
						{#if entry.allow_in_delegated_runs}
							<span class="preserved-flag">{$i18n('validation_mcp_allowlist_delegated_badge')}</span
							>
						{/if}
					</li>
				{/each}
			</ul>
		</div>
	{/if}
</div>

<style>
	.agent-mcp-allowlist {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.allowlist-warning {
		margin: 0;
		padding: var(--spacing-sm);
		background: var(--color-bg-secondary);
		border-left: 3px solid var(--color-warning, var(--color-border));
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.allowlist-empty {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.server-block {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
	}

	.server-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.server-name {
		font-weight: 600;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.server-no-tools {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.tool-group {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding-left: var(--spacing-sm);
	}

	.tool-item,
	.delegated-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		cursor: pointer;
	}

	.tool-name {
		font-size: var(--font-size-sm);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}

	.delegated-item {
		margin-top: var(--spacing-xs);
		padding-top: var(--spacing-xs);
		border-top: 1px dashed var(--color-border);
	}

	.delegated-content {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.delegated-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.delegated-help {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.preserved-block {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.preserved-note {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.preserved-list {
		list-style: none;
		margin: 0;
		padding: 0;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.preserved-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-bg-tertiary, var(--color-bg-secondary));
		border: 1px dashed var(--color-border);
		border-radius: var(--border-radius-sm);
		opacity: 0.8;
	}

	.preserved-id {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
	}

	.preserved-tools {
		flex: 1;
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		min-width: 0;
	}

	.preserved-flag {
		font-size: var(--font-size-xs);
		color: var(--color-warning, var(--color-text-secondary));
	}
</style>
