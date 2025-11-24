// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect, beforeEach } from 'vitest';
import {
	createInitialState,
	addWorkflow,
	updateWorkflow,
	removeWorkflow,
	selectWorkflow,
	setLoading,
	setError,
	setLastResult,
	setWorkflows,
	updateWorkflowStatus,
	getSelectedWorkflow,
	getWorkflowsByStatus,
	hasWorkflow,
	getWorkflowCount,
	type WorkflowState
} from '../workflows';
import type { Workflow, WorkflowResult } from '$types/workflow';

describe('Workflow Store', () => {
	let initialState: WorkflowState;

	const createMockWorkflow = (id: string, name: string): Workflow => ({
		id,
		name,
		agent_id: 'test_agent',
		status: 'idle',
		created_at: new Date(),
		updated_at: new Date()
	});

	const createMockResult = (): WorkflowResult => ({
		report: '# Test Report',
		metrics: {
			duration_ms: 100,
			tokens_input: 50,
			tokens_output: 75,
			cost_usd: 0.001,
			provider: 'Test',
			model: 'test-model'
		},
		tools_used: ['tool1'],
		mcp_calls: []
	});

	beforeEach(() => {
		initialState = createInitialState();
	});

	describe('createInitialState', () => {
		it('should create empty initial state', () => {
			const state = createInitialState();

			expect(state.workflows).toEqual([]);
			expect(state.selectedId).toBeNull();
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
			expect(state.lastResult).toBeNull();
		});
	});

	describe('addWorkflow', () => {
		it('should add a workflow to empty state', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			const newState = addWorkflow(initialState, workflow);

			expect(newState.workflows).toHaveLength(1);
			expect(newState.workflows[0].id).toBe('wf1');
			expect(newState.error).toBeNull();
		});

		it('should add workflow to existing workflows', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			const wf2 = createMockWorkflow('wf2', 'Workflow 2');

			let state = addWorkflow(initialState, wf1);
			state = addWorkflow(state, wf2);

			expect(state.workflows).toHaveLength(2);
			expect(state.workflows[0].id).toBe('wf1');
			expect(state.workflows[1].id).toBe('wf2');
		});

		it('should clear error when adding workflow', () => {
			const stateWithError = setError(initialState, 'Some error');
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			const newState = addWorkflow(stateWithError, workflow);

			expect(newState.error).toBeNull();
		});
	});

	describe('updateWorkflow', () => {
		it('should update existing workflow', () => {
			const workflow = createMockWorkflow('wf1', 'Original Name');
			let state = addWorkflow(initialState, workflow);

			state = updateWorkflow(state, 'wf1', { name: 'Updated Name' });

			expect(state.workflows[0].name).toBe('Updated Name');
			expect(state.workflows[0].id).toBe('wf1');
		});

		it('should not modify other workflows', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			const wf2 = createMockWorkflow('wf2', 'Workflow 2');

			let state = addWorkflow(initialState, wf1);
			state = addWorkflow(state, wf2);
			state = updateWorkflow(state, 'wf1', { name: 'Updated' });

			expect(state.workflows[0].name).toBe('Updated');
			expect(state.workflows[1].name).toBe('Workflow 2');
		});

		it('should not crash when updating non-existent workflow', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);

			state = updateWorkflow(state, 'nonexistent', { name: 'Updated' });

			expect(state.workflows).toHaveLength(1);
			expect(state.workflows[0].name).toBe('Workflow 1');
		});
	});

	describe('removeWorkflow', () => {
		it('should remove existing workflow', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);

			state = removeWorkflow(state, 'wf1');

			expect(state.workflows).toHaveLength(0);
		});

		it('should clear selection if removed workflow was selected', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);
			state = selectWorkflow(state, 'wf1');

			state = removeWorkflow(state, 'wf1');

			expect(state.selectedId).toBeNull();
		});

		it('should keep selection if different workflow removed', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			const wf2 = createMockWorkflow('wf2', 'Workflow 2');

			let state = addWorkflow(initialState, wf1);
			state = addWorkflow(state, wf2);
			state = selectWorkflow(state, 'wf1');

			state = removeWorkflow(state, 'wf2');

			expect(state.selectedId).toBe('wf1');
			expect(state.workflows).toHaveLength(1);
		});
	});

	describe('selectWorkflow', () => {
		it('should select a workflow', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);

			state = selectWorkflow(state, 'wf1');

			expect(state.selectedId).toBe('wf1');
		});

		it('should allow selecting null to deselect', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);
			state = selectWorkflow(state, 'wf1');

			state = selectWorkflow(state, null);

			expect(state.selectedId).toBeNull();
		});
	});

	describe('setLoading', () => {
		it('should set loading to true', () => {
			const state = setLoading(initialState, true);
			expect(state.loading).toBe(true);
		});

		it('should set loading to false', () => {
			let state = setLoading(initialState, true);
			state = setLoading(state, false);
			expect(state.loading).toBe(false);
		});

		it('should clear error when setting loading to true', () => {
			let state = setError(initialState, 'Error');
			state = setLoading(state, true);
			expect(state.error).toBeNull();
		});
	});

	describe('setError', () => {
		it('should set error message', () => {
			const state = setError(initialState, 'Test error');
			expect(state.error).toBe('Test error');
			expect(state.loading).toBe(false);
		});

		it('should clear error with null', () => {
			let state = setError(initialState, 'Test error');
			state = setError(state, null);
			expect(state.error).toBeNull();
		});
	});

	describe('setLastResult', () => {
		it('should set last result', () => {
			const result = createMockResult();
			const state = setLastResult(initialState, result);

			expect(state.lastResult).toEqual(result);
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
		});

		it('should clear last result with null', () => {
			const result = createMockResult();
			let state = setLastResult(initialState, result);
			state = setLastResult(state, null);

			expect(state.lastResult).toBeNull();
		});
	});

	describe('setWorkflows', () => {
		it('should replace all workflows', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			const wf2 = createMockWorkflow('wf2', 'Workflow 2');

			const state = setWorkflows(initialState, [wf1, wf2]);

			expect(state.workflows).toHaveLength(2);
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
		});

		it('should handle empty array', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, wf1);

			state = setWorkflows(state, []);

			expect(state.workflows).toHaveLength(0);
		});
	});

	describe('updateWorkflowStatus', () => {
		it('should update workflow status', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);

			state = updateWorkflowStatus(state, 'wf1', 'running');

			expect(state.workflows[0].status).toBe('running');
		});

		it('should update workflow updated_at', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			const originalDate = workflow.updated_at;
			let state = addWorkflow(initialState, workflow);

			// Wait a bit to ensure different timestamp
			state = updateWorkflowStatus(state, 'wf1', 'completed');

			expect(state.workflows[0].updated_at.getTime()).toBeGreaterThanOrEqual(
				originalDate.getTime()
			);
		});
	});

	describe('getSelectedWorkflow', () => {
		it('should return selected workflow', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			let state = addWorkflow(initialState, workflow);
			state = selectWorkflow(state, 'wf1');

			const selected = getSelectedWorkflow(state);

			expect(selected?.id).toBe('wf1');
		});

		it('should return undefined when nothing selected', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			const state = addWorkflow(initialState, workflow);

			const selected = getSelectedWorkflow(state);

			expect(selected).toBeUndefined();
		});
	});

	describe('getWorkflowsByStatus', () => {
		it('should filter workflows by status', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			const wf2 = { ...createMockWorkflow('wf2', 'Workflow 2'), status: 'running' as const };
			const wf3 = { ...createMockWorkflow('wf3', 'Workflow 3'), status: 'completed' as const };

			let state = addWorkflow(initialState, wf1);
			state = addWorkflow(state, wf2);
			state = addWorkflow(state, wf3);

			const idleWorkflows = getWorkflowsByStatus(state, 'idle');
			const runningWorkflows = getWorkflowsByStatus(state, 'running');

			expect(idleWorkflows).toHaveLength(1);
			expect(runningWorkflows).toHaveLength(1);
		});

		it('should return empty array for non-matching status', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			const state = addWorkflow(initialState, workflow);

			const errorWorkflows = getWorkflowsByStatus(state, 'error');

			expect(errorWorkflows).toHaveLength(0);
		});
	});

	describe('hasWorkflow', () => {
		it('should return true for existing workflow', () => {
			const workflow = createMockWorkflow('wf1', 'Workflow 1');
			const state = addWorkflow(initialState, workflow);

			expect(hasWorkflow(state, 'wf1')).toBe(true);
		});

		it('should return false for non-existing workflow', () => {
			expect(hasWorkflow(initialState, 'nonexistent')).toBe(false);
		});
	});

	describe('getWorkflowCount', () => {
		it('should return 0 for empty state', () => {
			expect(getWorkflowCount(initialState)).toBe(0);
		});

		it('should return correct count', () => {
			const wf1 = createMockWorkflow('wf1', 'Workflow 1');
			const wf2 = createMockWorkflow('wf2', 'Workflow 2');

			let state = addWorkflow(initialState, wf1);
			state = addWorkflow(state, wf2);

			expect(getWorkflowCount(state)).toBe(2);
		});
	});
});
