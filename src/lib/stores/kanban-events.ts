/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * Kanban events store — root-mounted listener for the Kanban analyze lifecycle.
 *
 * The Kanban auto-analyze runs in a detached backend task (fired by the
 * `workflow_complete` listener in `main.rs`), independent of which page is
 * mounted. Its UI signals (`kanban:analyzing` / `kanban:auto_analyzed` /
 * `kanban:needs_improvement`) were previously listened to in
 * `/kanban/+page.svelte` `onMount` and torn down in `onDestroy`. Navigating
 * away (e.g. to Settings) unmounted the page, so verdicts that arrived while
 * the user was elsewhere were lost: the board didn't refresh and the
 * `needs_improvement` modal never pre-opened.
 *
 * This store mirrors `backgroundWorkflowsStore`: it is initialized once at the
 * app root (`+layout.svelte`) so its Tauri listeners stay attached across
 * navigations. It keeps the analyze UX page-independent by:
 *   1. tracking in-flight analyses (`analyzingCardIds`) globally;
 *   2. bumping a `boardDirtySeq` counter whenever the board should reload
 *      (the `/kanban` page reacts and reloads with its current agent filter —
 *      the filter stays on the page, the store never reloads cards itself);
 *   3. buffering the latest verdict / needs-improvement payload so the
 *      `/kanban` page can drain and surface it whenever it next mounts.
 *
 * Note: `kanban:card_ready` (scheduler promotion → workflow launch) is NOT
 * handled here. It triggers `runCardWorkflow`, which is page-coupled
 * (WorkflowExecutorService, variable interpolation, folder move) and remains
 * on `/kanban/+page.svelte`.
 *
 * @module stores/kanban-events
 */

import { writable, derived } from 'svelte/store';
import { tauriListen as listen, type TauriUnlistenFn } from '$lib/tauri';

/** Buffered `kanban:auto_analyzed` verdict awaiting display on the board. */
export interface PendingVerdict {
	cardId: string;
	verdict: string;
	reasoning: string;
}

/** Buffered `kanban:needs_improvement` verdict awaiting the improve modal. */
export interface PendingNeedsImprovement {
	cardId: string;
	reasoning: string;
	suggestedPromptEdit: string | null;
}

interface KanbanEventsState {
	/** Card ids currently being finalized by the Kanban agent. */
	analyzingCardIds: string[];
	/**
	 * Monotonic counter bumped whenever the board became stale (workflow
	 * completed, verdict applied, or stale cards purged). The page reloads its
	 * cards — with its own agent filter — on every change.
	 */
	boardDirtySeq: number;
	/** Latest approve/reject verdict to surface; cleared via `consumeVerdict`. */
	pendingVerdict: PendingVerdict | null;
	/** Latest needs_improvement verdict; cleared via `consumeNeedsImprovement`. */
	pendingNeedsImprovement: PendingNeedsImprovement | null;
}

const initialState: KanbanEventsState = {
	analyzingCardIds: [],
	boardDirtySeq: 0,
	pendingVerdict: null,
	pendingNeedsImprovement: null
};

const store = writable<KanbanEventsState>(initialState);

let unlisteners: TauriUnlistenFn[] = [];
let isInitialized = false;
/**
 * Memoized in-flight init promise. Guards against concurrent `init()` calls
 * registering duplicate listeners. Reset to null in `destroy()` and on
 * failure.
 */
let initPromise: Promise<void> | null = null;

function addAnalyzing(cardId: string): void {
	store.update((s) =>
		s.analyzingCardIds.includes(cardId)
			? s
			: { ...s, analyzingCardIds: [...s.analyzingCardIds, cardId] }
	);
}

function removeAnalyzing(cardId: string): void {
	store.update((s) => ({
		...s,
		analyzingCardIds: s.analyzingCardIds.filter((id) => id !== cardId)
	}));
}

