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
(detached) runs.

Used by AgentAuthorizations.svelte on the Settings → Validation page (per-agent
authorizations live there, NOT in AgentForm). i18n keys use the
`validation_mcp_allowlist_*` namespace.

Fully CONTROLLED: the arming state is derived from the `value` prop (the current
`mcp_tool_allowlist`), never held as internal seeded $state — so there is no
stale-seed reactivity bug. The parent seeds `value` from
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
	import { ChevronRight, Trash2 } from '@lucide/svelte';
	import type { MCPServer } from '$types/mcp';
	import type { McpToolAllowlistEntry } from '$types/agent';
	import {
		areAllToolsArmed,
		buildMcpToolAllowlist,
		mergeServerTools,
		preservedMcpAllowlistEntries,
		removePreservedEntry,
		seedMcpArmingState,
		toggleAllServerTools
	} from './mcp-allowlist.helpers';

	interface Props {
		/** Editable servers: RUNNING and assigned to the agent (tools armable). */
		runningServers: MCPServer[];
		/**
		 * ALL known MCP servers (running + stopped), used ONLY to resolve a
		 * preserved entry's display name and status (de-selected vs stopped vs
		 * not-found). Never used to decide what is editable.
		 */
		knownServers: MCPServer[];
		/** Current allowlist (complete array, incl. preserved stopped entries). */
		value: McpToolAllowlistEntry[];
		/** Emitted with the rebuilt complete allowlist on every user change. */
		onchange: (allowlist: McpToolAllowlistEntry[]) => void;
	}

	let { runningServers, knownServers, value, onchange }: Props = $props();

	/**
	 * EPHEMERAL UI-only expansion state, keyed by immutable server id (default
	 * collapsed). It is NEVER part of `value` and NEVER triggers `onchange`:
	 * expanding/collapsing changes nothing in the allowlist (the component stays
	 * fully controlled — it derives from `value`).
	 */
	let expanded = $state<Record<string, boolean>>({});

	/** Immutable ids of the servers the user can edit (running = enumerable). */
	const enumerableIds = $derived(runningServers.map((s) => s.id));

	/** Existing entries for non-enumerable (stopped/absent) servers — read-only. */
	const preserved = $derived(preservedMcpAllowlistEntries(value, enumerableIds));

	/** Whether the server's tool list is expanded (default false = collapsed). */
	function isExpanded(serverId: string): boolean {
		return expanded[serverId] ?? false;
	}

	/** Toggles the server's expansion — pure UI state, never touches `value`. */
	function toggleExpanded(serverId: string): void {
		expanded[serverId] = !isExpanded(serverId);
	}

	/** Tool names currently armed for `serverId` (incl. tools no longer exposed). */
	function armedToolsFor(serverId: string): string[] {
		return value.find((e) => e.server_id === serverId)?.tools ?? [];
	}

	/** Total armed tools for a server (exposed + orphans) — the "N" in "N/M". */
	function countArmed(serverId: string): number {
		return armedToolsFor(serverId).length;
	}

	/**
	 * Tool rows for an editable server: its currently-exposed tools, plus any
	 * armed tools it no longer exposes (orphans, marked). Nothing armed stays
	 * hidden — every armed tool is visible and toggleable.
	 */
	function displayedTools(server: MCPServer) {
		return mergeServerTools(
			server.tools.map((t) => t.name),
			armedToolsFor(server.id)
		);
	}

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

	/** Whether every displayed tool of `server` is currently auto-approved. */
	function isAllArmed(server: MCPServer): boolean {
		return areAllToolsArmed(
			displayedTools(server).map((t) => t.name),
			armedToolsFor(server.id)
		);
	}

	/**
	 * Select-all / none for one server: arms every displayed tool when not all
	 * are armed yet, otherwise clears the selection. Preserves armed orphans.
	 */
	function toggleAllTools(server: MCPServer): void {
		const armed = currentArming();
		const cur = armed[server.id] ?? { tools: [], allowInDelegatedRuns: false };
		const tools = toggleAllServerTools(
			displayedTools(server).map((t) => t.name),
			armedToolsFor(server.id)
		);
		armed[server.id] = { tools, allowInDelegatedRuns: cur.allowInDelegatedRuns };
		onchange(buildMcpToolAllowlist(armed, enumerableIds, value));
	}

	/** Flips the per-server delegation flag and emits the rebuilt allowlist. */
	function toggleDelegated(serverId: string): void {
		const armed = currentArming();
		const cur = armed[serverId] ?? { tools: [], allowInDelegatedRuns: false };
		armed[serverId] = { tools: cur.tools, allowInDelegatedRuns: !cur.allowInDelegatedRuns };
		onchange(buildMcpToolAllowlist(armed, enumerableIds, value));
	}

	/**
	 * EXPLICIT revocation of a preserved (read-only) server's entry — the only UI
	 * path to disarm a de-selected / stopped / removed server. Emits the allowlist
	 * with ONLY that entry removed; every other entry stays intact.
	 */
	function removeServer(serverId: string): void {
		onchange(removePreservedEntry(value, serverId));
	}

	/** Display name for a (possibly stopped/removed) server id; falls back to id. */
	function resolveServerName(serverId: string): string {
		return knownServers.find((s) => s.id === serverId)?.name ?? serverId;
	}

	/** Whether the server id can be resolved to a known server (for the name). */
	function isResolvable(serverId: string): boolean {
		return knownServers.some((s) => s.id === serverId);
	}

	/** i18n key describing WHY a preserved entry is read-only here. */
	function preservedKindLabel(serverId: string): string {
		const known = knownServers.find((s) => s.id === serverId);
		if (!known) return 'validation_mcp_allowlist_preserved_missing';
		return known.status === 'running'
			? 'validation_mcp_allowlist_preserved_deselected'
			: 'validation_mcp_allowlist_preserved_stopped';
	}
