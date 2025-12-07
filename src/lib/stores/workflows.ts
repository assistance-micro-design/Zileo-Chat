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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Workflow store for managing workflow state in the frontend.
 * Provides reactive state management for workflows using Svelte 5 runes pattern.
 * @module stores/workflows
 */

import type { Workflow } from '$types/workflow';

import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

/**
 * State interface for the reactive workflow store
 */
export interface WorkflowState {
	/** List of all workflows */
	workflows: Workflow[];
	/** Currently selected workflow ID */
	selectedId: string | null;
	/** Loading state indicator */
	loading: boolean;
	/** Error message if any */
	error: string | null;
	/** Search filter text */
	searchFilter: string;
}

/**
 * Initial state for the reactive workflow store
 */
const initialStoreState: WorkflowState = {
	workflows: [],
	selectedId: null,
	loading: false,
	error: null,
	searchFilter: ''
};

/**
 * Internal writable store
 */
const workflowWritable = writable<WorkflowState>(initialStoreState);

/**
 * Reactive workflow store with actions for CRUD operations.
 * Provides reactive state management for workflows using Svelte stores.
 */
export const workflowStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: workflowWritable.subscribe,

	/**
	 * Load all workflows from backend.
	 */
	async loadWorkflows(): Promise<void> {
		workflowWritable.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const workflows = await invoke<Workflow[]>('load_workflows');
			workflowWritable.update((s) => ({ ...s, workflows, loading: false }));
		} catch (e) {
			const error = e instanceof Error ? e.message : String(e);
			workflowWritable.update((s) => ({ ...s, error, loading: false }));
		}
	},

	/**
	 * Select a workflow by ID.
	 *
	 * @param workflowId - ID to select (or null to deselect)
	 */
	select(workflowId: string | null): void {
		workflowWritable.update((s) => ({ ...s, selectedId: workflowId }));
	},

	/**
	 * Set the search filter text.
	 *
	 * @param filter - Search filter string
	 */
	setSearchFilter(filter: string): void {
		workflowWritable.update((s) => ({ ...s, searchFilter: filter }));
	},

	/**
	 * Get the currently selected workflow (synchronous).
	 *
	 * @returns Selected workflow or undefined
	 */
	getSelected(): Workflow | undefined {
		const state = get(workflowWritable);
		return state.workflows.find((w) => w.id === state.selectedId);
	},

	/**
	 * Reset store to initial state.
	 */
	reset(): void {
		workflowWritable.set(initialStoreState);
	}
};

/**
 * Derived store: list of all workflows
 */
export const workflows = derived(workflowWritable, ($s) => $s.workflows);

/**
 * Derived store: currently selected workflow ID
 */
export const selectedWorkflowId = derived(workflowWritable, ($s) => $s.selectedId);

/**
 * Derived store: loading state
 */
export const workflowsLoading = derived(workflowWritable, ($s) => $s.loading);

/**
 * Derived store: error message
 */
export const workflowsError = derived(workflowWritable, ($s) => $s.error);

/**
 * Derived store: search filter text
 */
export const workflowSearchFilter = derived(workflowWritable, ($s) => $s.searchFilter);

/**
 * Derived store: currently selected workflow
 */
export const selectedWorkflow = derived(
	workflowWritable,
	($s) => $s.workflows.find((w) => w.id === $s.selectedId) ?? null
);

/**
 * Derived store: workflows filtered by search text
 */
export const filteredWorkflows = derived(workflowWritable, ($s) => {
	if (!$s.searchFilter) return $s.workflows;
	const filter = $s.searchFilter.toLowerCase();
	return $s.workflows.filter((w) => w.name.toLowerCase().includes(filter));
});
