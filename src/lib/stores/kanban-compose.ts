/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * @fileoverview Async card-compose tracking store.
 *
 * Mirrors the backend detached `start_compose_card` flow: the frontend
 * `register`s a compose right after launching it, then the root-mounted Tauri
 * listeners (`kanban:compose_ready` / `kanban:compose_failed`) resolve it with a
 * toast and signal the board to reload the proposed-cards zone.
 *
 * Only IN-FLIGHT composes live in the map (the terminal outcome surfaces as a
 * toast + a DB-backed `proposed` card in the validation zone), so `composingCount`
 * stays an accurate mirror of the backend slot registry for the UI cap hint. A
 * stale-entry sweep guards against a frontend leak if a backend task ever dies
 * without emitting (the real cap is freed backend-side by the RAII guard).
 *
 * Root-mounted so its listeners survive
 * navigation away from /kanban — a compose launched there still toasts and
 * refreshes the board if it finishes while the user is elsewhere.
 *
 * @module stores/kanban-compose
 */

import { writable, derived } from 'svelte/store';
import { tauriListen as listen, type TauriUnlistenFn as UnlistenFn } from '$lib/tauri';
import { toastStore } from './toast';
import { t } from '$lib/i18n';

/**
 * Global concurrent-compose cap — mirrors the backend `MAX_CONCURRENT_COMPOSE`.
 * The backend gate is authoritative; this only drives the advisory UI hint.
 */
export const MAX_CONCURRENT_COMPOSE = 4;

/** How often the stale-entry sweep runs. */
const CLEANUP_INTERVAL_MS = 60_000;

/**
 * Age past which an in-flight entry is force-dropped. Set above the backend
 * `COMPOSE_TIMEOUT_SECS` (300s) so a normal timeout (which DOES emit
 * `compose_failed`) clears the entry first; this only catches a task that died
 * without emitting at all.
 */
const STALE_COMPOSE_MS = 360_000;

interface ComposeReadyEvent {
	card_id: string;
	title: string;
}

interface ComposeFailedEvent {
	card_id: string;
	error: string;
}

/** A compose currently in flight (no terminal entries are kept). */
interface ComposeEntry {
	cardId: string;
	/** Short hint (first chars of the description) for the in-progress UI. */
	titleHint: string;
	startedAt: number;
}

interface ComposeState {
	composing: Map<string, ComposeEntry>;
	/** Bumped on each `compose_ready` so /kanban can reload the proposed zone. */
	proposedDirtySeq: number;
}

const store = writable<ComposeState>({ composing: new Map(), proposedDirtySeq: 0 });

let unlisteners: UnlistenFn[] = [];
let initPromise: Promise<void> | null = null;
let initialized = false;
let cleanupTimer: ReturnType<typeof setInterval> | null = null;

function removeEntry(cardId: string): void {
	store.update((s) => {
		if (!s.composing.has(cardId)) return s;
		const composing = new Map(s.composing);
		composing.delete(cardId);
		return { ...s, composing };
	});
}

/**
 * A compose finished successfully — drop the in-flight entry, bump the dirty
 * counter so the board reloads the proposed zone, and toast (m4: fire even when
 * no prior `register` was seen, e.g. the event raced the register).
 */
function handleReady(payload: ComposeReadyEvent): void {
	removeEntry(payload.card_id);
	store.update((s) => ({ ...s, proposedDirtySeq: s.proposedDirtySeq + 1 }));
	toastStore.add({
		type: 'success',
		title: t('toast_compose_ready'),
		message: payload.title,
		persistent: false,
		duration: 5000
	});
}

/** A compose failed — drop the in-flight entry and toast the cleaned error. */
function handleFailed(payload: ComposeFailedEvent): void {
	removeEntry(payload.card_id);
	toastStore.add({
		type: 'error',
		title: t('toast_compose_failed'),
		message: payload.error,
		persistent: false,
		duration: 8000
	});
}

function sweepStale(): void {
	const cutoff = Date.now() - STALE_COMPOSE_MS;
	store.update((s) => {
		let changed = false;
		const composing = new Map(s.composing);
		for (const [id, entry] of composing) {
			if (entry.startedAt < cutoff) {
				composing.delete(id);
				changed = true;
			}
		}
		return changed ? { ...s, composing } : s;
	});
}

/**
 * Async card-compose tracking store.
 */
export const composingStore = {
	subscribe: store.subscribe,

	/**
	 * Register the root Tauri listeners + the stale sweep. Idempotent: concurrent
	 * calls share one in-flight promise (no duplicate listeners); a call after a
	 * successful init resolves immediately. Call `destroy()` to re-initialise.
	 */
	async init(): Promise<void> {
		if (initPromise) return initPromise;
		if (initialized) return;
		initPromise = (async () => {
			const unlistenReady = await listen<ComposeReadyEvent>('kanban:compose_ready', (event) =>
				handleReady(event.payload)
			);
			const unlistenFailed = await listen<ComposeFailedEvent>('kanban:compose_failed', (event) =>
				handleFailed(event.payload)
			);
			unlisteners = [unlistenReady, unlistenFailed];
			cleanupTimer = setInterval(sweepStale, CLEANUP_INTERVAL_MS);
			initialized = true;
		})();
		try {
			await initPromise;
		} catch (e) {
			initPromise = null;
			throw e;
		}
	},

	/**
	 * Track a freshly launched compose. Called right after `start_compose_card`
	 * returns its `card_id`. Safe to call before/after the event arrives.
	 */
	register(cardId: string, titleHint: string): void {
		store.update((s) => {
			const composing = new Map(s.composing);
			composing.set(cardId, { cardId, titleHint, startedAt: Date.now() });
			return { ...s, composing };
		});
	},

	/** Tear down listeners, timer and state. */
	destroy(): void {
		for (const unlisten of unlisteners) unlisten();
		unlisteners = [];
		if (cleanupTimer) {
			clearInterval(cleanupTimer);
			cleanupTimer = null;
		}
		initialized = false;
		initPromise = null;
		store.set({ composing: new Map(), proposedDirtySeq: 0 });
	}
};

/** In-flight composes (for the "génération en cours" UI). */
export const composingEntries = derived(store, ($s) => Array.from($s.composing.values()));

/** Number of in-flight composes (advisory cap hint). */
export const composingCount = derived(store, ($s) => $s.composing.size);

/** Whether a new compose may be started (advisory; backend is authoritative). */
export const canStartCompose = derived(store, ($s) => $s.composing.size < MAX_CONCURRENT_COMPOSE);

/** Monotonic counter bumped on each `compose_ready` (board-reload signal). */
export const proposedDirtySeq = derived(store, ($s) => $s.proposedDirtySeq);
