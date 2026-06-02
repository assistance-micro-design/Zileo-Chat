/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * Global "new task" modal controller.
 *
 * The `KanbanCardCreator` modal is mounted ONCE at the app root
 * (`+layout.svelte`) so the "Nouvelle tâche à faire" button in the global
 * `FloatingMenu` can open it from any route (DP-5: a single instance avoids the
 * divergent-form-state regression of mounting it on both the page and the root).
 * This store is the thin open/close channel between the nav button and the
 * root-mounted modal.
 *
 * @module stores/card-creator
 */

import { writable, derived } from 'svelte/store';

interface CardCreatorState {
	/** Whether the global creator modal is currently open. */
	open: boolean;
}

const store = writable<CardCreatorState>({ open: false });

/**
 * Controller for the global "new task" creator modal.
 */
export const cardCreatorStore = {
	subscribe: store.subscribe,

	/** Open the global card creator modal. */
	open(): void {
		store.set({ open: true });
	},

	/** Close the global card creator modal. */
	close(): void {
		store.set({ open: false });
	}
};

/** Reactive open flag for the root-mounted modal host. */
export const cardCreatorOpen = derived(store, ($s) => $s.open);
