/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * CRUD Store Factory
 *
 * Generic factory for creating CRUD stores with Tauri IPC integration.
 * Eliminates duplication between agents.ts and prompts.ts.
 *
 * @module stores/factory/createCRUDStore
 */

import { writable, derived, type Readable, type Writable } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '$lib/utils/error';

// ============================================================================
// Types
// ============================================================================

/**
 * State interface for CRUD stores
 * Generic over:
 * - TSummary: Lightweight list item type
 * - TFull: Full entity type (for editing)
 */
export interface CRUDStoreState<TSummary, TFull> {
	/** List of all items (summary view) */
	items: TSummary[];
	/** Currently selected item ID */
	selectedId: string | null;
	/** Loading state indicator */
	loading: boolean;
	/** Error message if any */
	error: string | null;
	/** Current form mode (create, edit, or null when closed) */
	formMode: 'create' | 'edit' | null;
	/** Item being edited (full config for edit mode) */
	editing: TFull | null;
}

/**
 * Configuration for CRUD store commands
 */
export interface CRUDCommands {
	/** Command to list all items (returns TSummary[]) */
	list: string;
	/** Command to get full item (returns TFull) */
	get: string;
	/** Command to create item (returns string ID) */
	create: string;
	/** Command to update item */
	update: string;
	/** Command to delete item */
	delete: string;
}

/**
 * Configuration for creating a CRUD store
 */
export interface CRUDStoreConfig {
	/** Store name for debugging/logging */
	name: string;
	/** Parameter name for ID in IPC calls (e.g., 'agentId', 'promptId') */
	idParamName: string;
	/** Tauri IPC command names */
	commands: CRUDCommands;
}

/**
 * Interface returned by createCRUDStore factory
 * Generic over:
 * - TFull: Full entity type
 * - TCreate: Creation payload type
 * - TUpdate: Update payload type
 * - TSummary: Summary/list item type
 */
export interface CRUDStore<TFull, TCreate, TUpdate, TSummary> {
	/** Subscribe to store changes */
	subscribe: Writable<CRUDStoreState<TSummary, TFull>>['subscribe'];

	/** Load all items from backend */
	loadItems(): Promise<void>;

	/** Get full item by ID */
	getItem(id: string): Promise<TFull>;

	/** Create new item */
	createItem(config: TCreate): Promise<string>;

	/** Update existing item */
	updateItem(id: string, config: TUpdate): Promise<void>;

	/** Delete item */
	deleteItem(id: string): Promise<void>;

	/** Select an item by ID (or null to deselect) */
	select(id: string | null): void;

	/** Open the create form */
	openCreateForm(): void;

	/** Open the edit form for a specific item */
	openEditForm(id: string): Promise<void>;

	/** Close the form (create or edit) */
	closeForm(): void;

	/** Clear the current error message */
	clearError(): void;

	/** Reset the store to initial state */
	reset(): void;

	/** Access to the internal store (for creating derived stores) */
	_store: Writable<CRUDStoreState<TSummary, TFull>>;
}

/**
 * Derived stores created by createDerivedStores
 */
export interface CRUDDerivedStores<TSummary, TFull> {
	/** All items (summaries) */
	items: Readable<TSummary[]>;
	/** Currently selected item (from list) */
	selected: Readable<TSummary | null>;
	/** Loading state */
	isLoading: Readable<boolean>;
	/** Error message */
	error: Readable<string | null>;
	/** Current form mode */
	formMode: Readable<'create' | 'edit' | null>;
	/** Item being edited (full data) */
	editing: Readable<TFull | null>;
	/** Item count */
	count: Readable<number>;
	/** Whether any items exist */
	hasItems: Readable<boolean>;
}

// ============================================================================
// Factory Functions
// ============================================================================

/**
 * Creates a CRUD store with Tauri IPC integration.
 *
 * @param config - Store configuration
 * @returns CRUD store with all actions
 *
 * @example
 * ```typescript
 * const agentStore = createCRUDStore<AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary>({
 *   name: 'agent',
 *   idParamName: 'agentId',
 *   commands: {
 *     list: 'list_agents',
 *     get: 'get_agent_config',
 *     create: 'create_agent',
 *     update: 'update_agent',
 *     delete: 'delete_agent'
 *   }
 * });
 * ```
 */
