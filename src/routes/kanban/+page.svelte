<!--
  Copyright 2025 Assistance Micro Design

  Kanban page — board for orchestrating recurring agent workflows.
-->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { get } from 'svelte/store';
	import { i18n } from '$lib/i18n';
	import { tauriListen, tauriInvoke as invoke, type TauriUnlistenFn } from '$lib/tauri';
	import { getErrorMessage } from '$lib/utils/error';
	import { locale } from '$lib/stores/locale';
	import { Badge, Button, DeleteConfirmModal, Spinner } from '$lib/components/ui';
	import { Activity, Bot, Check, Sparkles, X, Pencil, TriangleAlert } from '@lucide/svelte';

	import { kanbanStore, kanbanCardsByColumn, kanbanCards } from '$lib/stores/kanban';
	import {
		kanbanEventsStore,
		analyzingCardIds as analyzingCardIdsStore,
		boardDirtySeq,
		pendingVerdict,
		pendingNeedsImprovement
	} from '$lib/stores/kanban-events';
	import { composingEntries, composingCount, proposedDirtySeq } from '$lib/stores/kanban-compose';
	import { runningWorkflows, backgroundWorkflowsStore } from '$lib/stores/background-workflows';
	import { maxConcurrentWorkflows } from '$lib/stores/kanban-runtime';
	import { executionBlocksStore } from '$lib/stores/execution-blocks';
	import { userQuestionStore } from '$lib/stores/user-question';
	import { kanbanScheduleStore, kanbanSchedules } from '$lib/stores/kanban-schedule';
	import { agents as agentsStore, agentStore } from '$lib/stores/agents';
	import { prompts as promptsStore, promptStore } from '$lib/stores/prompts';
	import { folders as foldersStore, folderStore } from '$lib/stores/folders';
	import { kanbanSupervisorStore, analyzeSupervisorId } from '$lib/stores/kanban-settings';
	import { supervisorRoleState } from '$lib/utils/kanban-supervisors';
	import { LocalStorage, STORAGE_KEYS } from '$lib/services/localStorage.service';

	import KanbanBoard from '$lib/components/kanban/KanbanBoard.svelte';
	import KanbanCardItem from '$lib/components/kanban/KanbanCardItem.svelte';
	import KanbanCardReportViewer from '$lib/components/kanban/KanbanCardReportViewer.svelte';
	import KanbanFiltres from '$lib/components/kanban/KanbanFiltres.svelte';
	import KanbanImprovePromptModal from '$lib/components/kanban/KanbanImprovePromptModal.svelte';
	import KanbanScheduleModal from '$lib/components/kanban/KanbanScheduleModal.svelte';
	import KanbanCardEditModal from '$lib/components/kanban/KanbanCardEditModal.svelte';

	import type { KanbanCard, KanbanColumn } from '$types/kanban';
	import { WorkflowExecutorService } from '$lib/services/workflowExecutor.service';

	let viewerOpen = $state(false);
	let viewerCard = $state<KanbanCard | null>(null);
	let improveOpen = $state(false);
	let improvePromptId = $state<string | null>(null);
	let improveKanbanAgentId = $state<string | null>(null);
	let improveSuggestedContent = $state<string | null>(null);
	/** Card whose prompt is being improved — needed to re-queue it (K4). */
	let improveCardId = $state<string | null>(null);
	let scheduleModalOpen = $state(false);
	let scheduleModalCard = $state<KanbanCard | null>(null);
	let editModalOpen = $state(false);
	let editModalCard = $state<KanbanCard | null>(null);
	/**
	 * Card pending deletion confirmation. Set when the user clicks the trash
	 * icon; cleared on confirm or cancel. The delete modal disambiguates
	 * three cases via `deleteVariant`: a stale stuck card (workflow crashed),
	 * a card with a recurrence schedule attached, or a plain card.
	 */
	let pendingDeleteCard = $state<KanbanCard | null>(null);
	let deleteVariant = $state<'stuck' | 'with_schedule' | 'plain'>('plain');
	let deleting = $state(false);
	/**
	 * Card pending a duplicate-as-template confirmation. The flow has two
	 * phases: when the source card has no schedule, we first ask if the user
	 * wants to attach one (`attach-schedule`); when it already has one, we
	 * confirm the destructive duplicate (`confirm-duplicate`).
	 */
	let pendingDuplicateCard = $state<KanbanCard | null>(null);
	let duplicatePhase = $state<'attach-schedule' | 'confirm-duplicate'>('confirm-duplicate');
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

	function isStringArray(value: unknown): value is string[] {
		return Array.isArray(value) && value.every((item) => typeof item === 'string');
	}

	function getSeenCardIds(): string[] {
		return LocalStorage.get(STORAGE_KEYS.KANBAN_SEEN_CARDS, [], isStringArray);
	}

	function isCardSeen(cardId: string): boolean {
		return getSeenCardIds().includes(cardId);
	}

	function markCardSeen(cardId: string): void {
		const list = getSeenCardIds();
		if (!list.includes(cardId)) {
			list.push(cardId);
			// Soft cap to prevent unbounded growth — keep the last 500 seen IDs.
			LocalStorage.set(STORAGE_KEYS.KANBAN_SEEN_CARDS, list.slice(-500));
		}
	}

	// card_ready and settings:refresh stay page-local: the former launches a
	// workflow via the page-coupled runCardWorkflow, the latter reloads page
	// stores. The Kanban analyze lifecycle (analyzing / auto_analyzed /
	// needs_improvement) and board-refresh signals (workflow_complete /
	// cards_purged) are owned by the root-mounted `kanbanEventsStore` so they
	// survive navigation away from /kanban.
	let unlistenReady: TauriUnlistenFn | null = null;
	let unlistenSettingsRefresh: (() => void) | null = null;

	/** Card ids currently being finalized by the Kanban agent (root store). */
	const analyzingSet = $derived(new Set($analyzingCardIdsStore));

	/** Ids of the live Kanban-kind agents, for the supervisor integrity check. */
	const kanbanAgentIdSet = $derived(
		new Set($agentsStore.filter((a) => a.kind === 'kanban').map((a) => a.id))
	);
	/**
	 * True when a global analyze supervisor IS configured but no longer matches a
	 * live Kanban agent (deleted / demoted). This silently breaks re-analyze and
	 * the boot catch-up for cards already in review, so it warrants a board-level
	 * banner — not just a nudge in the creator modal (State B / D8). The banner
	 * clears itself once the configuration is fixed (reactive on both stores).
	 */
	const analyzeSupervisorDangling = $derived(
		supervisorRoleState($analyzeSupervisorId, kanbanAgentIdSet) === 'dangling'
	);

	$effect(() => {
		void kanbanStore.loadCards(agentFilter || undefined);
	});

	// Reload the board whenever the root store signals it went stale (a
	// workflow finished, a verdict was applied, or stale cards were purged) —
	// including while the user was on another page. Reads the page's current
	// agent filter so the scoped view is preserved.
	$effect(() => {
		// Touch the counter so this effect re-runs on every bump.
		void $boardDirtySeq;
		void kanbanStore.loadCards(agentFilter || undefined);
	});

	// Drain a buffered approve/reject verdict: open the report viewer on the
	// matching review card (once, if unseen) so a verdict that landed while the
	// user was away still surfaces when they return to /kanban. Reads the
	// derived store reactively; clearing it resolves the effect (re-run sees
	// null and bails) so there is no replay loop.
	$effect(() => {
		const verdict = $pendingVerdict;
		if (!verdict) return;
		kanbanEventsStore.clearVerdict();
		void (async () => {
			try {
				const updated = await kanbanStore.getCard(verdict.cardId);
				if (updated && !isCardSeen(verdict.cardId) && updated.column === 'review') {
					viewerCard = updated;
					viewerOpen = true;
					markCardSeen(verdict.cardId);
				}
			} catch (e) {
				pageError = getErrorMessage(e);
			}
		})();
	});

	// Drain a buffered needs_improvement verdict: pre-open the improve-prompt
	// modal with the analyzer's suggested rewrite.
	$effect(() => {
		const pending = $pendingNeedsImprovement;
		if (!pending) return;
		kanbanEventsStore.clearNeedsImprovement();
		void (async () => {
			try {
				const card = await kanbanStore.getCard(pending.cardId);
				if (!card || !card.prompt_id) return; // inline_prompt cards can't be edited via this modal
				improvePromptId = card.prompt_id;
				improveKanbanAgentId = card.kanban_agent_id;
				improveSuggestedContent = pending.suggestedPromptEdit;
				improveCardId = card.id;
				improveOpen = true;
			} catch (e) {
				pageError = getErrorMessage(e);
			}
		})();
	});

	onMount(async () => {
		try {
			await Promise.all([
				agentStore.loadAgents(),
				promptStore.loadPrompts(),
				folderStore.loadFolders(),
				kanbanScheduleStore.loadSchedules(),
				kanbanSupervisorStore.load()
			]);
		} catch (e) {
			pageError = getErrorMessage(e);
		}

		// Listener for cards promoted to "doing" by the scheduler. Kept on the
		// page (not the root store) because runCardWorkflow is page-coupled
		// (WorkflowExecutorService, variable interpolation, folder move).
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

		// The analyze-lifecycle listeners (kanban:analyzing / auto_analyzed /
		// needs_improvement) and the board-refresh signals (workflow_complete /
		// kanban:cards_purged) are owned by `kanbanEventsStore` at the app root.
		// This page reacts to them via the $effect blocks on `boardDirtySeq`,
		// `pendingVerdict` and `pendingNeedsImprovement` declared above, so a
		// verdict that arrives while the user is on another page is not lost.

		// Forward the viewed workflow's stream chunks to the shared execution
		// store so the per-card review chat (KanbanCardReportViewer) streams the
		// supervisor's tool calls live. `setForwardCallbacks` is global (the
		// background store is root-mounted); the agent page re-registers its own
		// richer version on its mount, so last-writer-wins is harmless. No token
		// mirroring here: the card modal has no metrics bar. Questions are still
		// routed so background-workflow question handling is not regressed.
		backgroundWorkflowsStore.setForwardCallbacks(
			(chunk) => executionBlocksStore.processChunk(chunk),
			() => executionBlocksStore.complete(),
			(payload, workflowId, isViewed) =>
				userQuestionStore.handleQuestionForWorkflow(payload, workflowId, isViewed)
		);

		// Cross-surface settings refresh (agents added/renamed) — reload agents list silently.
		const onSettingsRefresh = (): void => {
			void agentStore.loadAgents();
			void promptStore.loadPrompts();
			void folderStore.loadFolders();
		};
		window.addEventListener('settings:refresh', onSettingsRefresh);
		unlistenSettingsRefresh = () =>
			window.removeEventListener('settings:refresh', onSettingsRefresh);

		// K1(b): reconcile cards the scheduler promoted to `doing` while this
		// page was not mounted to consume `kanban:card_ready` — they are stuck
		// `doing` with no workflow_id. Relaunch their worker now.
		void reconcileOrphanedDoingCards();
	});

	/**
	 * Reclaims orphaned `doing` cards on mount (K1 frontend half). The scheduler
	 * promotes ready cards to `doing` and emits `kanban:card_ready`, but that
	 * event is only consumed while /kanban is mounted; cards promoted while the
	 * user was elsewhere stay `doing` with `workflow_id` NONE. Re-launch their
	 * worker. `runCardWorkflow` re-verifies the card is still `doing`+unlinked
	 * right before acting, so a card the backend orphan-reclaim just reset to
	 * `todo` (or one already linked to a running workflow) is skipped — no
	 * duplicate or stale launch.
	 */
	async function reconcileOrphanedDoingCards(): Promise<void> {
		try {
			await kanbanStore.loadCards(agentFilter || undefined);
			const orphans = get(kanbanCards).filter((c) => c.column === 'doing' && !c.workflow_id);
			for (const c of orphans) {
				await runCardWorkflow(c.id);
			}
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	onDestroy(() => {
		unlistenReady?.();
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

	/**
	 * Generated cards awaiting human validation. Excluded from the board
	 * (kanbanCardsByColumn skips `proposed`) but kept in the raw `$kanbanCards`
	 * list, which this derives from — they surface only in the validation zone.
	 */
	const proposedCards = $derived($kanbanCards.filter((c) => c.status === 'proposed'));

	/** True when there is anything to show in the validation zone. */
	const hasComposeActivity = $derived(proposedCards.length > 0 || $composingCount > 0);

	// Reload the board (and thus `proposedCards`) whenever a compose finishes —
	// the root composingStore bumps `proposedDirtySeq` on each `compose_ready`,
	// so a generation that completes while the user is on /kanban still surfaces
	// its proposed card without a manual refresh.
	$effect(() => {
		void $proposedDirtySeq;
		void kanbanStore.loadCards(agentFilter || undefined);
	});

	function agentName(id: string): string {
		return $agentsStore.find((a) => a.id === id)?.name ?? '';
	}

	/** Validate a generated card → status='ready' so the scheduler runs it. */
	async function approveProposedCard(card: KanbanCard): Promise<void> {
		pageError = null;
		try {
			await invoke('approve_proposed_card', { cardId: card.id });
			await kanbanStore.loadCards(agentFilter || undefined);
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	/** Reject a generated card → delete it outright. */
	async function rejectProposedCard(card: KanbanCard): Promise<void> {
		pageError = null;
		try {
			await kanbanStore.deleteCard(card.id);
			await kanbanStore.loadCards(agentFilter || undefined);
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	function cardHasSchedule(cardId: string): boolean {
		return $kanbanSchedules.some((s) => s.card_template_id === cardId);
	}

	/**
	 * Live slot accounting for the header indicator. The capacity is the backend
	 * worker-promotion budget (`DEFAULT_MAX_CONCURRENT_WORKFLOWS`), loaded once at
	 * boot via `get_max_concurrent_workflows` (root layout) — never a recopied
	 * literal. `null` until that one-shot load resolves; the badge shows a
	 * placeholder and `slotVariant` cannot be `error` until the cap is known.
	 */
	const slotCapacity = $derived($maxConcurrentWorkflows);
	const slotsUsed = $derived($kanbanCards.filter((c) => c.column === 'doing').length);
	const queuedReady = $derived(
		$kanbanCards.filter((c) => c.column === 'todo' && c.status === 'ready').length
	);
	const slotVariant = $derived(
		slotCapacity !== null && slotsUsed >= slotCapacity
			? 'error'
			: slotsUsed > 0
				? 'warning'
				: 'success'
	);

	function handleFilterChange(filters: { agentId: string; folderId: string }): void {
		agentFilter = filters.agentId;
		folderFilter = filters.folderId;
	}

	/**
	 * Opens the delete confirmation modal for the given card. Variant is
	 * resolved here (stuck > with_schedule > plain) so the modal can show
	 * the right title and warning. Actual deletion happens in
	 * `confirmDeleteCard` when the user clicks the destructive button.
	 */
	function requestDeleteCard(card: KanbanCard): void {
		pageError = null;
		const isStuck =
			card.column === 'doing' && !$runningWorkflows.some((w) => w.workflowId === card.workflow_id);
		const linkedSchedule = $kanbanSchedules.find((s) => s.card_template_id === card.id);
		deleteVariant = isStuck ? 'stuck' : linkedSchedule ? 'with_schedule' : 'plain';
		pendingDeleteCard = card;
	}

	function cancelDeleteCard(): void {
		if (deleting) return;
		pendingDeleteCard = null;
	}

	async function confirmDeleteCard(): Promise<void> {
		const card = pendingDeleteCard;
		if (!card) return;
		deleting = true;
		pageError = null;
		const linkedSchedule = $kanbanSchedules.find((s) => s.card_template_id === card.id);
		if (linkedSchedule) {
			try {
				await kanbanScheduleStore.deleteSchedule(linkedSchedule.id);
			} catch (e) {
				pageError = getErrorMessage(e);
				deleting = false;
				return;
			}
		}
		try {
			await kanbanStore.deleteCard(card.id);
			viewerOpen = false;
			viewerCard = null;
			pendingDeleteCard = null;
		} catch (e) {
			pageError = getErrorMessage(e);
		} finally {
			deleting = false;
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

	/**
	 * Manually re-run the Kanban agent's report analysis for a card stuck in
	 * review. Mirrors what the `workflow_complete` listener does automatically;
	 * used when the auto-analyze silently failed (e.g. the model never called
	 * SubmitAnalysis, a provider error, or the app was closed mid-workflow).
	 * Errors are propagated so the viewer surfaces them inline.
	 */
	async function reanalyzeCard(card: KanbanCard): Promise<void> {
		await invoke('analyze_card_report', { cardId: card.id });
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

	function duplicateAsTemplate(card: KanbanCard): void {
		const hasSchedule = $kanbanSchedules.some((s) => s.card_template_id === card.id);
		// No recurrence yet → ask if the user wants to attach one first.
		// Has a recurrence → ask to confirm the destructive duplicate.
		// In both cases the actual action runs in `confirmDuplicate`.
		duplicatePhase = hasSchedule ? 'confirm-duplicate' : 'attach-schedule';
		pendingDuplicateCard = card;
	}

	function cancelDuplicate(): void {
		const wasAttachPhase = duplicatePhase === 'attach-schedule';
		const card = pendingDuplicateCard;
		pendingDuplicateCard = null;
		// Declining the "attach schedule" step is a deliberate abort — surface
		// the warning so the user understands why nothing happened.
		if (wasAttachPhase && card) {
			pageError = $i18n('kanban_duplicate_no_schedule_warning');
		}
	}

	async function confirmDuplicate(): Promise<void> {
		const card = pendingDuplicateCard;
		const phase = duplicatePhase;
		pendingDuplicateCard = null;
		if (!card) return;
		if (phase === 'attach-schedule') {
			pendingDuplicateCardId = card.id;
			openSchedule(card);
			return;
		}
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
		improveCardId = card.id;
		improveOpen = true;
	}

	function closeImprove(): void {
		improveOpen = false;
		improvePromptId = null;
		improveKanbanAgentId = null;
		improveSuggestedContent = null;
		improveCardId = null;
	}

	/**
	 * K5: re-queue a failed/rejected review card for a fresh run. Reuses the
	 * Phase 1 re-queue (Review→Todo resets status='ready' regardless of the
	 * card's failed/done status) so the scheduler relaunches it. Failed cards
	 * are never auto-deleted — this just makes them actionable.
	 */
	async function retryCard(card: KanbanCard): Promise<void> {
		try {
			await kanbanStore.moveCard(card.id, 'todo', 0);
			await kanbanStore.loadCards(agentFilter || undefined);
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	/**
	 * K4: after the prompt is improved, re-queue the card (Review→Todo, which
	 * Phase 1 resets to status='ready') so the scheduler relaunches a FRESH run
	 * that reads the corrected prompt — closing the needs_improvement loop
	 * instead of leaving a stale report in review.
	 */
	async function improveSavedRequeue(): Promise<void> {
		const cardId = improveCardId;
		if (!cardId) return;
		try {
			await kanbanStore.moveCard(cardId, 'todo', 0);
			await kanbanStore.loadCards(agentFilter || undefined);
		} catch (e) {
			pageError = getErrorMessage(e);
		}
	}

	/** Cards whose worker launch is in flight — per-card reentrancy guard. */
	const launchingCardIds = new SvelteSet<string>();

	/**
	 * Reentrancy-guarded entry point for launching a card's worker. Two triggers
	 * can fire for the same card in quick succession — the mount reconciliation
	 * (`reconcileOrphanedDoingCards`) and a `kanban:card_ready` event — and there
	 * are several awaits inside `launchCardWorkflow` between the `!workflow_id`
	 * re-check and the `set_kanban_card_workflow_id` that closes the window. Both
	 * could otherwise pass the check and start two workflows for one card. Keyed
	 * by cardId: WorkflowExecutorService's own double-submit guard is keyed by the
	 * per-call workflowId (fresh each launch), so it cannot catch this.
	 */
	async function runCardWorkflow(cardId: string): Promise<void> {
		if (launchingCardIds.has(cardId)) return;
		launchingCardIds.add(cardId);
		try {
			await launchCardWorkflow(cardId);
		} finally {
			launchingCardIds.delete(cardId);
		}
	}

	/**
	 * When a card is promoted to "doing" by the scheduler, the page is
	 * responsible for kicking off the workflow execution: create a workflow,
	 * invoke execute_workflow_streaming with the resolved prompt + variables,
	 * then update the card with the workflow_id so the report viewer can link
	 * back to it. Always reached through `runCardWorkflow` (reentrancy guard).
	 */
	async function launchCardWorkflow(cardId: string): Promise<void> {
		// Refresh first to get the new column/status from the scheduler.
		await kanbanStore.loadCards(agentFilter || undefined);
		const card = await kanbanStore.getCard(cardId);
		if (!card) return;
		// Re-verify the card still needs a worker run: it must be in `doing` with
		// no workflow yet (K1). Guards a stale card_ready or the mount
		// reconciliation racing the backend orphan-reclaim, which may have just
		// reset the card to todo — relaunching then would start a workflow on a
		// todo card. Also skips a duplicate launch once workflow_id is set.
		if (card.column !== 'doing' || card.workflow_id) return;

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
			//
			// bypassConcurrencyGate: the backend already authorized this run by
			// promoting the card (DEFAULT_MAX_CONCURRENT_WORKFLOWS is the sole
			// authority). Re-applying the interactive /agent gate would refuse a
			// legitimately promoted card, stranding it as a `doing` zombie.
			const result = await WorkflowExecutorService.execute(
				{
					workflowId,
					message,
					agentId: card.target_agent_id,
					locale: $locale
				},
				undefined,
				{ bypassConcurrencyGate: true }
			);
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
		<div class="slot-indicators" aria-live="polite">
			<Badge variant={slotVariant}>
				<Activity size={11} aria-hidden="true" />
				{$i18n('kanban_slot_active', {
					used: String(slotsUsed),
					max: slotCapacity !== null ? String(slotCapacity) : '—'
				})}
			</Badge>
			{#if queuedReady > 0}
				<Badge variant="primary">
					{$i18n('kanban_slot_queued', { count: String(queuedReady) })}
				</Badge>
			{/if}
		</div>
		<div class="head-right">
			<KanbanFiltres
				agents={$agentsStore}
				folders={$foldersStore}
				selectedAgentId={agentFilter}
				selectedFolderId={folderFilter}
				onchange={handleFilterChange}
			/>
		</div>
	</header>

	{#if pageError}
		<p class="page-error" role="alert">{pageError}</p>
	{/if}

	{#if analyzeSupervisorDangling}
		<p class="supervisor-banner" role="alert">
			<TriangleAlert size={16} aria-hidden="true" />
			<span class="supervisor-banner-text">
				{$i18n('kanban_supervisor_analyze_dangling_board')}
			</span>
			<a class="supervisor-banner-link" href="/settings/kanban">
				{$i18n('kanban_supervisor_configure_link')}
			</a>
		</p>
	{/if}

	{#if hasComposeActivity}
		<section class="proposed-zone" aria-labelledby="proposed-zone-title">
			<h2 id="proposed-zone-title" class="proposed-zone-title">
				<Sparkles size={16} aria-hidden="true" />
				{$i18n('kanban_proposed_zone_title')}
			</h2>

			{#each $composingEntries as entry (entry.cardId)}
				<article class="composing-row" aria-live="polite">
					<Spinner size="sm" />
					<span class="composing-text">
						{$i18n('kanban_composing_count', { count: String($composingCount) })}
						{#if entry.titleHint}
							— {entry.titleHint}
						{/if}
					</span>
				</article>
			{/each}

			{#if proposedCards.length > 0}
				<div class="proposed-cards">
					{#each proposedCards as card (card.id)}
						<article class="proposed-card">
							<div class="proposed-card-head">
								<h3 class="proposed-card-title">{card.title}</h3>
								<Badge variant="warning">{$i18n('kanban_proposed_badge')}</Badge>
							</div>
							{#if agentName(card.target_agent_id)}
								<p class="proposed-card-meta">
									<Bot size={13} aria-hidden="true" />
									{agentName(card.target_agent_id)}
								</p>
							{/if}
							{#if card.description}
								<p class="proposed-card-desc">{card.description}</p>
							{/if}
							<div class="proposed-card-actions">
								<Button variant="primary" size="sm" onclick={() => approveProposedCard(card)}>
									<Check size={14} />
									{$i18n('kanban_proposed_validate')}
								</Button>
								<Button variant="outline" size="sm" onclick={() => openEdit(card)}>
									<Pencil size={14} />
									{$i18n('kanban_proposed_review')}
								</Button>
								<Button variant="ghost" size="sm" onclick={() => rejectProposedCard(card)}>
									<X size={14} />
									{$i18n('kanban_proposed_reject')}
								</Button>
							</div>
						</article>
					{/each}
				</div>
			{/if}
		</section>
	{/if}

	<KanbanBoard cardsByColumn={filteredByColumn}>
		{#snippet card(c)}
			<KanbanCardItem
				card={c}
				targetAgentName={agentName(c.target_agent_id)}
				hasSchedule={cardHasSchedule(c.id)}
				isAnalyzing={analyzingSet.has(c.id)}
				onview={openView}
				onimprove={handleImprovePrompt}
				ondelete={requestDeleteCard}
				onschedule={openSchedule}
				onduplicate={duplicateAsTemplate}
				onedit={openEdit}
				onretry={retryCard}
			/>
		{/snippet}
	</KanbanBoard>
</section>

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
	onsavedrequeue={improveSavedRequeue}
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
	ondelete={requestDeleteCard}
	onreanalyze={reanalyzeCard}
	onboardchanged={async () => {
		await kanbanStore.loadCards(agentFilter || undefined);
		if (viewerCard) {
			const refreshed = await kanbanStore.getCard(viewerCard.id);
			if (refreshed) viewerCard = refreshed;
		}
	}}
/>

<DeleteConfirmModal
	open={pendingDuplicateCard !== null}
	titleKey={duplicatePhase === 'attach-schedule'
		? 'kanban_duplicate_attach_schedule_title'
		: 'kanban_duplicate_confirm_title'}
	confirmMessageKey={duplicatePhase === 'attach-schedule'
		? 'kanban_duplicate_requires_schedule_prompt'
		: 'kanban_confirm_duplicate_template'}
	itemName={pendingDuplicateCard?.title}
	deleting={false}
	variant="primary"
	confirmLabelKey={duplicatePhase === 'attach-schedule'
		? 'kanban_duplicate_attach_schedule_confirm'
		: 'kanban_duplicate_confirm_label'}
	onConfirm={confirmDuplicate}
	onCancel={cancelDuplicate}
/>

<DeleteConfirmModal
	open={pendingDeleteCard !== null}
	titleKey={deleteVariant === 'stuck'
		? 'kanban_delete_modal_title_stuck'
		: deleteVariant === 'with_schedule'
			? 'kanban_delete_modal_title_with_schedule'
			: 'kanban_delete_modal_title'}
	confirmMessageKey={deleteVariant === 'stuck'
		? 'kanban_confirm_force_delete_stuck'
		: deleteVariant === 'with_schedule'
			? 'kanban_confirm_delete_with_schedule'
			: 'kanban_confirm_delete'}
	itemName={pendingDeleteCard?.title}
	{deleting}
	onConfirm={confirmDeleteCard}
	onCancel={cancelDeleteCard}
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
	/* Single header row, mock layout: title + live badges inline, filter
	   selects pushed to the far right. */
	.page-head {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		flex-wrap: wrap;
	}
	.page-head h1 {
		margin: 0;
		font-size: var(--font-size-2xl);
	}
	.head-right {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
		margin-left: auto;
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
	/* Integrity banner: the configured analyze supervisor no longer exists. Amber
	   warning surface, non-dismissible (clears itself once reconfigured). */
	.supervisor-banner {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		flex-wrap: wrap;
		margin: 0;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-warning);
		background: var(--color-warning-bg);
		border: 1px solid var(--color-warning);
		border-radius: var(--border-radius-md);
	}
	.supervisor-banner :global(svg) {
		flex-shrink: 0;
	}
	.supervisor-banner-text {
		flex: 1;
		min-width: 0;
	}
	.supervisor-banner-link {
		flex-shrink: 0;
		font-weight: var(--font-weight-medium);
		color: inherit;
		text-decoration: underline;
	}
	/* Review zone on the site's signature cream surface, with a dashed brand
	   border and a soft halo so proposed cards read as "awaiting a decision". */
	.proposed-zone {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
		padding: var(--spacing-md);
		border: 1px dashed var(--color-accent-hover);
		border-radius: var(--border-radius-lg);
		background: var(--surface-cream);
		box-shadow: var(--glow-accent-soft);
		color: var(--color-text-primary);
	}
	.proposed-zone-title {
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-accent-deep);
	}
	.composing-row {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-size: var(--font-size-xs);
		font-style: italic;
		color: var(--color-accent-deep);
	}
	/* Proposed cards as board-style vignettes: grid of cards carrying the
	   brand-accent side rib (mirrors the column cards' status rib). */
	.proposed-cards {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
		gap: 0.5rem;
	}
	.proposed-card {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
		min-width: 0;
		padding: 0.6rem 0.75rem;
		background: var(--surface-1);
		color: var(--color-text-primary);
		border: 1px solid var(--color-border);
		border-left: 3px solid var(--color-accent-deep);
		border-radius: var(--border-radius-md);
		box-shadow: var(--shadow-xs);
	}
	.proposed-card-head {
		display: flex;
		align-items: flex-start;
		gap: 0.5rem;
	}
	.proposed-card-title {
		margin: 0;
		flex: 1;
		min-width: 0;
		font-size: var(--font-size-sm);
		font-weight: 600;
		line-height: 1.35;
	}
	.proposed-card-meta {
		margin: 0;
		display: flex;
		align-items: center;
		gap: 0.25rem;
		font-size: var(--font-size-2xs);
		color: var(--color-text-tertiary);
	}
	.proposed-card-desc {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}
	.proposed-card-actions {
		display: flex;
		flex-wrap: wrap;
		gap: 0.25rem;
		justify-content: flex-end;
		margin-top: 0.35rem;
	}
</style>
