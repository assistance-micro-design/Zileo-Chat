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
 * Workflow store for managing workflow state in the frontend.
 * Provides reactive state management for workflows using Svelte 5 runes pattern.
 * @module stores/workflows
 */

import type { Workflow } from '$types/workflow';
import type { StatusFilter } from '$types/sidebar';

import { writable, derived, get } from 'svelte/store';
import { getErrorMessage } from '$lib/utils/error';
import { WorkflowService } from '$lib/services/workflow.service';

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
			const workflows = await WorkflowService.loadAll();
			workflowWritable.update((s) => ({ ...s, workflows, loading: false }));
		} catch (e) {
			const error = getErrorMessage(e);
			workflowWritable.update((s) => ({ ...s, error, loading: false }));
		}
	},

	/**
	 * Create a new workflow and append it to local state without triggering a
	 * full reload. Avoids the `loading: true` flicker that users see after a
	 * fast CRUD action when the list re-fetches.
	 *
	 * @param name - Workflow name
	 * @param agentId - Agent ID to associate with the workflow
	 * @returns ID of the created workflow
	 */
	async createWorkflow(name: string, agentId: string): Promise<string> {
		const id = await WorkflowService.create(name, agentId);
		try {
			const created = await WorkflowService.getFullState(id);
			workflowWritable.update((s) => ({
				...s,
				workflows: [...s.workflows, created.workflow]
			}));
		} catch {
			// If we cannot fetch the fresh entity, fall back to a full reload so
			// the list stays consistent with the backend.
			await workflowStore.loadWorkflows();
		}
		return id;
	},

	/**
	 * Rename a workflow in-place without triggering a full reload.
	 *
	 * @param workflowId - Workflow ID to rename
	 * @param name - New workflow name
	 * @returns Updated workflow entity
	 * @throws The original error after setting `error` state.
	 */
	async renameWorkflow(workflowId: string, name: string): Promise<Workflow> {
		workflowWritable.update((s) => ({ ...s, error: null }));
		try {
			const updated = await WorkflowService.rename(workflowId, name);
			workflowWritable.update((s) => ({
				...s,
				workflows: s.workflows.map((w) => (w.id === updated.id ? updated : w))
			}));
			return updated;
		} catch (e) {
			workflowWritable.update((s) => ({ ...s, error: getErrorMessage(e) }));
			throw e;
		}
	},

	/**
	 * Delete a workflow and remove it from local state without triggering a
	 * full reload.
	 *
	 * @param workflowId - Workflow ID to delete
	 * @throws The original error after setting `error` state.
	 */
	async deleteWorkflow(workflowId: string): Promise<void> {
		workflowWritable.update((s) => ({ ...s, error: null }));
		try {
			await WorkflowService.delete(workflowId);
			workflowWritable.update((s) => ({
				...s,
				workflows: s.workflows.filter((w) => w.id !== workflowId),
				selectedId: s.selectedId === workflowId ? null : s.selectedId
			}));
		} catch (e) {
			workflowWritable.update((s) => ({ ...s, error: getErrorMessage(e) }));
			throw e;
		}
	},

	/**
	 * Clear `folder_id` locally for every workflow that belonged to a deleted
	 * folder, matching the backend's cascade behaviour without a full reload.
	 *
	 * @param folderId - Folder ID that was deleted
	 */
	detachFromFolder(folderId: string): void {
		workflowWritable.update((s) => ({
			...s,
			workflows: s.workflows.map((w) =>
				w.folder_id === folderId ? { ...w, folder_id: undefined } : w
			)
		}));
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
	 * @throws The original error after setting `error` state.
	 */
	async deleteBatch(ids: string[]): Promise<{ deleted: number; skipped_running: string[] }> {
		workflowWritable.update((s) => ({ ...s, error: null }));
		try {
			const result = await WorkflowService.deleteBatch(ids);
			workflowWritable.update((s) => ({
				...s,
				workflows: s.workflows.filter(
					(w) => !ids.includes(w.id) || result.skipped_running.includes(w.id)
				),
				selectedId: ids.includes(s.selectedId ?? '') ? null : s.selectedId
			}));
			return result;
		} catch (e) {
			workflowWritable.update((s) => ({ ...s, error: getErrorMessage(e) }));
			throw e;
		}
	},

	/**
	 * Move a single workflow to a folder (or remove from folder).
	 *
	 * @param workflowId - The workflow ID to move
	 * @param folderId - Target folder ID, or null to remove from folder
	 * @throws The original error after setting `error` state.
	 */
	async moveToFolder(workflowId: string, folderId: string | null): Promise<void> {
		workflowWritable.update((s) => ({ ...s, error: null }));
		try {
			const updated = await WorkflowService.moveToFolder(workflowId, folderId);
			workflowWritable.update((s) => ({
				...s,
				workflows: s.workflows.map((w) => (w.id === updated.id ? updated : w))
			}));
		} catch (e) {
			workflowWritable.update((s) => ({ ...s, error: getErrorMessage(e) }));
			throw e;
		}
	},

	/**
	 * Move multiple workflows to a folder (or remove from folder).
	 *
	 * @param workflowIds - Array of workflow IDs to move
	 * @param folderId - Target folder ID, or null to remove from folder
	 * @returns Number of workflows moved
	 * @throws The original error after setting `error` state.
	 */
	async moveBatchToFolder(workflowIds: string[], folderId: string | null): Promise<number> {
		workflowWritable.update((s) => ({ ...s, error: null }));
		try {
			const moved = await WorkflowService.moveBatchToFolder(workflowIds, folderId);
			workflowWritable.update((s) => ({
				...s,
				workflows: s.workflows.map((w) =>
					workflowIds.includes(w.id) ? { ...w, folder_id: folderId ?? undefined } : w
				)
			}));
			return moved;
		} catch (e) {
			workflowWritable.update((s) => ({ ...s, error: getErrorMessage(e) }));
			throw e;
		}
	},

	/**
	 * Toggle the pinned state of a workflow.
	 *
	 * @param workflowId - The workflow ID to toggle
	 * @throws The original error after setting `error` state.
	 */
	async togglePinned(workflowId: string): Promise<void> {
		workflowWritable.update((s) => ({ ...s, error: null }));
		try {
			const updated = await WorkflowService.togglePinned(workflowId);
			workflowWritable.update((s) => ({
				...s,
				workflows: s.workflows.map((w) => (w.id === updated.id ? updated : w))
			}));
		} catch (e) {
			workflowWritable.update((s) => ({ ...s, error: getErrorMessage(e) }));
			throw e;
		}
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
