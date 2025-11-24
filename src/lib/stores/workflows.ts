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
