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
AgentAuthorizations — per-agent unattended-run authorizations, edited on the
Settings → Validation page (NOT in AgentForm). For the selected agent it edits:
  - require_file_confirmation (destructive file-op confirmation), and
  - the MCP auto-approval allowlist for detached runs.

Seeds imperatively from get_agent_config on selection (AgentSummary does not
carry these fields) — no reactive $effect that could clobber edits
Save is EXPLICIT (disarming is sensitive): a single
update_agent carries the rebuilt allowlist + the confirmation flag; omitted
fields keep their existing value (tri-state PATCH). Entries of stopped/absent
MCP servers are preserved verbatim by the allowlist helpers.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { Button, Select, type SelectOption } from '$lib/components/ui';
	import { agents, agentStore } from '$lib/stores/agents';
	import { loadServers } from '$lib/stores/mcp';
	import {
		validationSettingsStore,
		settings as validationSettings
	} from '$lib/stores/validation-settings';
	import { getErrorMessage } from '$lib/utils/error';
	import type { AgentConfig, McpToolAllowlistEntry } from '$types/agent';
	import type { MCPServer } from '$types/mcp';
	import AgentMcpAllowlist from './AgentMcpAllowlist.svelte';
	import { filterEditableMcpServers } from './mcp-allowlist.helpers';

	let selectedAgentId = $state('');
	let loadedConfig = $state<AgentConfig | null>(null);
	let runningServers = $state.raw<MCPServer[]>([]);
	/** ALL known servers (running + stopped) — only to resolve preserved names. */
	let allKnownServers = $state.raw<MCPServer[]>([]);
	/** Buffered allowlist being edited (seeded from the loaded config). */
	let allowlistDraft = $state.raw<McpToolAllowlistEntry[]>([]);
	/** Buffered confirmation flag being edited. */
	let requireFileConfirmation = $state(true);
	let dirty = $state(false);
	let loading = $state(false);
	let saving = $state(false);
	let errorMsg = $state<string | null>(null);
	let saved = $state(false);

	// Sequence token guarding against a seed race: if the user switches agent
	// while a load is in flight and the older load resolves last, its buffers
	// would otherwise be written under the newer agent's label (and a Save would
	// cross-write A's authorizations onto B). Non-reactive on purpose.
	let seq = 0;

	// Agent options, with a textual marker on agents that have >=1 MCP tool
	// auto-approved for unattended runs — native <option> can only carry text, so
	// the marker is folded into the label. (file-operation confirmation is NOT
	// part of this: it is near-universally off and would mark every agent.)
	const agentOptions = $derived<SelectOption[]>(
		$agents.map((a) => ({
			value: a.id,
			label: a.has_mcp_auto_approval
				? $i18n('validation_authorizations_configured_marker', { name: a.name })
				: a.name
		}))
	);

	/** True when any agent has MCP auto-approval configured (shows the legend). */
	const anyConfigured = $derived($agents.some((a) => a.has_mcp_auto_approval));

	/**
	 * Localized label of the current GLOBAL validation mode — shown for context
	 * only. The allowlist is NEVER coupled to it (detached vs attended runs are
	 * governed independently).
	 */
	const currentModeLabel = $derived(
		$validationSettings ? $i18n(`validation_mode_${$validationSettings.mode}`) : null
	);

	onMount(() => {
		agentStore.loadAgents();
		// Load the global validation mode for the read-only context block (a
		// sibling section may already have loaded it; the store is idempotent).
		void validationSettingsStore.loadSettings().catch(() => {});
	});

	/**
	 * Seeds the editable buffers from the selected agent's full config. Runs only
	 * on an explicit user selection (not in a reactive $effect), so user edits are
	 * never clobbered by a re-run.
	 */
	async function selectAgent(id: string): Promise<void> {
		const reqId = ++seq;
		selectedAgentId = id;
		loadedConfig = null;
		errorMsg = null;
		saved = false;
		dirty = false;
		if (!id) return;
		loading = true;
		try {
			const [config, servers] = await Promise.all([
				agentStore.getAgentConfig(id),
				loadServers(true)
			]);
			// Bail if a newer selection superseded this one mid-await: only the
			// latest selection may write the buffers (anti seed-race / cross-write).
			if (reqId !== seq) return;
			loadedConfig = config;
			allKnownServers = servers;
			// Editable servers = RUNNING and ASSIGNED to this agent (by name).
			// De-selected or stopped servers are not editable here; their armed
			// tools fall through to preserved read-only (never disarmed — Piège 2).
			runningServers = filterEditableMcpServers(servers, config.mcp_servers);
			allowlistDraft = [...(config.mcp_tool_allowlist ?? [])];
			requireFileConfirmation = config.require_file_confirmation ?? true;
		} catch (e) {
			if (reqId !== seq) return;
			errorMsg = getErrorMessage(e);
		} finally {
			if (reqId === seq) loading = false;
		}
	}

	function onAllowlistChange(next: McpToolAllowlistEntry[]): void {
		allowlistDraft = next;
		dirty = true;
		saved = false;
	}

	function toggleRequireFileConfirmation(): void {
		requireFileConfirmation = !requireFileConfirmation;
		dirty = true;
		saved = false;
	}

	/**
	 * Persists the buffered authorizations with a SINGLE update_agent call, then
	 * re-fetches to reflect the canonical persisted state (preserved stopped
	 * entries, server-side normalisation). Omitting other fields is the tri-state
	 * "keep existing" contract.
	 */
	async function save(): Promise<void> {
		// Capture the target so the write + refetch can never drift onto another
		// agent if the selection changes mid-await (defense in depth; the Select
		// is also disabled while saving).
		const id = selectedAgentId;
		if (!id || !loadedConfig) return;
		saving = true;
		errorMsg = null;
		saved = false;
		try {
			await agentStore.updateAgent(id, {
				mcp_tool_allowlist: allowlistDraft,
				require_file_confirmation: requireFileConfirmation
			});
			const config = await agentStore.getAgentConfig(id);
			// Drop the refetch if the user moved on to another agent meanwhile.
			if (selectedAgentId !== id) return;
			loadedConfig = config;
			allowlistDraft = [...(config.mcp_tool_allowlist ?? [])];
			requireFileConfirmation = config.require_file_confirmation ?? true;
			dirty = false;
			saved = true;
			// Refresh the summary list so the "MCP auto-approval" marker in the
			// agent dropdown reflects the change just persisted (the list is
			// otherwise only loaded on mount).
			void agentStore.loadAgents();
		} catch (e) {
			errorMsg = getErrorMessage(e);
		} finally {
			saving = false;
		}
	}
