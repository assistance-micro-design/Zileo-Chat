<!--
  Copyright 2025 Assistance Micro Design

  Kanban page — board for orchestrating recurring agent workflows.
-->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { tauriListen, tauriInvoke as invoke, type TauriUnlistenFn } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { locale } from '$lib/stores/locale';
	import { Button } from '$lib/components/ui';
	import { Plus } from '@lucide/svelte';

	import { kanbanStore, kanbanCardsByColumn } from '$lib/stores/kanban';
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

	import type {
		KanbanCard,
		KanbanCardCreate,
		KanbanCardStatus,
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
	let pageError = $state<string | null>(null);

	let agentFilter = $state('');
	let folderFilter = $state('');
	let statusFilter = $state<KanbanCardStatus | ''>('');

	let unlistenReady: TauriUnlistenFn | null = null;
	let unlistenComplete: TauriUnlistenFn | null = null;
	let unlistenSettingsRefresh: (() => void) | null = null;

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
		unlistenSettingsRefresh?.();
	});

	/**
	 * Filtered card map. Visual filtering is applied after the column grouping
	 * so empty columns still render their drop zones.
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
				if (statusFilter && c.status !== statusFilter) return false;
				return true;
			});
		}
		return result;
	});

	function agentName(id: string): string {
		return $agentsStore.find((a) => a.id === id)?.name ?? '';
	}

	function handleFilterChange(filters: {
		agentId: string;
		folderId: string;
		status: KanbanCardStatus | '';
	}): void {
		agentFilter = filters.agentId;
		folderFilter = filters.folderId;
		statusFilter = filters.status;
	}

	async function handleDrop(
		cardIds: string[],
		targetColumn: KanbanColumn,
		targetOrder: number
	): Promise<void> {
		pageError = null;
		try {
			for (const id of cardIds) {
				await kanbanStore.moveCard(id, targetColumn, targetOrder);
			}
		} catch (e) {
			pageError = getErrorMessage(e);
		}
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
		if (linkedSchedule) {
			const confirmed = confirm($i18n('kanban_confirm_delete_with_schedule'));
			if (!confirmed) return;
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
		<Button variant="primary" onclick={() => (creatorOpen = true)}>
			<Plus size={16} />
			{$i18n('kanban_new_card')}
		</Button>
	</header>

	{#if pageError}
		<p class="page-error" role="alert">{pageError}</p>
	{/if}

	<KanbanFiltres
		agents={$agentsStore}
		folders={$foldersStore}
		selectedAgentId={agentFilter}
		selectedFolderId={folderFilter}
		selectedStatus={statusFilter}
		onchange={handleFilterChange}
	/>

	<KanbanBoard cardsByColumn={filteredByColumn} ondrop={handleDrop}>
		{#snippet card(c)}
			<KanbanCardItem
				card={c}
				targetAgentName={agentName(c.target_agent_id)}
				onview={openView}
				onimprove={handleImprovePrompt}
				ondelete={deleteCard}
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

<KanbanImprovePromptModal
	open={improveOpen}
	promptId={improvePromptId}
	kanbanAgentId={improveKanbanAgentId}
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
	.page-error {
		color: var(--color-error);
		margin: 0;
	}
</style>
