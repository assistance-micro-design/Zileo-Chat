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
 * Pattern follows src/lib/stores/agents.ts
 */

import { writable, derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type {
	Prompt,
	PromptCreate,
	PromptUpdate,
	PromptSummary,
	PromptStoreState,
	PromptCategory
} from '$types/prompt';

// ===== Initial State =====

const initialState: PromptStoreState = {
	prompts: [],
	selectedId: null,
	loading: false,
	error: null,
	formMode: null,
	editingPrompt: null
};

// ===== Store =====

const store = writable<PromptStoreState>(initialState);

export const promptStore = {
	subscribe: store.subscribe,

	// ===== CRUD Operations =====

	/**
	 * Load all prompts from backend
	 */
	async loadPrompts(): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const prompts = await invoke<PromptSummary[]>('list_prompts');
			store.update((s) => ({ ...s, prompts, loading: false }));
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
		}
	},

	/**
	 * Get full prompt by ID
	 */
	async getPrompt(id: string): Promise<Prompt> {
		return await invoke<Prompt>('get_prompt', { promptId: id });
	},

	/**
	 * Create a new prompt
	 */
	async createPrompt(config: PromptCreate): Promise<string> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const id = await invoke<string>('create_prompt', { config });
			await this.loadPrompts();
			store.update((s) => ({ ...s, formMode: null, loading: false }));
			return id;
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Update an existing prompt
	 */
	async updatePrompt(id: string, updates: PromptUpdate): Promise<Prompt> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const updated = await invoke<Prompt>('update_prompt', {
				promptId: id,
				config: updates
			});
			await this.loadPrompts();
			store.update((s) => ({
				...s,
				formMode: null,
				editingPrompt: null,
				loading: false
			}));
			return updated;
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Delete a prompt
	 */
	async deletePrompt(id: string): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			await invoke('delete_prompt', { promptId: id });
			await this.loadPrompts();
			store.update((s) => ({
				...s,
				loading: false,
				selectedId: s.selectedId === id ? null : s.selectedId
			}));
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Search prompts by query and/or category
	 */
	async searchPrompts(query?: string, category?: PromptCategory): Promise<PromptSummary[]> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const prompts = await invoke<PromptSummary[]>('search_prompts', {
				query: query || null,
				category: category || null
			});
			store.update((s) => ({ ...s, prompts, loading: false }));
			return prompts;
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	// ===== UI State Management =====

	/**
	 * Select a prompt by ID
	 */
	select(promptId: string | null): void {
		store.update((s) => ({ ...s, selectedId: promptId }));
	},

	/**
	 * Open the create form
	 */
	openCreateForm(): void {
		store.update((s) => ({ ...s, formMode: 'create', editingPrompt: null }));
	},

	/**
	 * Open the edit form for a specific prompt
	 */
	async openEditForm(id: string): Promise<void> {
		store.update((s) => ({ ...s, loading: true }));
		try {
			const prompt = await this.getPrompt(id);
			store.update((s) => ({
				...s,
				formMode: 'edit',
				editingPrompt: prompt,
				loading: false
			}));
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
		}
	},

	/**
	 * Close the form (create or edit)
	 */
	closeForm(): void {
		store.update((s) => ({ ...s, formMode: null, editingPrompt: null }));
	},

	/**
	 * Clear error state
	 */
	clearError(): void {
		store.update((s) => ({ ...s, error: null }));
	},

	/**
	 * Reset store to initial state
	 */
	reset(): void {
		store.set(initialState);
	}
};

// ===== Derived Stores =====

/** All prompts (summaries) */
export const prompts = derived(store, (s) => s.prompts);

/** Currently selected prompt ID */
export const selectedPromptId = derived(store, (s) => s.selectedId);

/** Currently selected prompt (from list) */
export const selectedPrompt = derived(store, (s) =>
	s.prompts.find((p) => p.id === s.selectedId) ?? null
);

/** Prompt loading state */
export const promptLoading = derived(store, (s) => s.loading);

/** Prompt error state */
export const promptError = derived(store, (s) => s.error);

/** Prompt form mode */
export const promptFormMode = derived(store, (s) => s.formMode);

/** Prompt being edited (full data) */
export const editingPrompt = derived(store, (s) => s.editingPrompt);

/** Whether any prompts exist */
export const hasPrompts = derived(store, (s) => s.prompts.length > 0);

/** Prompt count */
export const promptCount = derived(store, (s) => s.prompts.length);

// ===== Utility Functions (Frontend-only) =====

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
export function interpolateVariables(
	content: string,
	values: Record<string, string>
): string {
	return content.replace(
		/\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}/g,
		(match, name) => values[name] ?? match
	);
}

/**
 * Check if all required variables have values
 */
export function getMissingVariables(
	content: string,
	values: Record<string, string>
): string[] {
	const required = extractVariables(content);
	return required.filter((name) => !values[name] || values[name].trim() === '');
}
