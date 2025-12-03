// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Workflow store for managing workflow state in the frontend.
 * Provides reactive state management for workflows using Svelte 5 runes pattern.
 * @module stores/workflows
 */

import type { Workflow, WorkflowResult, WorkflowStatus } from '$types/workflow';

/**
 * State interface for the workflow store
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
	/** Last execution result */
	lastResult: WorkflowResult | null;
}

/**
 * Creates the initial workflow state
 * @returns Initial workflow state with empty values
 */
export function createInitialState(): WorkflowState {
	return {
		workflows: [],
		selectedId: null,
		loading: false,
		error: null,
		lastResult: null
	};
}

/**
 * Adds a workflow to the state
 * @param state - Current workflow state
 * @param workflow - Workflow to add
 * @returns Updated state with new workflow
 */
export function addWorkflow(state: WorkflowState, workflow: Workflow): WorkflowState {
	return {
		...state,
		workflows: [...state.workflows, workflow],
		error: null
	};
}

/**
 * Updates an existing workflow in the state
 * @param state - Current workflow state
 * @param id - ID of workflow to update
 * @param updates - Partial workflow updates
 * @returns Updated state with modified workflow
 */
export function updateWorkflow(
	state: WorkflowState,
	id: string,
	updates: Partial<Workflow>
): WorkflowState {
	return {
		...state,
		workflows: state.workflows.map((w) => (w.id === id ? { ...w, ...updates } : w)),
		error: null
	};
}

/**
 * Removes a workflow from the state
 * @param state - Current workflow state
 * @param id - ID of workflow to remove
 * @returns Updated state without the removed workflow
 */
export function removeWorkflow(state: WorkflowState, id: string): WorkflowState {
	const newWorkflows = state.workflows.filter((w) => w.id !== id);
	const newSelectedId = state.selectedId === id ? null : state.selectedId;

	return {
		...state,
		workflows: newWorkflows,
		selectedId: newSelectedId,
		error: null
	};
}

/**
 * Sets the selected workflow ID
 * @param state - Current workflow state
 * @param id - ID of workflow to select (or null to deselect)
 * @returns Updated state with new selection
 */
export function selectWorkflow(state: WorkflowState, id: string | null): WorkflowState {
	return {
		...state,
		selectedId: id,
		error: null
	};
}

/**
 * Sets the loading state
 * @param state - Current workflow state
 * @param loading - Loading state value
 * @returns Updated state with new loading value
 */
export function setLoading(state: WorkflowState, loading: boolean): WorkflowState {
	return {
		...state,
		loading,
		error: loading ? null : state.error
	};
}

/**
 * Sets an error message
 * @param state - Current workflow state
 * @param error - Error message (or null to clear)
 * @returns Updated state with error
 */
export function setError(state: WorkflowState, error: string | null): WorkflowState {
	return {
		...state,
		error,
		loading: false
	};
}

/**
 * Sets the last execution result
 * @param state - Current workflow state
 * @param result - Workflow execution result
 * @returns Updated state with result
 */
export function setLastResult(state: WorkflowState, result: WorkflowResult | null): WorkflowState {
	return {
		...state,
		lastResult: result,
		loading: false,
		error: null
	};
}

/**
 * Sets the complete workflows list
 * @param state - Current workflow state
 * @param workflows - New workflows array
 * @returns Updated state with new workflows
 */
export function setWorkflows(state: WorkflowState, workflows: Workflow[]): WorkflowState {
	return {
		...state,
		workflows,
		loading: false,
		error: null
	};
}

/**
 * Updates workflow status
 * @param state - Current workflow state
 * @param id - Workflow ID
 * @param status - New status
 * @returns Updated state with new workflow status
 */
export function updateWorkflowStatus(
	state: WorkflowState,
	id: string,
	status: WorkflowStatus
): WorkflowState {
	return updateWorkflow(state, id, {
		status,
		updated_at: new Date()
	});
}

/**
 * Gets the currently selected workflow
 * @param state - Current workflow state
 * @returns Selected workflow or undefined
 */
export function getSelectedWorkflow(state: WorkflowState): Workflow | undefined {
	if (!state.selectedId) return undefined;
	return state.workflows.find((w) => w.id === state.selectedId);
}

/**
 * Gets workflows filtered by status
 * @param state - Current workflow state
 * @param status - Status to filter by
 * @returns Filtered workflows array
 */
export function getWorkflowsByStatus(state: WorkflowState, status: WorkflowStatus): Workflow[] {
	return state.workflows.filter((w) => w.status === status);
}

/**
 * Checks if a workflow exists
 * @param state - Current workflow state
 * @param id - Workflow ID to check
 * @returns True if workflow exists
 */
export function hasWorkflow(state: WorkflowState, id: string): boolean {
	return state.workflows.some((w) => w.id === id);
}

/**
 * Gets workflow count
 * @param state - Current workflow state
 * @returns Total number of workflows
 */
export function getWorkflowCount(state: WorkflowState): number {
	return state.workflows.length;
}

// ============================================================================
// Reactive Store (NEW - Phase B)
// ============================================================================

import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';

/**
 * State interface for the reactive workflow store
 */
interface WorkflowStoreState {
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
const initialStoreState: WorkflowStoreState = {
	workflows: [],
	selectedId: null,
	loading: false,
	error: null,
	searchFilter: ''
};

/**
 * Internal writable store
 */
const workflowWritable = writable<WorkflowStoreState>(initialStoreState);

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
