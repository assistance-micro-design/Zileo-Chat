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
import type { StatusFilter } from '$types/sidebar';

import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '$lib/utils/error';

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
	/** Active status filter */
	statusFilter: StatusFilter;
}

/**
 * Initial state for the reactive workflow store
 */
const initialStoreState: WorkflowState = {
	workflows: [],
	selectedId: null,
	loading: false,
	error: null,
	searchFilter: '',
	statusFilter: 'all'
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
			const error = getErrorMessage(e);
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
	 * Set the active status filter.
	 *
	 * @param filter - Status filter value
	 */
	setStatusFilter(filter: StatusFilter): void {
		workflowWritable.update((s) => ({ ...s, statusFilter: filter }));
	},

	/**
	 * Batch delete multiple workflows.
	 * Calls the backend command and removes deleted workflows from state.
	 *
	 * @param ids - Array of workflow IDs to delete
	 * @returns Result with deleted count and skipped running IDs
	 */
	async deleteBatch(ids: string[]): Promise<{ deleted: number; skipped_running: string[] }> {
		const result = await invoke<{ deleted: number; skipped_running: string[] }>(
			'delete_workflows_batch',
			{ workflowIds: ids }
		);
		workflowWritable.update((s) => ({
			...s,
			workflows: s.workflows.filter((w) => !ids.includes(w.id) || result.skipped_running.includes(w.id)),
			selectedId: ids.includes(s.selectedId ?? '') ? null : s.selectedId
		}));
		return result;
	},

	/**
	 * Move a single workflow to a folder (or remove from folder).
	 *
	 * @param workflowId - The workflow ID to move
	 * @param folderId - Target folder ID, or null to remove from folder
	 */
	async moveToFolder(workflowId: string, folderId: string | null): Promise<void> {
		const updated = await invoke<Workflow>('move_workflow_to_folder', {
			workflowId,
			folderId
		});
		workflowWritable.update((s) => ({
			...s,
			workflows: s.workflows.map((w) => (w.id === updated.id ? updated : w))
		}));
	},

	/**
	 * Move multiple workflows to a folder (or remove from folder).
	 *
	 * @param workflowIds - Array of workflow IDs to move
	 * @param folderId - Target folder ID, or null to remove from folder
	 * @returns Number of workflows moved
	 */
	async moveBatchToFolder(workflowIds: string[], folderId: string | null): Promise<number> {
		const moved = await invoke<number>('move_workflows_to_folder', {
			workflowIds,
			folderId
		});
		workflowWritable.update((s) => ({
			...s,
			workflows: s.workflows.map((w) =>
				workflowIds.includes(w.id) ? { ...w, folder_id: folderId ?? undefined } : w
			)
		}));
		return moved;
	},

	/**
	 * Toggle the pinned state of a workflow.
	 *
	 * @param workflowId - The workflow ID to toggle
	 */
	async togglePinned(workflowId: string): Promise<void> {
		const updated = await invoke<Workflow>('toggle_workflow_pinned', { workflowId });
		workflowWritable.update((s) => ({
			...s,
			workflows: s.workflows.map((w) => (w.id === updated.id ? updated : w))
		}));
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
 * Derived store: active status filter
 */
export const statusFilter = derived(workflowWritable, ($s) => $s.statusFilter);

/**
 * Derived store: workflow count per status (includes 'all')
 */
export const statusCounts = derived(workflowWritable, ($s) => {
	const counts: Record<StatusFilter, number> = {
		all: $s.workflows.length,
		idle: 0,
		running: 0,
		completed: 0,
		error: 0
	};
	for (const w of $s.workflows) {
		counts[w.status]++;
	}
	return counts;
});

/**
 * Derived store: pinned workflows
 */
export const pinnedWorkflows = derived(workflowWritable, ($s) =>
	$s.workflows.filter((w) => w.pinned)
);

/**
 * Derived store: workflows filtered by search text and status filter
 */
export const filteredWorkflows = derived(workflowWritable, ($s) => {
	let result = $s.workflows;

	if ($s.statusFilter !== 'all') {
		result = result.filter((w) => w.status === $s.statusFilter);
	}

	if ($s.searchFilter) {
		const filter = $s.searchFilter.toLowerCase();
		result = result.filter((w) => w.name.toLowerCase().includes(filter));
	}

	return result;
});
