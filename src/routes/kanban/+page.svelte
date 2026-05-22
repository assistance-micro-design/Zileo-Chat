<!--
  Copyright 2025 Assistance Micro Design

  Kanban page — board for orchestrating recurring agent workflows.
-->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { i18n } from '$lib/i18n';
	import { tauriListen, tauriInvoke as invoke, type TauriUnlistenFn } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { locale } from '$lib/stores/locale';
	import { Badge, Button } from '$lib/components/ui';
	import { Plus, Activity } from '@lucide/svelte';

	import { kanbanStore, kanbanCardsByColumn, kanbanCards } from '$lib/stores/kanban';
	import { kanbanScheduleStore, kanbanSchedules } from '$lib/stores/kanban-schedule';
	import { agents as agentsStore, agentStore } from '$lib/stores/agents';
	import { prompts as promptsStore, promptStore } from '$lib/stores/prompts';
	import { folders as foldersStore, folderStore } from '$lib/stores/folders';

	import KanbanBoard from '$lib/components/kanban/KanbanBoard.svelte';
	import KanbanCardItem from '$lib/components/kanban/KanbanCardItem.svelte';
	import KanbanCardCreator from '$lib/components/kanban/KanbanCardCreator.svelte';
	import KanbanCardReportViewer from '$lib/components/kanban/KanbanCardReportViewer.svelte';
	import KanbanFiltres from '$lib/components/kanban/KanbanFiltres.svelte';
	import KanbanImprovePromptModal from '$lib/components/kanban/KanbanImprovePromptModal.svelte';
	import KanbanScheduleModal from '$lib/components/kanban/KanbanScheduleModal.svelte';
	import KanbanCardEditModal from '$lib/components/kanban/KanbanCardEditModal.svelte';

	import type {
		KanbanCard,
		KanbanCardCreate,
		KanbanColumn,
		KanbanScheduleCreate
	} from '$types/kanban';
	import { WorkflowExecutorService } from '$lib/services/workflowExecutor.service';

	let creatorOpen = $state(false);
	let viewerOpen = $state(false);
	let viewerCard = $state<KanbanCard | null>(null);
	let improveOpen = $state(false);
	let improvePromptId = $state<string | null>(null);
	let improveKanbanAgentId = $state<string | null>(null);
	let improveSuggestedContent = $state<string | null>(null);
	let scheduleModalOpen = $state(false);
	let scheduleModalCard = $state<KanbanCard | null>(null);
	let editModalOpen = $state(false);
	let editModalCard = $state<KanbanCard | null>(null);
	/**
	 * Set to a card id when the user clicked "Duplicate as template" on a card
	 * that has no schedule yet. The schedule modal is opened so they can attach
	 * one; if they save, the pending duplication fires (handleScheduleSaved). If
	 * they close the modal without saving, the duplication is aborted with a
	 * warning (closeSchedule).
	 */
	let pendingDuplicateCardId = $state<string | null>(null);
	let pageError = $state<string | null>(null);

	let agentFilter = $state('');
	let folderFilter = $state('');

	const SEEN_CARDS_STORAGE_KEY = 'kanban-seen-cards';

	function isCardSeen(cardId: string): boolean {
		try {
			const raw = localStorage.getItem(SEEN_CARDS_STORAGE_KEY);
			if (!raw) return false;
			const list = JSON.parse(raw) as string[];
			return Array.isArray(list) && list.includes(cardId);
		} catch {
			return false;
		}
	}

	function markCardSeen(cardId: string): void {
		try {
			const raw = localStorage.getItem(SEEN_CARDS_STORAGE_KEY);
			const list = raw ? (JSON.parse(raw) as string[]) : [];
			if (!Array.isArray(list)) return;
			if (!list.includes(cardId)) {
				list.push(cardId);
				// Soft cap to prevent unbounded growth — keep the last 500 seen IDs.
				const trimmed = list.slice(-500);
				localStorage.setItem(SEEN_CARDS_STORAGE_KEY, JSON.stringify(trimmed));
			}
		} catch {
			/* best-effort persistence */
		}
	}

	let unlistenReady: TauriUnlistenFn | null = null;
	let unlistenComplete: TauriUnlistenFn | null = null;
	let unlistenAutoAnalyzed: TauriUnlistenFn | null = null;
	let unlistenNeedsImprovement: TauriUnlistenFn | null = null;
	let unlistenAnalyzing: TauriUnlistenFn | null = null;
	let unlistenSettingsRefresh: (() => void) | null = null;

	/** Card ids currently being finalized by the Kanban agent. */
	const analyzingCardIds = new SvelteSet<string>();

	function setAnalyzing(cardId: string, on: boolean): void {
		if (on) analyzingCardIds.add(cardId);
		else analyzingCardIds.delete(cardId);
	}

	$effect(() => {
		void kanbanStore.loadCards(agentFilter || undefined);
	});

	onMount(async () => {
		try {
			await Promise.all([
				agentStore.loadAgents(),
				promptStore.loadPrompts(),
				folderStore.loadFolders(),
				kanbanScheduleStore.loadSchedules()
			]);
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Listener for workflow completions — refresh cards so column transitions
		// applied by the backend (mark_card_done_core) appear in the board.
		try {
			unlistenComplete = await tauriListen('workflow_complete', () => {
				void kanbanStore.loadCards(agentFilter || undefined);
			});
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Listener for cards promoted to "doing" by the scheduler.
		try {
			unlistenReady = await tauriListen<{ card_id: string }>('kanban:card_ready', async (event) => {
				const cardId = event.payload?.card_id;
				if (!cardId) return;
				try {
					await runCardWorkflow(cardId);
				} catch (e) {
					pageError = getErrorMessage(e);
				}
			});
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Listener for "analyzer started" — surface a "finalizing" indicator
		// on the matching review card until the verdict comes back.
		try {
			unlistenAnalyzing = await tauriListen<{ card_id: string }>('kanban:analyzing', (event) => {
				const cardId = event.payload?.card_id;
				if (cardId) setAnalyzing(cardId, true);
			});
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Listener for auto-analyze verdict (approve / reject). The backend
		// has already updated the card; we just refresh the board and surface
		// the verdict to the user via the page error / a viewer auto-open
		// (handled by the $effect on viewerCard below).
		try {
			unlistenAutoAnalyzed = await tauriListen<{
				card_id: string;
				verdict: string;
				reasoning: string;
			}>('kanban:auto_analyzed', async (event) => {
				const cardId = event.payload?.card_id;
				if (!cardId) return;
				setAnalyzing(cardId, false);
				await kanbanStore.loadCards(agentFilter || undefined);
				try {
					const updated = await kanbanStore.getCard(cardId);
					if (updated && !isCardSeen(cardId) && updated.column === 'review') {
						viewerCard = updated;
						viewerOpen = true;
						markCardSeen(cardId);
					}
				} catch (e) {
					pageError = getErrorMessage(e);
				}
			});
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Listener for auto-analyze `needs_improvement` verdict — opens the
		// improvement modal with the analyzer's suggested rewrite pre-filled.
		try {
			unlistenNeedsImprovement = await tauriListen<{
				card_id: string;
				reasoning: string;
				suggested_prompt_edit: string | null;
			}>('kanban:needs_improvement', async (event) => {
				const cardId = event.payload?.card_id;
				if (!cardId) return;
				setAnalyzing(cardId, false);
				await kanbanStore.loadCards(agentFilter || undefined);
				try {
					const card = await kanbanStore.getCard(cardId);
					if (!card || !card.prompt_id) return; // inline_prompt cards can't be edited via this modal
					improvePromptId = card.prompt_id;
					improveKanbanAgentId = card.kanban_agent_id;
					improveSuggestedContent = event.payload?.suggested_prompt_edit ?? null;
					improveOpen = true;
				} catch (e) {
					pageError = getErrorMessage(e);
				}
			});
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Cross-surface settings refresh (agents added/renamed) — reload agents list silently.
		const onSettingsRefresh = (): void => {
			void agentStore.loadAgents();
			void promptStore.loadPrompts();
			void folderStore.loadFolders();
		};
		window.addEventListener('settings:refresh', onSettingsRefresh);
		unlistenSettingsRefresh = () =>
			window.removeEventListener('settings:refresh', onSettingsRefresh);
	});

	onDestroy(() => {
		unlistenReady?.();
		unlistenComplete?.();
		unlistenAutoAnalyzed?.();
		unlistenNeedsImprovement?.();
		unlistenAnalyzing?.();
		unlistenSettingsRefresh?.();
	});

	/**
	 * Filtered card map. Folder filter is applied after the column grouping
	 * so empty columns still render. Column transitions are driven by the
	 * backend (workflow lifecycle) — there is no manual status filter.
	 */
	const filteredByColumn = $derived.by(() => {
		const all = $kanbanCardsByColumn;
		const result: Record<KanbanColumn, KanbanCard[]> = {
			todo: [],
			doing: [],
			review: [],
			done: []
		};
		for (const key of Object.keys(all) as KanbanColumn[]) {
			result[key] = all[key].filter((c) => {
				if (folderFilter && c.target_folder_id !== folderFilter) return false;
				return true;
			});
		}
		return result;
	});

	function agentName(id: string): string {
		return $agentsStore.find((a) => a.id === id)?.name ?? '';
	}

	function cardHasSchedule(cardId: string): boolean {
		return $kanbanSchedules.some((s) => s.card_template_id === cardId);
	}

	/**
	 * Live slot accounting for the header indicator. Mirrors the backend
	 * scheduler logic (`DEFAULT_MAX_CONCURRENT_WORKFLOWS = 3`).
	 */
	const SLOT_CAPACITY = 3;
	const slotsUsed = $derived($kanbanCards.filter((c) => c.column === 'doing').length);
	const queuedReady = $derived(
		$kanbanCards.filter((c) => c.column === 'todo' && c.status === 'ready').length
	);
	const slotVariant = $derived(
		slotsUsed >= SLOT_CAPACITY ? 'error' : slotsUsed > 0 ? 'warning' : 'success'
	);

	function handleFilterChange(filters: { agentId: string; folderId: string }): void {
		agentFilter = filters.agentId;
		folderFilter = filters.folderId;
	}

	async function createCard(
		payload: KanbanCardCreate,
		schedule?: Omit<KanbanScheduleCreate, 'card_template_id'>
	): Promise<void> {
		const created = await kanbanStore.createCard(payload);
		if (schedule) {
			const cardTemplateId = typeof created === 'string' ? created : '';
			if (cardTemplateId) {
				await kanbanScheduleStore.createSchedule({
					card_template_id: cardTemplateId,
					days_of_week: schedule.days_of_week,
					hour: schedule.hour,
					minute: schedule.minute
				});
			}
		}
		await kanbanStore.loadCards(agentFilter || undefined);
	}

	async function deleteCard(card: KanbanCard): Promise<void> {
		pageError = null;
		const linkedSchedule = $kanbanSchedules.find((s) => s.card_template_id === card.id);
		// Recurrence-specific confirm takes precedence — it already communicates
		// the destructive nature of the action AND the schedule loss in one prompt.
		// Otherwise, fall back to the generic "delete this card?" warning.
		const confirmKey = linkedSchedule
			? 'kanban_confirm_delete_with_schedule'
			: 'kanban_confirm_delete';
		if (!confirm($i18n(confirmKey))) return;
		if (linkedSchedule) {
			try {
				await kanbanScheduleStore.deleteSchedule(linkedSchedule.id);
			} catch (e) {
				pageError = getErrorMessage(e);
				return;
			}
		}
		try {
			await kanbanStore.deleteCard(card.id);
			viewerOpen = false;
			viewerCard = null;
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	async function validateCard(card: KanbanCard): Promise<void> {
		pageError = null;
		try {
			await kanbanStore.moveCard(card.id, 'done', 0);
			viewerOpen = false;
			viewerCard = null;
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	function openView(card: KanbanCard): void {
		viewerCard = card;
		viewerOpen = true;
	}

	function openEdit(card: KanbanCard): void {
		editModalCard = card;
		editModalOpen = true;
	}

	function closeEdit(): void {
		editModalOpen = false;
		editModalCard = null;
	}

	function openSchedule(card: KanbanCard): void {
		scheduleModalCard = card;
		scheduleModalOpen = true;
	}

	function closeSchedule(): void {
		scheduleModalOpen = false;
		scheduleModalCard = null;
		// User closed the schedule modal without saving while a duplication was
		// pending — duplication requires a recurrence, so abort with a warning.
		if (pendingDuplicateCardId) {
			pendingDuplicateCardId = null;
			pageError = $i18n('kanban_duplicate_no_schedule_warning');
		}
	}

	async function handleScheduleSaved(): Promise<void> {
		await kanbanScheduleStore.loadSchedules();
		if (!pendingDuplicateCardId) return;
		const cardId = pendingDuplicateCardId;
		pendingDuplicateCardId = null;
		const card = await kanbanStore.getCard(cardId);
		if (card) await performDuplicate(card);
	}

	async function duplicateAsTemplate(card: KanbanCard): Promise<void> {
		const hasSchedule = $kanbanSchedules.some((s) => s.card_template_id === card.id);
		if (!hasSchedule) {
			// No recurrence yet — duplicating into a fresh template would discard
			// the only existing instance for nothing. Offer to attach a recurrence
			// first; if the user agrees, the duplication fires once the schedule
			// is saved (handleScheduleSaved). Otherwise abort with a warning.
			if (!confirm($i18n('kanban_duplicate_requires_schedule_prompt'))) {
				pageError = $i18n('kanban_duplicate_no_schedule_warning');
				return;
			}
			pendingDuplicateCardId = card.id;
			openSchedule(card);
			return;
		}
		if (!confirm($i18n('kanban_confirm_duplicate_template'))) return;
		await performDuplicate(card);
	}

	async function performDuplicate(card: KanbanCard): Promise<void> {
		pageError = null;
		try {
			await invoke<KanbanCard>('duplicate_kanban_card_as_template', { cardId: card.id });
			// Reload schedules first so the badge follows the new template card,
			// then refresh the board so the source disappears and the clone shows
			// up in `todo` (or `doing` if the scheduler promoted it immediately).
			await Promise.all([
				kanbanScheduleStore.loadSchedules(),
				kanbanStore.loadCards(agentFilter || undefined)
			]);
			if (viewerCard?.id === card.id) {
				viewerOpen = false;
				viewerCard = null;
			}
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	function handleImprovePrompt(card: KanbanCard): void {
		if (!card.prompt_id) {
			pageError = $i18n('kanban_improve_no_prompt');
			return;
		}
		improvePromptId = card.prompt_id;
		improveKanbanAgentId = card.kanban_agent_id;
		improveOpen = true;
	}

	function closeImprove(): void {
		improveOpen = false;
		improvePromptId = null;
		improveKanbanAgentId = null;
		improveSuggestedContent = null;
	}

	/**
	 * When a card is promoted to "doing" by the scheduler, the page is
	 * responsible for kicking off the workflow execution: create a workflow,
	 * invoke execute_workflow_streaming with the resolved prompt + variables,
	 * then update the card with the workflow_id so the report viewer can link
	 * back to it.
	 */
	async function runCardWorkflow(cardId: string): Promise<void> {
		// Refresh first to get the new column/status from the scheduler.
		await kanbanStore.loadCards(agentFilter || undefined);
		const card = await kanbanStore.getCard(cardId);
		if (!card) return;

		// Build the message: inline_prompt or fetch the prompt content.
		let message = card.inline_prompt ?? '';
		if (!message && card.prompt_id) {
			const prompt = await invoke<{ content: string }>('get_prompt', {
				promptId: card.prompt_id
			});
			message = prompt.content;
		}

		// Interpolate variables ({{name}} → value).
		let variables: Record<string, string> = {};
		try {
			variables = JSON.parse(card.variables || '{}');
		} catch {
			variables = {};
		}
		message = message.replace(/\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g, (m, name: string) => {
			const value = variables[name];
			return typeof value === 'string' ? value : m;
		});

		// Create the workflow + attach to the card folder (if any) + execute.
		const workflowId = await invoke<string>('create_workflow', {
			name: card.title,
			agentId: card.target_agent_id
		});

		if (card.target_folder_id) {
			try {
				await invoke('move_workflow_to_folder', {
					workflowId,
					folderId: card.target_folder_id
				});
			} catch (e) {
				// Non-fatal: a missing folder shouldn't block execution.
				pageError = getErrorMessage(e);
			}
		}

		// Persist the workflow_id link on the card. mark_card_done_core (fired
		// by the workflow_complete listener) matches the card via
		// `WHERE workflow_id = $wid` — without this UPDATE the card would stay
		// stuck in 'doing' forever after the workflow finishes.
		try {
			await invoke('set_kanban_card_workflow_id', {
				cardId: card.id,
				workflowId
			});
		} catch (e) {
			pageError = getErrorMessage(e);
		}
		kanbanStore.upsertLocal({ ...card, workflow_id: workflowId });

		try {
			// Go through WorkflowExecutorService so the user message gets
			// persisted (MessageService.saveUser) BEFORE execute_workflow_streaming
			// runs. A raw invoke skipped that step, leaving an empty chat when
			// the workflow viewer reloaded blocks. The service also registers
			// the workflow in the background store so navigating away no longer
			// loses streaming state.
			const result = await WorkflowExecutorService.execute({
				workflowId,
				message,
				agentId: card.target_agent_id,
				locale: $locale
			});
			if (!result.success && result.error) {
				pageError = result.error;
			}
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		await kanbanStore.loadCards(agentFilter || undefined);
	}
</script>

<svelte:head>
	<title>Kanban — Zileo Chat</title>
</svelte:head>

<section class="kanban-page" aria-labelledby="kanban-title">
	<header class="page-head">
		<h1 id="kanban-title">{$i18n('kanban_page_title')}</h1>
		<div class="head-right">
			<div class="slot-indicators" aria-live="polite">
				<Badge variant={slotVariant}>
					<Activity size={11} aria-hidden="true" />
					{$i18n('kanban_slot_active', { used: String(slotsUsed), max: String(SLOT_CAPACITY) })}
				</Badge>
				{#if queuedReady > 0}
					<Badge variant="primary">
						{$i18n('kanban_slot_queued', { count: String(queuedReady) })}
					</Badge>
				{/if}
			</div>
			<Button variant="primary" onclick={() => (creatorOpen = true)}>
				<Plus size={16} />
				{$i18n('kanban_new_card')}
			</Button>
		</div>
	</header>

	{#if pageError}
		<p class="page-error" role="alert">{pageError}</p>
	{/if}

	<KanbanFiltres
		agents={$agentsStore}
		folders={$foldersStore}
		selectedAgentId={agentFilter}
		selectedFolderId={folderFilter}
		onchange={handleFilterChange}
	/>

	<KanbanBoard cardsByColumn={filteredByColumn}>
		{#snippet card(c)}
			<KanbanCardItem
				card={c}
				targetAgentName={agentName(c.target_agent_id)}
				hasSchedule={cardHasSchedule(c.id)}
				isAnalyzing={analyzingCardIds.has(c.id)}
				onview={openView}
				onimprove={handleImprovePrompt}
				ondelete={deleteCard}
				onschedule={openSchedule}
				onduplicate={duplicateAsTemplate}
				onedit={openEdit}
			/>
		{/snippet}
	</KanbanBoard>
</section>

<KanbanCardCreator
	open={creatorOpen}
	agents={$agentsStore}
	prompts={$promptsStore}
	folders={$foldersStore}
	defaultKanbanAgentId={agentFilter}
	onclose={() => (creatorOpen = false)}
	oncreated={createCard}
/>

<KanbanScheduleModal
	open={scheduleModalOpen}
	card={scheduleModalCard}
	onclose={closeSchedule}
	onsaved={handleScheduleSaved}
/>

<KanbanCardEditModal
	open={editModalOpen}
	card={editModalCard}
	agents={$agentsStore}
	prompts={$promptsStore}
	folders={$foldersStore}
	onclose={closeEdit}
	onsaved={() => kanbanStore.loadCards(agentFilter || undefined)}
/>

<KanbanImprovePromptModal
	open={improveOpen}
	promptId={improvePromptId}
	kanbanAgentId={improveKanbanAgentId}
	suggestedContent={improveSuggestedContent}
	onclose={closeImprove}
	onupdated={() => kanbanStore.loadCards(agentFilter || undefined)}
/>

<KanbanCardReportViewer
	open={viewerOpen}
	card={viewerCard}
	agents={$agentsStore}
	prompts={$promptsStore}
	onclose={() => {
		viewerOpen = false;
		viewerCard = null;
	}}
	onvalidate={validateCard}
	onimprove={handleImprovePrompt}
	ondelete={deleteCard}
/>

<style>
	.kanban-page {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1rem;
		height: 100%;
		min-height: 0;
		flex: 1;
		min-width: 0;
	}
	.page-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
	}
	.page-head h1 {
		margin: 0;
		font-size: 1.4rem;
	}
	.head-right {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex-wrap: wrap;
	}
	.slot-indicators {
		display: flex;
		gap: 0.35rem;
		align-items: center;
	}
	.page-error {
		color: var(--color-error);
		margin: 0;
	}
</style>