export function createCRUDStore<TFull, TCreate, TUpdate, TSummary extends { id: string }>(
	config: CRUDStoreConfig
): CRUDStore<TFull, TCreate, TUpdate, TSummary> {
	const { idParamName, commands } = config;

	// Initial state
	const initialState: CRUDStoreState<TSummary, TFull> = {
		items: [],
		selectedId: null,
		loading: false,
		error: null,
		formMode: null,
		editing: null
	};

	// Internal writable store
	const store = writable<CRUDStoreState<TSummary, TFull>>(initialState);

	// Create the store object
	const crudStore: CRUDStore<TFull, TCreate, TUpdate, TSummary> = {
		subscribe: store.subscribe,
		_store: store,

		async loadItems(): Promise<void> {
			store.update((s) => ({ ...s, loading: true, error: null }));
			try {
				const items = await invoke<TSummary[]>(commands.list);
				store.update((s) => ({ ...s, items, loading: false }));
			} catch (e) {
				store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			}
		},

		async getItem(id: string): Promise<TFull> {
			return await invoke<TFull>(commands.get, { [idParamName]: id });
		},

		async createItem(itemConfig: TCreate): Promise<string> {
			store.update((s) => ({ ...s, loading: true, error: null }));
			try {
				const id = await invoke<string>(commands.create, { config: itemConfig });
				await this.loadItems();
				store.update((s) => ({ ...s, formMode: null }));
				return id;
			} catch (e) {
				store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
				throw e;
			}
		},

		async updateItem(id: string, itemConfig: TUpdate): Promise<void> {
			store.update((s) => ({ ...s, loading: true, error: null }));
			try {
				await invoke(commands.update, { [idParamName]: id, config: itemConfig });
				await this.loadItems();
				store.update((s) => ({ ...s, formMode: null, editing: null }));
			} catch (e) {
				store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
				throw e;
			}
		},

		async deleteItem(id: string): Promise<void> {
			store.update((s) => ({ ...s, loading: true, error: null }));
			try {
				await invoke(commands.delete, { [idParamName]: id });
				await this.loadItems();
				store.update((s) => ({
					...s,
					selectedId: s.selectedId === id ? null : s.selectedId
				}));
			} catch (e) {
				store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
				throw e;
			}
		},

		select(id: string | null): void {
			store.update((s) => ({ ...s, selectedId: id }));
		},

		openCreateForm(): void {
			store.update((s) => ({ ...s, formMode: 'create', editing: null }));
		},

		async openEditForm(id: string): Promise<void> {
			store.update((s) => ({ ...s, loading: true }));
			try {
				const item = await this.getItem(id);
				store.update((s) => ({ ...s, formMode: 'edit', editing: item, loading: false }));
			} catch (e) {
				store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			}
		},

		closeForm(): void {
			store.update((s) => ({ ...s, formMode: null, editing: null }));
		},

		clearError(): void {
			store.update((s) => ({ ...s, error: null }));
		},

		reset(): void {
			store.set(initialState);
		}
	};

	return crudStore;
}

/**
 * Creates standard derived stores from a CRUD store.
 *
 * @param store - The base CRUD store
 * @returns Object with all derived stores
 *
 * @example
 * ```typescript
 * const baseStore = createCRUDStore<...>({...});
 * const { items, selected, isLoading, error } = createDerivedStores(baseStore);
 * ```
 */
export function createDerivedStores<TFull, TCreate, TUpdate, TSummary extends { id: string }>(
	store: CRUDStore<TFull, TCreate, TUpdate, TSummary>
): CRUDDerivedStores<TSummary, TFull> {
	const internalStore = store._store;

	return {
		items: derived(internalStore, ($s) => $s.items),
		selected: derived(internalStore, ($s) => $s.items.find((item) => item.id === $s.selectedId) ?? null),
		isLoading: derived(internalStore, ($s) => $s.loading),
		error: derived(internalStore, ($s) => $s.error),
		formMode: derived(internalStore, ($s) => $s.formMode),
		editing: derived(internalStore, ($s) => $s.editing),
		count: derived(internalStore, ($s) => $s.items.length),
		hasItems: derived(internalStore, ($s) => $s.items.length > 0)
	};
}