</script>

<div class="agent-mcp-allowlist">
	<p class="allowlist-warning" role="note">{$i18n('validation_mcp_allowlist_warning')}</p>

	{#if runningServers.length === 0}
		<p class="allowlist-empty">{$i18n('validation_mcp_allowlist_none')}</p>
	{:else}
		{#each runningServers as server (server.id)}
			{@const displayed = displayedTools(server)}
			{@const armed = countArmed(server.id)}
			<div class="server-block">
				<button
					type="button"
					class="server-summary"
					aria-expanded={isExpanded(server.id)}
					aria-controls={isExpanded(server.id) ? `mcp-srv-body-${server.id}` : undefined}
					onclick={() => toggleExpanded(server.id)}
				>
					<span class={['chevron', isExpanded(server.id) && 'chevron-open']} aria-hidden="true">
						<ChevronRight size={16} />
					</span>
					<span class="server-name">{server.name}</span>
					<span class="armed-count">
						{$i18n('validation_mcp_allowlist_armed_count', { armed, total: displayed.length })}
					</span>
				</button>

				{#if displayed.length === 0}
					{#if isExpanded(server.id)}
						<p id="mcp-srv-body-{server.id}" class="server-no-tools">
							{$i18n('validation_mcp_allowlist_no_tools')}
						</p>
					{/if}
				{:else}
					<!-- Delegation toggle stays visible without expanding (it governs the
					     whole server). Disabled when nothing is armed: it only takes effect
					     once at least one tool is auto-approved (otherwise the entry is pruned). -->
					<label class="delegated-item">
						<input
							type="checkbox"
							checked={isDelegatedAllowed(server.id)}
							disabled={armed === 0}
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

					{#if isExpanded(server.id)}
						<div id="mcp-srv-body-{server.id}" class="tool-group">
							<label class="tool-item tool-select-all">
								<input
									type="checkbox"
									checked={isAllArmed(server)}
									onchange={() => toggleAllTools(server)}
								/>
								<span class="select-all-label">{$i18n('validation_mcp_allowlist_select_all')}</span>
							</label>
							{#each displayed as tool (tool.name)}
								<label class={['tool-item', tool.orphan && 'tool-orphan']}>
									<input
										type="checkbox"
										checked={isToolArmed(server.id, tool.name)}
										onchange={() => toggleTool(server.id, tool.name)}
									/>
									<span class="tool-name">{tool.name}</span>
									{#if tool.orphan}
										<span class="orphan-badge"
											>{$i18n('validation_mcp_allowlist_orphan_badge')}</span
										>
									{/if}
								</label>
							{/each}
						</div>
					{/if}
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
						<div class="preserved-info">
							<span class="preserved-name">{resolveServerName(entry.server_id)}</span>
							<span class="preserved-kind">{$i18n(preservedKindLabel(entry.server_id))}</span>
							{#if !isResolvable(entry.server_id)}
								<span class="preserved-raw-id" title={entry.server_id}>({entry.server_id})</span>
							{/if}
						</div>
						<span class="preserved-tools">{entry.tools.join(', ')}</span>
						{#if entry.allow_in_delegated_runs}
							<span class="preserved-flag">{$i18n('validation_mcp_allowlist_delegated_badge')}</span
							>
						{/if}
						<button
							type="button"
							class="preserved-remove"
							aria-label={$i18n('validation_mcp_allowlist_remove', {
								name: resolveServerName(entry.server_id)
							})}
							onclick={() => removeServer(entry.server_id)}
						>
							<Trash2 size={14} aria-hidden="true" />
						</button>
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

	.server-summary {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		width: 100%;
		padding: 0;
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
		color: inherit;
		font: inherit;
	}

	.chevron {
		display: inline-flex;
		flex-shrink: 0;
		color: var(--color-text-secondary);
		transition: transform 0.15s ease;
	}

	.chevron-open {
		transform: rotate(90deg);
	}

	.server-name {
		font-weight: 600;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.armed-count {
		margin-left: auto;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
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

	.tool-select-all {
		padding-bottom: var(--spacing-xs);
		margin-bottom: 2px;
		border-bottom: 1px dashed var(--color-border);
	}

	.select-all-label {
		font-size: var(--font-size-sm);
		font-weight: 600;
		color: var(--color-text-secondary);
	}

	.tool-orphan .tool-name {
		color: var(--color-text-secondary);
	}

	.orphan-badge {
		font-size: var(--font-size-xs);
		font-style: italic;
		color: var(--color-warning, var(--color-text-secondary));
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

	.preserved-info {
		display: flex;
		flex-direction: column;
		gap: 2px;
		min-width: 0;
	}

	.preserved-name {
		font-size: var(--font-size-sm);
		font-weight: 600;
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preserved-kind {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.preserved-raw-id {
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

	.preserved-remove {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
		padding: var(--spacing-xs);
		background: none;
		border: none;
		border-radius: var(--border-radius-sm);
		color: var(--color-danger);
		cursor: pointer;
	}

	.preserved-remove:hover {
		background: var(--color-danger-light);
	}
</style>
