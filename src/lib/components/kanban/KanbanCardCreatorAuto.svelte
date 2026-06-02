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
	import { composingStore, canStartCompose } from '$lib/stores/kanban-compose';
	import type { ComposeStartResponse } from '$types/kanban';

	interface Props {
		kanbanAgentOptions: SelectOption[];
		defaultKanbanAgentId: string;
		/** Called when an error happens locally so the parent can render it. */
		onerror: (message: string | null) => void;
	}

	let { kanbanAgentOptions, defaultKanbanAgentId, onerror }: Props = $props();

	let description = $state('');
	let kanbanAgentId = $state(untrack(() => defaultKanbanAgentId));

	/**
	 * Launches a DETACHED compose. Returns `true` when the generation was
	 * started (the parent then closes the modal); `false` on a validation error
	 * or a rejected launch (the message is surfaced via `onerror`).
	 */
	export async function compose(): Promise<boolean> {
		onerror(null);
		if (!kanbanAgentId) {
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
				kanbanAgentId,
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
		value={kanbanAgentId}
		onchange={(e) => (kanbanAgentId = e.currentTarget.value)}
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
		<p class="hint">{$i18n('kanban_compose_launched')}</p>
	{/if}
</div>

<style>
	.form-section {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
	}
	.hint {
		margin: 0;
		font-size: 0.8rem;
		color: var(--color-text-muted);
		font-style: italic;
	}
	.cap-notice {
		margin: 0;
		padding: 0.5rem 0.7rem;
		font-size: 0.82rem;
		color: var(--color-warning, #b45309);
		background: var(--color-warning-bg);
		border-radius: 6px;
	}
</style>
