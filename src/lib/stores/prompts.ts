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
 * Prompt Library Store
 *
 * Manages prompt state with Tauri IPC integration.
 * Uses the CRUD store factory for standardized state management.
 *
 * @module stores/prompts
 */

import { derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import {
	createCRUDStore,
	createDerivedStores
} from './factory/createCRUDStore';
import { getErrorMessage } from '$lib/utils/error';
import type {
	Prompt,
	PromptCreate,
	PromptUpdate,
	PromptSummary,
	PromptStoreState,
	PromptCategory
} from '$types/prompt';

// ============================================================================
// Base CRUD Store
// ============================================================================

const baseCrudStore = createCRUDStore<Prompt, PromptCreate, PromptUpdate, PromptSummary>({
	name: 'prompt',
	idParamName: 'promptId',
	commands: {
		list: 'list_prompts',
		get: 'get_prompt',
		create: 'create_prompt',
		update: 'update_prompt',
		delete: 'delete_prompt'
	}
});

// ============================================================================
// Prompt Store (with backward-compatible API + search extension)
// ============================================================================

/**
 * Prompt store with actions for CRUD operations, search, and UI state management.
 * Extends the base CRUD store with prompt-specific search functionality.
 */
export const promptStore = {
	/**
	 * Subscribe to store changes
	 * Maps internal state to PromptStoreState interface
	 */
	subscribe: (run: (value: PromptStoreState) => void) => {
		return baseCrudStore.subscribe((state) => {
			run({
				prompts: state.items,
				selectedId: state.selectedId,
				loading: state.loading,
				error: state.error,
				formMode: state.formMode,
				editingPrompt: state.editing
			});
		});
	},

	// ===== CRUD Operations =====

	/**
	 * Load all prompts from backend
	 */
	loadPrompts: () => baseCrudStore.loadItems(),

	/**
	 * Get full prompt by ID
	 */
	getPrompt: (id: string) => baseCrudStore.getItem(id),

	/**
	 * Create a new prompt
	 */
	createPrompt: (config: PromptCreate) => baseCrudStore.createItem(config),

	/**
	 * Update an existing prompt
	 */
	async updatePrompt(id: string, updates: PromptUpdate): Promise<Prompt> {
		// Store state before update
		baseCrudStore._store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const updated = await invoke<Prompt>('update_prompt', {
				promptId: id,
				config: updates
			});
			await baseCrudStore.loadItems();
			baseCrudStore._store.update((s) => ({
				...s,
				formMode: null,
				editing: null,
				loading: false
			}));
			return updated;
		} catch (e) {
			baseCrudStore._store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Delete a prompt
	 */
	deletePrompt: (id: string) => baseCrudStore.deleteItem(id),

	/**
	 * Search prompts by query and/or category
	 * (Prompt-specific extension, not part of base CRUD)
	 */
	async searchPrompts(query?: string, category?: PromptCategory): Promise<PromptSummary[]> {
		baseCrudStore._store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const prompts = await invoke<PromptSummary[]>('search_prompts', {
				query: query || null,
				category: category || null
			});
			baseCrudStore._store.update((s) => ({ ...s, items: prompts, loading: false }));
			return prompts;
		} catch (e) {
			baseCrudStore._store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	// ===== UI State Management =====

	/**
	 * Select a prompt by ID
	 */
	select: (promptId: string | null) => baseCrudStore.select(promptId),

	/**
	 * Open the create form
	 */
	openCreateForm: () => baseCrudStore.openCreateForm(),

	/**
	 * Open the edit form for a specific prompt
	 */
	openEditForm: (id: string) => baseCrudStore.openEditForm(id),

	/**
	 * Close the form (create or edit)
	 */
	closeForm: () => baseCrudStore.closeForm(),

	/**
	 * Clear error state
	 */
	clearError: () => baseCrudStore.clearError(),

	/**
	 * Reset store to initial state
	 */
	reset: () => baseCrudStore.reset()
};

// ============================================================================
// Derived Stores
// ============================================================================

// Get base derived stores
const derivedStores = createDerivedStores(baseCrudStore);

/** All prompts (summaries) */
export const prompts = derivedStores.items;

/** Currently selected prompt ID */
export const selectedPromptId = derived(baseCrudStore._store, (s) => s.selectedId);

/** Currently selected prompt (from list) */
export const selectedPrompt = derivedStores.selected;

/** Prompt loading state */
export const promptLoading = derivedStores.isLoading;

/** Prompt error state */
export const promptError = derivedStores.error;

/** Prompt form mode */
export const promptFormMode = derivedStores.formMode;

/** Prompt being edited (full data) */
export const editingPrompt = derivedStores.editing;

/** Whether any prompts exist */
export const hasPrompts = derivedStores.hasItems;

/** Prompt count */
export const promptCount = derivedStores.count;

// ============================================================================
// Utility Functions (Frontend-only)
// ============================================================================

/**
 * Extract variables from content (frontend version)
 * Mirrors backend Prompt::detect_variables
 */
export function extractVariables(content: string): string[] {
	const pattern = /\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g;
	const seen = new Set<string>();
	const variables: string[] = [];

	let match;
	while ((match = pattern.exec(content)) !== null) {
		const name = match[1];
		if (!seen.has(name)) {
			seen.add(name);
			variables.push(name);
		}
	}

	return variables;
}

/**
 * Interpolate variables into content (frontend version)
 * Mirrors backend Prompt::interpolate
 */
export function interpolateVariables(content: string, values: Record<string, string>): string {
	return content.replace(/\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g, (match, name) => values[name] ?? match);
}

/**
 * Check if all required variables have values
 */
export function getMissingVariables(content: string, values: Record<string, string>): string[] {
	const required = extractVariables(content);
	return required.filter((name) => !values[name] || values[name].trim() === '');
}
