/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * Shared read model for the GLOBAL Kanban supervisor agents
 * (`settings:kanban.composeAgentId` / `analyzeAgentId`).
 *
 * Two surfaces need these ids reactively: the root-mounted card creator (for the
 * compose/analyze nudges + the D7 pre-selected compose select) and the /kanban
 * board (for the "configured analyze agent no longer exists" banner). A single
 * store keeps them in sync — loaded when the creator opens and when /kanban
 * mounts, and refreshed in place when the settings page persists a change — so
 * the derived notices/banner resolve themselves as soon as the configuration is
 * fixed.
 *
 * @module stores/kanban-settings
 */

import { writable, derived } from 'svelte/store';
import { tauriInvoke as invoke } from '$lib/tauri';
import type { KanbanSettings } from '$types/kanban-settings';

interface KanbanSupervisorState {
	/** Configured compose supervisor id, or `null` when unset. */
	composeAgentId: string | null;
	/** Configured analyze supervisor id, or `null` when unset. */
	analyzeAgentId: string | null;
}

const store = writable<KanbanSupervisorState>({
	composeAgentId: null,
	analyzeAgentId: null
});

/** Normalizes a persisted settings object into the store shape. */
function toState(settings: KanbanSettings): KanbanSupervisorState {
	return {
		composeAgentId: settings.composeAgentId ?? null,
		analyzeAgentId: settings.analyzeAgentId ?? null
	};
}

/**
 * Controller for the shared Kanban supervisor ids.
 */
export const kanbanSupervisorStore = {
	subscribe: store.subscribe,

	/**
	 * Loads the configured supervisor ids from the backend. Best-effort: a
	 * failed load keeps the last known values (the nudges are advisory, never
	 * blocking).
	 */
	async load(): Promise<void> {
		try {
			const settings = await invoke<KanbanSettings>('get_kanban_settings');
			store.set(toState(settings));
		} catch {
			// Advisory only — keep the last known ids on a transient failure.
		}
	},

	/** Reflects a freshly persisted settings object (e.g. after a save). */
	setFromSettings(settings: KanbanSettings): void {
		store.set(toState(settings));
	}
};

/** Configured compose supervisor id (`null` when unset). */
export const composeSupervisorId = derived(store, ($s) => $s.composeAgentId);

/** Configured analyze supervisor id (`null` when unset). */
export const analyzeSupervisorId = derived(store, ($s) => $s.analyzeAgentId);
