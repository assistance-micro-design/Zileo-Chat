<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardCreatorAuto — sub-pane of KanbanCardCreator (Mode A).
  Asks the Kanban agent to compose a structured card from a free-form
  description. The generation is now DETACHED: `compose()` fires
  `start_compose_card` and returns immediately; the result arrives later as a
  `proposed` card in the /kanban validation zone (via the `composingStore`
  events + toast). No inline preview — validation moved to the board.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { get } from 'svelte/store';
	import { i18n } from '$lib/i18n';
	import { tauriInvoke as invoke } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { locale } from '$lib/stores/locale';
	import { Select, Textarea } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import { Info } from '@lucide/svelte';
	import { composingStore, canStartCompose } from '$lib/stores/kanban-compose';
	import type { ComposeStartResponse } from '$types/kanban';
	import type { SupervisorRoleState } from '$lib/utils/kanban-supervisors';
	import KanbanSupervisorNotice from './KanbanSupervisorNotice.svelte';

	interface Props {
		kanbanAgentOptions: SelectOption[];
		defaultKanbanAgentId: string;
		/** Global compose supervisor id (D7): when set, the select is locked on it. */
		globalComposeAgentId?: string;
		/** Configuration state of the compose supervisor role. */
		composeState?: SupervisorRoleState;
		/** Configuration state of the analyze supervisor role. */
		analyzeState?: SupervisorRoleState;
		/** Called when an error happens locally so the parent can render it. */
		onerror: (message: string | null) => void;
	}

	let {
		kanbanAgentOptions,
		defaultKanbanAgentId,
		globalComposeAgentId = '',
		composeState = 'unset',
		analyzeState = 'unset',
		onerror
	}: Props = $props();

	let description = $state('');
	// Manual selection (used only when the global compose agent is NOT locking it).
	let manualAgentId = $state(untrack(() => defaultKanbanAgentId));
	// D7: when a valid global compose agent is configured, lock the select on it.
	// `globalComposeAgentId` arrives asynchronously (the settings store loads on
	// modal open), so the effective id MUST be derived — initializing a `$state`
	// once would leave a locked select showing the stale manual value. The backend
	// is authoritative either way, but the UI must be honest.
	const composeLocked = $derived(!!globalComposeAgentId);
	const effectiveAgentId = $derived(composeLocked ? globalComposeAgentId : manualAgentId);

	/**
	 * Launches a DETACHED compose. Returns `true` when the generation was
	 * started (the parent then closes the modal); `false` on a validation error
	 * or a rejected launch (the message is surfaced via `onerror`).
	 */
	export async function compose(): Promise<boolean> {
		onerror(null);
		if (!effectiveAgentId) {
			onerror($i18n('kanban_error_kanban_agent_required'));
			return false;
		}
		if (!description.trim()) {
			onerror($i18n('kanban_error_description_required'));
			return false;
		}
		// Advisory front gate; the backend cap is authoritative and also refuses.
		if (!get(canStartCompose)) {
			onerror($i18n('kanban_compose_cap_reached'));
			return false;
		}
		try {
			const { card_id } = await invoke<ComposeStartResponse>('start_compose_card', {
				kanbanAgentId: effectiveAgentId,
				description,
				locale: $locale
			});
			composingStore.register(card_id, description.slice(0, 80));
			return true;
		} catch (e) {
			onerror(getErrorMessage(e));
			return false;
		}
	}
</script>

<div class="form-section">
	<Select
		label={$i18n('kanban_kanban_agent')}
		options={kanbanAgentOptions}
		value={effectiveAgentId}
		disabled={composeLocked}
		help={composeLocked ? $i18n('kanban_compose_agent_global_hint') : undefined}
		onchange={(e) => (manualAgentId = e.currentTarget.value)}
	/>
	<KanbanSupervisorNotice
		state={composeState}
		unsetKey="kanban_supervisor_compose_unset"
		danglingKey="kanban_supervisor_compose_dangling"
	/>
	<KanbanSupervisorNotice
		state={analyzeState}
		unsetKey="kanban_supervisor_analyze_unset"
		danglingKey="kanban_supervisor_analyze_dangling"
	/>
	<Textarea
		label={$i18n('kanban_describe_card')}
		value={description}
		oninput={(e) => (description = e.currentTarget.value)}
		rows={6}
	/>
	{#if !$canStartCompose}
		<p class="cap-notice" role="status">{$i18n('kanban_compose_cap_reached')}</p>
	{:else}
		<p class="info-notice">
			<Info size={16} aria-hidden="true" />
			<span>{$i18n('kanban_compose_launched')}</span>
		</p>
	{/if}
</div>

<style>
	.form-section {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	/* Info alert (mock style): pale blue surface, matching border, leading icon. */
	.info-notice {
		margin: 0;
		display: flex;
		gap: 0.5rem;
		padding: var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-info);
		background: var(--color-info-light);
		border: 1px solid rgba(59, 130, 246, 0.3);
		border-radius: var(--border-radius-md);
	}
	.info-notice :global(svg) {
		flex-shrink: 0;
		margin-top: 2px;
	}
	.cap-notice {
		margin: 0;
		padding: 0.5rem 0.7rem;
		font-size: var(--font-size-xs);
		color: var(--color-warning);
		background: var(--color-warning-bg);
		border-radius: var(--border-radius-md);
	}
</style>