function bumpBoardDirty(): void {
	store.update((s) => ({ ...s, boardDirtySeq: s.boardDirtySeq + 1 }));
}

export const kanbanEventsStore = {
	subscribe: store.subscribe,

	/**
	 * Register the Kanban lifecycle Tauri listeners. Safe to call multiple
	 * times: concurrent calls share the in-flight promise, and a call after a
	 * successful init resolves immediately. Call `destroy()` before re-init.
	 */
	async init(): Promise<void> {
		if (initPromise) {
			return initPromise;
		}
		if (isInitialized) {
			return;
		}

		initPromise = (async () => {
			const unlistenAnalyzing = await listen<{ card_id: string }>('kanban:analyzing', (event) => {
				const cardId = event.payload?.card_id;
				if (cardId) addAnalyzing(cardId);
			});

			const unlistenAutoAnalyzed = await listen<{
				card_id: string;
				verdict: string;
				reasoning: string;
			}>('kanban:auto_analyzed', (event) => {
				const cardId = event.payload?.card_id;
				if (!cardId) return;
				removeAnalyzing(cardId);
				store.update((s) => ({
					...s,
					boardDirtySeq: s.boardDirtySeq + 1,
					pendingVerdict: {
						cardId,
						verdict: event.payload.verdict,
						reasoning: event.payload.reasoning
					}
				}));
			});

			const unlistenNeedsImprovement = await listen<{
				card_id: string;
				reasoning: string;
				suggested_prompt_edit: string | null;
			}>('kanban:needs_improvement', (event) => {
				const cardId = event.payload?.card_id;
				if (!cardId) return;
				removeAnalyzing(cardId);
				store.update((s) => ({
					...s,
					boardDirtySeq: s.boardDirtySeq + 1,
					pendingNeedsImprovement: {
						cardId,
						reasoning: event.payload.reasoning,
						suggestedPromptEdit: event.payload.suggested_prompt_edit ?? null
					}
				}));
			});

			const unlistenComplete = await listen('workflow_complete', () => {
				bumpBoardDirty();
			});

			const unlistenPurged = await listen<{ card_ids: string[] }>('kanban:cards_purged', () => {
				bumpBoardDirty();
			});

			unlisteners = [
				unlistenAnalyzing,
				unlistenAutoAnalyzed,
				unlistenNeedsImprovement,
				unlistenComplete,
				unlistenPurged
			];
			isInitialized = true;
		})();

		try {
			await initPromise;
		} catch (e) {
			initPromise = null;
			throw e;
		}
	},

	/**
	 * Clear the buffered approve/reject verdict once the page has surfaced it.
	 * Consumers read the value reactively via the `pendingVerdict` derived
	 * store, then call this to drain it.
	 */
	clearVerdict(): void {
		store.update((s) => (s.pendingVerdict ? { ...s, pendingVerdict: null } : s));
	},

	/** Clear the buffered needs_improvement verdict once the page handled it. */
	clearNeedsImprovement(): void {
		store.update((s) => (s.pendingNeedsImprovement ? { ...s, pendingNeedsImprovement: null } : s));
	},

	/** Tear down listeners and reset state. */
	destroy(): void {
		for (const unlisten of unlisteners) {
			unlisten();
		}
		unlisteners = [];
		isInitialized = false;
		initPromise = null;
		store.set(initialState);
	}
};

/** Card ids currently being analyzed (survives page navigation). */
export const analyzingCardIds = derived(store, ($s) => $s.analyzingCardIds);

/** Bumped whenever the board should reload (workflow done / verdict / purge). */
export const boardDirtySeq = derived(store, ($s) => $s.boardDirtySeq);

/** Buffered approve/reject verdict awaiting display; `null` once drained. */
export const pendingVerdict = derived(store, ($s) => $s.pendingVerdict);

/** Buffered needs_improvement verdict awaiting the modal; `null` once drained. */
export const pendingNeedsImprovement = derived(store, ($s) => $s.pendingNeedsImprovement);
