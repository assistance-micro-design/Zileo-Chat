/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * Kanban schedule store — CRUD for recurring card templates.
 *
 * @module stores/kanban-schedule
 */

import { tauriInvoke as invoke } from '$lib/tauri';
import { createCRUDStore, createDerivedStores } from './factory/createCRUDStore';
import { getErrorMessage } from '$lib/utils/error';
import type { KanbanSchedule, KanbanScheduleCreate, KanbanScheduleUpdate } from '$types/kanban';

const baseStore = createCRUDStore<
	KanbanSchedule,
	KanbanScheduleCreate,
	KanbanScheduleUpdate,
	KanbanSchedule
>({
	name: 'kanban-schedule',
	idParamName: 'id',
	commands: {
		list: 'list_kanban_schedules',
		get: 'get_kanban_schedule',
		create: 'create_kanban_schedule',
		update: 'update_kanban_schedule',
		delete: 'delete_kanban_schedule'
	}
});

/**
 * Kanban schedule store. `get_kanban_schedule` is intentionally absent backend-side;
 * consumers should call `loadSchedules` then filter the in-memory list by `card_template_id`.
 */
export const kanbanScheduleStore = {
	subscribe: baseStore.subscribe,

	async loadSchedules(): Promise<void> {
		baseStore._store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const items = await invoke<KanbanSchedule[]>('list_kanban_schedules');
			baseStore._store.update((s) => ({ ...s, items, loading: false }));
		} catch (e) {
			baseStore._store.update((s) => ({
				...s,
				error: getErrorMessage(e),
				loading: false
			}));
		}
	},

	createSchedule: (data: KanbanScheduleCreate) => baseStore.createItem(data),
	updateSchedule: (id: string, data: KanbanScheduleUpdate) => baseStore.updateItem(id, data),
	deleteSchedule: (id: string) => baseStore.deleteItem(id),

	clearError: () => baseStore.clearError(),
	reset: () => baseStore.reset()
};

const derivedStores = createDerivedStores(baseStore);

/** All kanban schedules. */
export const kanbanSchedules = derivedStores.items;

/** Loading state. */
export const kanbanSchedulesLoading = derivedStores.isLoading;

/** Error state. */
export const kanbanSchedulesError = derivedStores.error;
