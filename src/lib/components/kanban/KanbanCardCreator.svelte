<!--
  Copyright 2025 Assistance Micro Design

  KanbanCardCreator — modal shell with two creation modes:
   - Mode A (auto): see KanbanCardCreatorAuto.
   - Mode B (manual): see KanbanCardCreatorManual.
  This component owns the modal, the tab switcher, and submission plumbing.
  Per-mode form state lives inside each sub-pane.
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { Modal, Button } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import { canStartCompose } from '$lib/stores/kanban-compose';
	import KanbanCardCreatorAuto from './KanbanCardCreatorAuto.svelte';
	import KanbanCardCreatorManual from './KanbanCardCreatorManual.svelte';
	import type { AgentSummary } from '$types/agent';
	import type { PromptSummary } from '$types/prompt';
	import type { WorkflowFolder } from '$types/workflow';
	import type { KanbanCardCreate, KanbanScheduleCreate } from '$types/kanban';

	interface Props {
		open: boolean;
		agents: AgentSummary[];
		prompts: PromptSummary[];
		folders: WorkflowFolder[];
		/** Pre-selected Kanban agent (current filter), or empty. */
		defaultKanbanAgentId?: string;
		onclose: () => void;
		oncreated: (
			payload: KanbanCardCreate,
			schedule?: Omit<KanbanScheduleCreate, 'card_template_id'>
		) => Promise<void>;
	}

	let {
		open,
		agents,
		prompts,
		folders,
		defaultKanbanAgentId = '',
		onclose,
		oncreated
	}: Props = $props();

	type Mode = 'auto' | 'manual';
	let mode = $state<Mode>('auto');
	let submitting = $state(false);
	let error = $state<string | null>(null);
	/** True while the detached compose launch (`start_compose_card`) is in flight. */
	let launching = $state(false);

	// Force a fresh sub-pane each time the modal re-opens.
	let paneKey = $state(0);
	// `open` is the only tracked dependency ; writes are untracked so the
	// effect doesn't self-trigger (an `open=true` tick would otherwise loop
	// and continuously reset `mode` to 'auto', defeating tab clicks).
	$effect(() => {
		if (open) {
			untrack(() => {
				paneKey += 1;
				mode = 'auto';
				error = null;
				submitting = false;
				launching = false;
			});
		}
	});

	const kanbanAgentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_select_kanban_agent') },
		...agents.filter((a) => a.kind === 'kanban').map((a) => ({ value: a.id, label: a.name }))
	]);

	const targetAgentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_select_target_agent') },
		...agents.filter((a) => a.kind !== 'kanban').map((a) => ({ value: a.id, label: a.name }))
	]);

	const promptOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_select_prompt_optional') },
		...prompts.map((p) => ({ value: p.id, label: p.name }))
	]);

	const folderOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_no_folder') },
		...folders.map((f) => ({ value: f.id, label: f.name }))
	]);

	let autoPane = $state<KanbanCardCreatorAuto | undefined>(undefined);
	let manualPane = $state<KanbanCardCreatorManual | undefined>(undefined);

	function close(): void {
		onclose();
	}

	/**
	 * Auto mode: launch a DETACHED compose. On success the modal closes — the
	 * generated card lands in the /kanban validation zone (via the composingStore
	 * events + toast), so there is no inline preview / "Créer" step any more.
	 */
	async function runAutoCompose(): Promise<void> {
		launching = true;
		try {
			const launched = await autoPane?.compose();
			if (launched) close();
		} finally {
			launching = false;
		}
	}

	async function submitManual(): Promise<void> {
		const built = manualPane?.buildPayload();
		if (!built) return;
		submitting = true;
		try {
			await oncreated(built.payload, built.schedule);
			close();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			submitting = false;
		}
	}
</script>

<Modal {open} title={$i18n('kanban_create_title')} onclose={close}>
	{#snippet body()}
		<div class="mode-tabs" role="tablist">
			<button
				type="button"
				role="tab"
				class="tab"
				class:active={mode === 'auto'}
				aria-selected={mode === 'auto'}
				onclick={() => (mode = 'auto')}
			>
				{$i18n('kanban_mode_auto')}
			</button>
			<button
				type="button"
				role="tab"
				class="tab"
				class:active={mode === 'manual'}
				aria-selected={mode === 'manual'}
				onclick={() => (mode = 'manual')}
			>
				{$i18n('kanban_mode_manual')}
			</button>
		</div>

		{#if error}
			<p class="error" role="alert">{error}</p>
		{/if}

		{#key paneKey}
			{#if mode === 'auto'}
				<KanbanCardCreatorAuto
					bind:this={autoPane}
					{kanbanAgentOptions}
					{defaultKanbanAgentId}
					onerror={(m) => (error = m)}
				/>
			{:else}
				<KanbanCardCreatorManual
					bind:this={manualPane}
					{agents}
					{prompts}
					{folders}
					{kanbanAgentOptions}
					{targetAgentOptions}
					{promptOptions}
					{folderOptions}
					{defaultKanbanAgentId}
					onerror={(m) => (error = m)}
				/>
			{/if}
		{/key}
	{/snippet}
	{#snippet footer()}
		<Button variant="ghost" onclick={close} disabled={submitting || launching}>
			{$i18n('common_cancel')}
		</Button>
		{#if mode === 'auto'}
			<Button variant="primary" onclick={runAutoCompose} disabled={launching || !$canStartCompose}>
				{launching ? $i18n('kanban_composing_preview_btn') : $i18n('kanban_compose_preview')}
			</Button>
		{:else}
			<Button variant="primary" onclick={submitManual} disabled={submitting}>
				{$i18n('common_create')}
			</Button>
		{/if}
	{/snippet}
</Modal>

<style>
	.mode-tabs {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.75rem;
	}
	.tab {
		background: none;
		border: 1px solid var(--color-border);
		border-radius: 6px;
		padding: 0.35rem 0.75rem;
		cursor: pointer;
		color: var(--color-text);
	}
	.tab.active {
		background: var(--color-accent);
		color: var(--color-accent-text);
		border-color: var(--color-accent);
	}
	.error {
		color: var(--color-error);
		margin: 0.25rem 0 0.75rem;
	}
</style>