</script>

<div class="agent-authorizations">
	<Select
		label={$i18n('validation_authorizations_select_agent')}
		placeholder={$i18n('validation_authorizations_select_placeholder')}
		value={selectedAgentId}
		options={agentOptions}
		disabled={loading || saving}
		onchange={(e) => selectAgent(e.currentTarget.value)}
	/>

	{#if anyConfigured}
		<p class="auth-legend" role="note">
			{$i18n('validation_authorizations_configured_legend')}
		</p>
	{/if}

	{#if errorMsg}
		<p class="auth-error" role="alert">{errorMsg}</p>
	{/if}

	{#if loading}
		<p class="auth-info">{$i18n('validation_authorizations_loading')}</p>
	{:else if loadedConfig}
		<div class="mode-context" role="note">
			<p class="mode-context-line">{$i18n('validation_authorizations_detached_scope')}</p>
			<p class="mode-context-line">
				{$i18n('validation_authorizations_attended_scope', { mode: currentModeLabel ?? '—' })}
			</p>
		</div>

		<label class="checkbox-item">
			<input
				type="checkbox"
				checked={requireFileConfirmation}
				onchange={toggleRequireFileConfirmation}
			/>
			<div class="checkbox-content">
				<span class="checkbox-label">{$i18n('validation_require_file_confirmation')}</span>
				<span class="checkbox-description"
					>{$i18n('validation_require_file_confirmation_desc')}</span
				>
			</div>
		</label>

		<div class="allowlist-section">
			<h4 class="subsection-title">{$i18n('validation_mcp_allowlist_section')}</h4>
			<p class="subsection-help">{$i18n('validation_mcp_allowlist_help')}</p>
			<AgentMcpAllowlist
				{runningServers}
				knownServers={allKnownServers}
				value={allowlistDraft}
				onchange={onAllowlistChange}
			/>
		</div>

		<div class="auth-actions">
			{#if dirty}
				<span class="auth-dirty">{$i18n('validation_authorizations_dirty')}</span>
			{/if}
			{#if saved}
				<span class="auth-saved">{$i18n('validation_authorizations_saved')}</span>
			{/if}
			<Button variant="primary" disabled={!dirty || saving} onclick={save}>
				{saving
					? $i18n('validation_saving')
					: $i18n('validation_authorizations_save', { name: loadedConfig.name })}
			</Button>
		</div>
	{:else}
		<p class="auth-info">{$i18n('validation_authorizations_none')}</p>
	{/if}
</div>

<style>
	.agent-authorizations {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.auth-error {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-danger);
	}

	.auth-legend {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.auth-info {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.mode-context {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-secondary);
		border-left: 3px solid var(--color-accent, var(--color-border));
		border-radius: var(--border-radius-sm);
	}

	.mode-context-line {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.checkbox-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		cursor: pointer;
	}

	.checkbox-content {
		display: flex;
		flex-direction: column;
		gap: 2px;
	}

	.checkbox-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.checkbox-description {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.allowlist-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.subsection-title {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.subsection-help {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.auth-actions {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--spacing-md);
	}

	.auth-dirty {
		font-size: var(--font-size-sm);
		color: var(--color-warning, var(--color-text-secondary));
	}

	.auth-saved {
		font-size: var(--font-size-sm);
		color: var(--color-success, var(--color-text-secondary));
	}
</style>
