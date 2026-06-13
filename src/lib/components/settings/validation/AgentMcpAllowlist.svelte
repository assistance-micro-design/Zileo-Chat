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

Each running server renders as a nested card: collapsible header carrying the
armed-count badge plus the arm-all / disarm-all actions, expanded body with the
per-tool checkbox grid and the per-server `allow_in_delegated_runs` switch
(also auto-approve when another autonomous agent delegates to this one).
Entries of STOPPED / absent servers cannot be edited (their tools are not
listable) and are shown as read-only dimmed cards — they are preserved verbatim
in the payload so they are never silently disarmed.
-->

<script lang="ts">
	import { i18n } from '$lib/i18n';
	import { Badge, Button, Switch } from '$lib/components/ui';
	import { ChevronRight, Trash2 } from '@lucide/svelte';
	import type { MCPServer } from '$types/mcp';
	import type { McpToolAllowlistEntry } from '$types/agent';
	import {
		areAllToolsArmed,
		buildMcpToolAllowlist,
		mergeServerTools,
		preservedMcpAllowlistEntries,
		removePreservedEntry,
		seedMcpArmingState
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

	/** Replaces one server's armed tool list and emits the rebuilt allowlist. */
	function setServerTools(serverId: string, tools: string[]): void {
		const armed = currentArming();
		const cur = armed[serverId] ?? { tools: [], allowInDelegatedRuns: false };
		armed[serverId] = { tools, allowInDelegatedRuns: cur.allowInDelegatedRuns };
		onchange(buildMcpToolAllowlist(armed, enumerableIds, value));
	}

	/** Arms every displayed tool of `server` (exposed + armed orphans). */
	function armAllTools(server: MCPServer): void {
		setServerTools(
			server.id,
			displayedTools(server).map((t) => t.name)
		);
	}

	/** Disarms every tool of `server` (the entry is pruned from the payload). */
	function disarmAllTools(server: MCPServer): void {
		setServerTools(server.id, []);
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
	{#if runningServers.length === 0}
		<p class="allowlist-empty">{$i18n('validation_mcp_allowlist_none')}</p>
	{:else}
		{#each runningServers as server (server.id)}
			<div class="card server-card">
				<div class="server-header">
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
						<strong class="server-name">{server.name}</strong>
						<Badge variant="mcp">
							{$i18n('validation_mcp_allowlist_armed_count', {
								armed: countArmed(server.id),
								total: displayedTools(server).length
							})}
						</Badge>
					</button>
					<div class="server-actions">
						<Button
							variant="ghost"
							size="sm"
							disabled={displayedTools(server).length === 0 || isAllArmed(server)}
							onclick={() => armAllTools(server)}
						>
							{$i18n('validation_mcp_allowlist_select_all')}
						</Button>
						<Button
							variant="ghost"
							size="sm"
							disabled={countArmed(server.id) === 0}
							onclick={() => disarmAllTools(server)}
						>
							{$i18n('validation_mcp_allowlist_remove', { name: server.name })}
						</Button>
					</div>
				</div>

				{#if isExpanded(server.id)}
					<div id="mcp-srv-body-{server.id}" class="server-body">
						{#if displayedTools(server).length === 0}
							<p class="server-no-tools">{$i18n('validation_mcp_allowlist_no_tools')}</p>
						{:else}
							<div class="tool-grid">
								{#each displayedTools(server) as tool (tool.name)}
									<label class={['tool-item', tool.orphan && 'tool-orphan']}>
										<input
											type="checkbox"
											class="form-checkbox"
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
							<!-- Delegation switch: disabled when nothing is armed — it only
							     takes effect once at least one tool is auto-approved
							     (otherwise the entry is pruned from the payload). -->
							<div class="toggle-row delegated-row">
								<span class="toggle-text">
									<strong id="mcp-delegated-{server.id}">
										{$i18n('validation_mcp_allowlist_allow_delegated')}
									</strong>
									<span>{$i18n('validation_mcp_allowlist_allow_delegated_help')}</span>
								</span>
								<Switch
									checked={isDelegatedAllowed(server.id)}
									disabled={countArmed(server.id) === 0}
									onchange={() => toggleDelegated(server.id)}
									labelledBy="mcp-delegated-{server.id}"
								/>
							</div>
						{/if}
					</div>
				{/if}
			</div>
		{/each}
	{/if}

	{#if preserved.length > 0}
		<p class="preserved-note">{$i18n('validation_mcp_allowlist_stopped_note')}</p>
		{#each preserved as entry (entry.server_id)}
			<div class="card server-card preserved-card">
				<div class="server-header">
					<div class="preserved-title">
						<strong class="server-name">{resolveServerName(entry.server_id)}</strong>
						{#if !isResolvable(entry.server_id)}
							<span class="preserved-raw-id" title={entry.server_id}>({entry.server_id})</span>
						{/if}
						<Badge variant="neutral">{entry.tools.length}</Badge>
						<Badge variant="warning">{$i18n(preservedKindLabel(entry.server_id))}</Badge>
					</div>
					<Button
						variant="ghost"
						size="sm"
						ariaLabel={$i18n('validation_mcp_allowlist_remove', {
							name: resolveServerName(entry.server_id)
						})}
						onclick={() => removeServer(entry.server_id)}
					>
						<Trash2 size={14} aria-hidden="true" />
					</Button>
				</div>
				<div class="server-body preserved-body">
					<span class="preserved-tools">{entry.tools.join(', ')}</span>
					{#if entry.allow_in_delegated_runs}
						<span class="preserved-flag">{$i18n('validation_mcp_allowlist_delegated_badge')}</span>
					{/if}
				</div>
			</div>
		{/each}
	{/if}
</div>

<style>
	.agent-mcp-allowlist {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.allowlist-empty {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	/* Nested cards: keep the global card surface but drop the outer shadow. */
	.server-card {
		box-shadow: none;
	}

	.server-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-md);
		flex-wrap: wrap;
		padding: var(--spacing-md);
	}

	.server-card:not(.preserved-card) .server-header:not(:last-child) {
		border-bottom: 1px solid var(--color-border-light);
	}

	.server-summary {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
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

	@media (prefers-reduced-motion: reduce) {
		.chevron {
			transition: none;
		}
	}

	.chevron-open {
		transform: rotate(90deg);
	}

	.server-name {
		font-weight: var(--font-weight-semibold);
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.server-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		flex-wrap: wrap;
	}

	.server-body {
		padding: var(--spacing-md);
	}

	.server-no-tools {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.tool-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
		gap: var(--spacing-sm);
	}

	.tool-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		cursor: pointer;
		min-width: 0;
	}

	.tool-name {
		font-size: var(--font-size-sm);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.tool-orphan .tool-name {
		color: var(--color-text-secondary);
	}

	.orphan-badge {
		font-size: var(--font-size-xs);
		font-style: italic;
		color: var(--color-warning, var(--color-text-secondary));
		white-space: nowrap;
	}

	/* Delegation switch row under the tool grid */
	.delegated-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
		margin-top: var(--spacing-sm);
		padding-top: var(--spacing-sm);
		border-top: 1px solid var(--color-border-light);
	}

	.toggle-text strong {
		display: block;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.toggle-text span {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: 2px;
		max-width: 56ch;
	}

	/* Preserved (read-only) entries: dimmed cards, explicit revoke only */
	.preserved-note {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.preserved-card {
		opacity: 0.8;
	}

	.preserved-title {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		min-width: 0;
	}

	.preserved-raw-id {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-secondary);
	}

	.preserved-body {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		padding-top: 0;
	}

	.preserved-tools {
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
		white-space: nowrap;
	}
</style>
