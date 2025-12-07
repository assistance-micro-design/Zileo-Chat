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

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import {
	workflowStore,
	workflows,
	selectedWorkflowId,
	workflowsLoading,
	workflowsError,
	workflowSearchFilter,
	selectedWorkflow,
	filteredWorkflows,
	type WorkflowState
} from '../workflows';
import type { Workflow } from '$types/workflow';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

// Import invoke after mocking
import { invoke } from '@tauri-apps/api/core';

describe('Workflow Store', () => {
	const createMockWorkflow = (id: string, name: string): Workflow => ({
		id,
		name,
		agent_id: 'test_agent',
		status: 'idle',
		created_at: new Date(),
		updated_at: new Date(),
		total_tokens_input: 0,
		total_tokens_output: 0,
		total_cost_usd: 0,
		current_context_tokens: 0
	});

	beforeEach(() => {
		workflowStore.reset();
		vi.resetAllMocks();
	});

	describe('workflowStore.loadWorkflows', () => {
		it('should load workflows from backend', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Test Workflow')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);

			await workflowStore.loadWorkflows();

			expect(get(workflows)).toEqual(mockWorkflows);
			expect(get(workflowsLoading)).toBe(false);
			expect(get(workflowsError)).toBeNull();
		});

		it('should set loading state during load', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Test')];
			vi.mocked(invoke).mockImplementation(
				() =>
					new Promise((resolve) => {
						expect(get(workflowsLoading)).toBe(true);
						resolve(mockWorkflows);
					})
			);

			await workflowStore.loadWorkflows();

			expect(get(workflowsLoading)).toBe(false);
		});

		it('should handle errors during load', async () => {
			const errorMessage = 'Network error';
			vi.mocked(invoke).mockRejectedValue(new Error(errorMessage));

			await workflowStore.loadWorkflows();

			expect(get(workflowsError)).toBe(errorMessage);
			expect(get(workflowsLoading)).toBe(false);
		});

		it('should handle non-Error rejections', async () => {
			vi.mocked(invoke).mockRejectedValue('String error');

			await workflowStore.loadWorkflows();

			expect(get(workflowsError)).toBe('String error');
			expect(get(workflowsLoading)).toBe(false);
		});

		it('should clear previous error on successful load', async () => {
			// First load fails
			vi.mocked(invoke).mockRejectedValue(new Error('First error'));
			await workflowStore.loadWorkflows();
			expect(get(workflowsError)).toBe('First error');

			// Second load succeeds
			const mockWorkflows = [createMockWorkflow('wf1', 'Test')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			expect(get(workflowsError)).toBeNull();
			expect(get(workflows)).toEqual(mockWorkflows);
		});
	});

	describe('workflowStore.select', () => {
		it('should select a workflow by ID', () => {
			workflowStore.select('wf1');

			expect(get(selectedWorkflowId)).toBe('wf1');
		});

		it('should allow selecting null to deselect', () => {
			workflowStore.select('wf1');
			expect(get(selectedWorkflowId)).toBe('wf1');

			workflowStore.select(null);
			expect(get(selectedWorkflowId)).toBeNull();
		});

		it('should change selection', () => {
			workflowStore.select('wf1');
			expect(get(selectedWorkflowId)).toBe('wf1');

			workflowStore.select('wf2');
			expect(get(selectedWorkflowId)).toBe('wf2');
		});
	});

	describe('workflowStore.setSearchFilter', () => {
		it('should set search filter', () => {
			workflowStore.setSearchFilter('test query');

			expect(get(workflowSearchFilter)).toBe('test query');
		});

		it('should update search filter', () => {
			workflowStore.setSearchFilter('first');
			expect(get(workflowSearchFilter)).toBe('first');

			workflowStore.setSearchFilter('second');
			expect(get(workflowSearchFilter)).toBe('second');
		});

		it('should clear search filter with empty string', () => {
			workflowStore.setSearchFilter('test');
			expect(get(workflowSearchFilter)).toBe('test');

			workflowStore.setSearchFilter('');
			expect(get(workflowSearchFilter)).toBe('');
		});
	});

	describe('workflowStore.getSelected', () => {
		it('should return selected workflow', async () => {
			const mockWorkflows = [
				createMockWorkflow('wf1', 'Workflow 1'),
				createMockWorkflow('wf2', 'Workflow 2')
			];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			workflowStore.select('wf1');

			const selected = workflowStore.getSelected();
			expect(selected?.id).toBe('wf1');
			expect(selected?.name).toBe('Workflow 1');
		});

		it('should return undefined when nothing selected', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Workflow 1')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			const selected = workflowStore.getSelected();
			expect(selected).toBeUndefined();
		});

		it('should return undefined when selected workflow does not exist', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Workflow 1')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			workflowStore.select('nonexistent');

			const selected = workflowStore.getSelected();
			expect(selected).toBeUndefined();
		});
	});

	describe('workflowStore.reset', () => {
		it('should reset to initial state', async () => {
			// Load some workflows
			const mockWorkflows = [createMockWorkflow('wf1', 'Test')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			// Select a workflow and set filter
			workflowStore.select('wf1');
			workflowStore.setSearchFilter('test');

			// Reset
			workflowStore.reset();

			expect(get(workflows)).toEqual([]);
			expect(get(selectedWorkflowId)).toBeNull();
			expect(get(workflowsLoading)).toBe(false);
			expect(get(workflowsError)).toBeNull();
			expect(get(workflowSearchFilter)).toBe('');
		});

		it('should clear error state', async () => {
			vi.mocked(invoke).mockRejectedValue(new Error('Test error'));
			await workflowStore.loadWorkflows();
			expect(get(workflowsError)).toBe('Test error');

			workflowStore.reset();

			expect(get(workflowsError)).toBeNull();
		});
	});

	describe('derived store: selectedWorkflow', () => {
		it('should return selected workflow object', async () => {
			const mockWorkflows = [
				createMockWorkflow('wf1', 'Workflow 1'),
				createMockWorkflow('wf2', 'Workflow 2')
			];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			workflowStore.select('wf2');

			const selected = get(selectedWorkflow);
			expect(selected?.id).toBe('wf2');
			expect(selected?.name).toBe('Workflow 2');
		});

		it('should return null when nothing selected', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Workflow 1')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			const selected = get(selectedWorkflow);
			expect(selected).toBeNull();
		});

		it('should return null when selected workflow does not exist', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Workflow 1')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			workflowStore.select('nonexistent');

			const selected = get(selectedWorkflow);
			expect(selected).toBeNull();
		});
	});

	describe('derived store: filteredWorkflows', () => {
		const mockWorkflows = [
			createMockWorkflow('wf1', 'Database Migration'),
			createMockWorkflow('wf2', 'API Testing'),
			createMockWorkflow('wf3', 'Database Backup')
		];

		beforeEach(async () => {
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();
		});

		it('should return all workflows when filter is empty', () => {
			const filtered = get(filteredWorkflows);
			expect(filtered).toHaveLength(3);
		});

		it('should filter workflows by name (case insensitive)', () => {
			workflowStore.setSearchFilter('database');

			const filtered = get(filteredWorkflows);
			expect(filtered).toHaveLength(2);
			expect(filtered.map((w) => w.id)).toEqual(['wf1', 'wf3']);
		});

		it('should filter workflows case insensitively', () => {
			workflowStore.setSearchFilter('DATABASE');

			const filtered = get(filteredWorkflows);
			expect(filtered).toHaveLength(2);
		});

		it('should return empty array when no matches', () => {
			workflowStore.setSearchFilter('nonexistent');

			const filtered = get(filteredWorkflows);
			expect(filtered).toHaveLength(0);
		});

		it('should update when filter changes', () => {
			workflowStore.setSearchFilter('api');
			let filtered = get(filteredWorkflows);
			expect(filtered).toHaveLength(1);
			expect(filtered[0].id).toBe('wf2');

			workflowStore.setSearchFilter('migration');
			filtered = get(filteredWorkflows);
			expect(filtered).toHaveLength(1);
			expect(filtered[0].id).toBe('wf1');
		});

		it('should show all workflows when filter is cleared', () => {
			workflowStore.setSearchFilter('database');
			expect(get(filteredWorkflows)).toHaveLength(2);

			workflowStore.setSearchFilter('');
			expect(get(filteredWorkflows)).toHaveLength(3);
		});
	});

	describe('derived stores: basic getters', () => {
		it('should expose workflows array', async () => {
			const mockWorkflows = [createMockWorkflow('wf1', 'Test')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			expect(get(workflows)).toEqual(mockWorkflows);
		});

		it('should expose selectedWorkflowId', () => {
			workflowStore.select('wf1');
			expect(get(selectedWorkflowId)).toBe('wf1');
		});

		it('should expose workflowsLoading', async () => {
			expect(get(workflowsLoading)).toBe(false);

			const promise = workflowStore.loadWorkflows();
			// Note: Loading state is set synchronously before await
			await promise;

			expect(get(workflowsLoading)).toBe(false);
		});

		it('should expose workflowsError', async () => {
			vi.mocked(invoke).mockRejectedValue(new Error('Test error'));
			await workflowStore.loadWorkflows();

			expect(get(workflowsError)).toBe('Test error');
		});

		it('should expose workflowSearchFilter', () => {
			workflowStore.setSearchFilter('test');
			expect(get(workflowSearchFilter)).toBe('test');
		});
	});

	describe('store subscription', () => {
		it('should allow subscribing to store changes', async () => {
			const states: WorkflowState[] = [];
			const unsubscribe = workflowStore.subscribe((state) => {
				states.push(state);
			});

			// Initial state
			expect(states).toHaveLength(1);
			expect(states[0].workflows).toEqual([]);

			// Load workflows
			const mockWorkflows = [createMockWorkflow('wf1', 'Test')];
			vi.mocked(invoke).mockResolvedValue(mockWorkflows);
			await workflowStore.loadWorkflows();

			// Should have captured state changes
			expect(states.length).toBeGreaterThan(1);
			expect(states[states.length - 1].workflows).toEqual(mockWorkflows);

			unsubscribe();
		});
	});
});
