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

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { get } from 'svelte/store';
import {
	agentStore,
	agents,
	selectedAgent,
	isLoading,
	error,
	formMode,
	editingAgent,
	agentCount,
	hasAgents,
	createInitialAgentState
} from '../agents';
import type { AgentConfig, AgentSummary, AgentConfigCreate } from '$types/agent';

// Mock Tauri's invoke function
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

import { invoke } from '@tauri-apps/api/core';

const mockInvoke = invoke as Mock;

describe('Agent Store', () => {
	// Helper to create mock agent summary
	const createMockAgentSummary = (
		id: string,
		lifecycle: 'permanent' | 'temporary' = 'permanent'
	): AgentSummary => ({
		id,
		name: `Agent ${id}`,
		lifecycle,
		provider: 'Mistral',
		model: 'mistral-large-latest',
		tools_count: 2,
		mcp_servers_count: 1
	});

	// Helper to create mock agent config
	const createMockAgentConfig = (
		id: string,
		lifecycle: 'permanent' | 'temporary' = 'permanent'
	): AgentConfig => ({
		id,
		name: `Agent ${id}`,
		lifecycle,
		llm: {
			provider: 'Mistral',
			model: 'mistral-large-latest',
			temperature: 0.7,
			max_tokens: 4096
		},
		tools: ['MemoryTool', 'TodoTool'],
		mcp_servers: ['serena'],
		system_prompt: 'You are a helpful assistant.',
		max_tool_iterations: 50
	});

	beforeEach(() => {
		// Reset store to initial state
		agentStore.reset();
		// Clear all mock calls
		vi.clearAllMocks();
	});

	describe('Initial State', () => {
		it('should have empty initial state', () => {
			const state = get(agentStore);

			expect(state.agents).toEqual([]);
			expect(state.selectedId).toBeNull();
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
			expect(state.formMode).toBeNull();
			expect(state.editingAgent).toBeNull();
		});
	});

	describe('loadAgents', () => {
		it('should load agents from backend', async () => {
			const mockAgents = [
				createMockAgentSummary('agent1'),
				createMockAgentSummary('agent2', 'temporary')
			];
			mockInvoke.mockResolvedValueOnce(mockAgents);

			await agentStore.loadAgents();

			expect(mockInvoke).toHaveBeenCalledWith('list_agents');
			expect(get(agents)).toEqual(mockAgents);
			expect(get(isLoading)).toBe(false);
			expect(get(error)).toBeNull();
		});

		it('should set loading state while loading', async () => {
			mockInvoke.mockImplementation(
				() => new Promise((resolve) => setTimeout(() => resolve([]), 100))
			);

			const loadPromise = agentStore.loadAgents();

			// Check loading state immediately
			expect(get(isLoading)).toBe(true);

			await loadPromise;
			expect(get(isLoading)).toBe(false);
		});

		it('should handle errors', async () => {
			mockInvoke.mockRejectedValueOnce(new Error('Network error'));

			await agentStore.loadAgents();

			expect(get(error)).toBe('Error: Network error');
			expect(get(isLoading)).toBe(false);
		});
	});

	describe('createAgent', () => {
		it('should create agent and refresh list', async () => {
			const newAgentId = 'new-agent-uuid';
			const config: AgentConfigCreate = {
				name: 'New Agent',
				lifecycle: 'permanent',
				llm: {
					provider: 'Mistral',
					model: 'mistral-large-latest',
					temperature: 0.7,
					max_tokens: 4096
				},
				tools: ['MemoryTool'],
				mcp_servers: [],
				system_prompt: 'Test prompt'
			};

			mockInvoke.mockResolvedValueOnce(newAgentId); // create_agent
			mockInvoke.mockResolvedValueOnce([createMockAgentSummary(newAgentId)]); // list_agents

			const result = await agentStore.createAgent(config);

			expect(result).toBe(newAgentId);
			expect(mockInvoke).toHaveBeenCalledWith('create_agent', { config });
			expect(get(formMode)).toBeNull(); // Form closed after creation
		});

		it('should handle creation errors', async () => {
			const config: AgentConfigCreate = {
				name: '',
				lifecycle: 'permanent',
				llm: {
					provider: 'Mistral',
					model: 'mistral-large-latest',
					temperature: 0.7,
					max_tokens: 4096
				},
				tools: [],
				mcp_servers: [],
				system_prompt: ''
			};

			mockInvoke.mockRejectedValueOnce(new Error('Validation failed'));

			await expect(agentStore.createAgent(config)).rejects.toThrow();
			expect(get(error)).toBe('Error: Validation failed');
		});
	});

	describe('updateAgent', () => {
		it('should update agent and refresh list', async () => {
			mockInvoke.mockResolvedValueOnce(undefined); // update_agent
			mockInvoke.mockResolvedValueOnce([createMockAgentSummary('agent1')]); // list_agents

			await agentStore.updateAgent('agent1', { name: 'Updated Name' });

			expect(mockInvoke).toHaveBeenCalledWith('update_agent', {
				agentId: 'agent1',
				config: { name: 'Updated Name' }
			});
			expect(get(formMode)).toBeNull();
			expect(get(editingAgent)).toBeNull();
		});
	});

	describe('deleteAgent', () => {
		it('should delete agent and refresh list', async () => {
			// Setup initial state with agents
			mockInvoke.mockResolvedValueOnce([
				createMockAgentSummary('agent1'),
				createMockAgentSummary('agent2')
			]);
			await agentStore.loadAgents();

			// Select agent1
			agentStore.select('agent1');

			// Delete agent1
			mockInvoke.mockResolvedValueOnce(undefined); // delete_agent
			mockInvoke.mockResolvedValueOnce([createMockAgentSummary('agent2')]); // list_agents

			await agentStore.deleteAgent('agent1');

			expect(mockInvoke).toHaveBeenCalledWith('delete_agent', { agentId: 'agent1' });
			expect(get(agentStore).selectedId).toBeNull(); // Selection cleared
		});

		it('should keep selection if different agent deleted', async () => {
			// Setup initial state
			mockInvoke.mockResolvedValueOnce([
				createMockAgentSummary('agent1'),
				createMockAgentSummary('agent2')
			]);
			await agentStore.loadAgents();

			// Select agent1
			agentStore.select('agent1');

			// Delete agent2
			mockInvoke.mockResolvedValueOnce(undefined);
			mockInvoke.mockResolvedValueOnce([createMockAgentSummary('agent1')]);

			await agentStore.deleteAgent('agent2');

			expect(get(agentStore).selectedId).toBe('agent1');
		});
	});

	describe('getAgentConfig', () => {
		it('should fetch full agent config', async () => {
			const config = createMockAgentConfig('agent1');
			mockInvoke.mockResolvedValueOnce(config);

			const result = await agentStore.getAgentConfig('agent1');

			expect(result).toEqual(config);
			expect(mockInvoke).toHaveBeenCalledWith('get_agent_config', { agentId: 'agent1' });
		});
	});

	describe('select', () => {
		it('should select an agent', () => {
			agentStore.select('agent1');
			expect(get(agentStore).selectedId).toBe('agent1');
		});

		it('should deselect with null', () => {
			agentStore.select('agent1');
			agentStore.select(null);
			expect(get(agentStore).selectedId).toBeNull();
		});
	});

	describe('Form Management', () => {
		it('should open create form', () => {
			agentStore.openCreateForm();

			expect(get(formMode)).toBe('create');
			expect(get(editingAgent)).toBeNull();
		});

		it('should open edit form with agent config', async () => {
			const config = createMockAgentConfig('agent1');
			mockInvoke.mockResolvedValueOnce(config);

			await agentStore.openEditForm('agent1');

			expect(get(formMode)).toBe('edit');
			expect(get(editingAgent)).toEqual(config);
		});

		it('should close form', () => {
			agentStore.openCreateForm();
			agentStore.closeForm();

			expect(get(formMode)).toBeNull();
			expect(get(editingAgent)).toBeNull();
		});
	});

	describe('Error Handling', () => {
		it('should clear error', async () => {
			mockInvoke.mockRejectedValueOnce(new Error('Test error'));
			await agentStore.loadAgents();

			expect(get(error)).toBe('Error: Test error');

			agentStore.clearError();
			expect(get(error)).toBeNull();
		});
	});

	describe('reset', () => {
		it('should reset to initial state', async () => {
			// Setup some state
			mockInvoke.mockResolvedValueOnce([createMockAgentSummary('agent1')]);
			await agentStore.loadAgents();
			agentStore.select('agent1');
			agentStore.openCreateForm();

			// Reset
			agentStore.reset();

			const state = get(agentStore);
			expect(state.agents).toEqual([]);
			expect(state.selectedId).toBeNull();
			expect(state.formMode).toBeNull();
		});
	});

	describe('Derived Stores', () => {
		beforeEach(async () => {
			mockInvoke.mockResolvedValueOnce([
				createMockAgentSummary('agent1'),
				createMockAgentSummary('agent2', 'temporary')
			]);
			await agentStore.loadAgents();
		});

		it('agents should return all agents', () => {
			expect(get(agents)).toHaveLength(2);
		});

		it('selectedAgent should return selected agent', () => {
			agentStore.select('agent1');
			expect(get(selectedAgent)?.id).toBe('agent1');
		});

		it('selectedAgent should return null when nothing selected', () => {
			expect(get(selectedAgent)).toBeNull();
		});

		it('agentCount should return correct count', () => {
			expect(get(agentCount)).toBe(2);
		});

		it('hasAgents should return true when agents exist', () => {
			expect(get(hasAgents)).toBe(true);
		});

		it('hasAgents should return false when no agents', () => {
			agentStore.reset();
			expect(get(hasAgents)).toBe(false);
		});
	});

	describe('Legacy Functions', () => {
		it('createInitialAgentState should return legacy state structure', () => {
			const state = createInitialAgentState();

			expect(state.agentIds).toEqual([]);
			expect(state.configs).toBeInstanceOf(Map);
			expect(state.selectedId).toBeNull();
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
		});
	});
});
