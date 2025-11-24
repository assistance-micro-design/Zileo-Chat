// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect, beforeEach } from 'vitest';
import {
	createInitialAgentState,
	setAgentIds,
	addAgentConfig,
	removeAgent,
	selectAgent,
	setAgentLoading,
	setAgentError,
	getSelectedAgentConfig,
	getAgentConfig,
	getAgentsByLifecycle,
	hasAgent,
	getAgentCount,
	getPermanentAgentCount,
	getTemporaryAgentCount,
	getAllAgentConfigs,
	type AgentState
} from '../agents';
import type { AgentConfig } from '$lib/types/agent';

describe('Agent Store', () => {
	let initialState: AgentState;

	const createMockAgentConfig = (id: string, lifecycle: 'permanent' | 'temporary'): AgentConfig => ({
		id,
		name: `Agent ${id}`,
		lifecycle,
		llm: {
			provider: 'Test',
			model: 'test-model',
			temperature: 0.7,
			max_tokens: 1000
		},
		tools: ['tool1'],
		mcp_servers: [],
		system_prompt: 'Test prompt'
	});

	beforeEach(() => {
		initialState = createInitialAgentState();
	});

	describe('createInitialAgentState', () => {
		it('should create empty initial state', () => {
			const state = createInitialAgentState();

			expect(state.agentIds).toEqual([]);
			expect(state.configs.size).toBe(0);
			expect(state.selectedId).toBeNull();
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
		});
	});

	describe('setAgentIds', () => {
		it('should set agent IDs', () => {
			const state = setAgentIds(initialState, ['agent1', 'agent2']);

			expect(state.agentIds).toEqual(['agent1', 'agent2']);
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
		});

		it('should replace existing IDs', () => {
			let state = setAgentIds(initialState, ['agent1']);
			state = setAgentIds(state, ['agent2', 'agent3']);

			expect(state.agentIds).toEqual(['agent2', 'agent3']);
		});
	});

	describe('addAgentConfig', () => {
		it('should add agent config', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			const state = addAgentConfig(initialState, config);

			expect(state.agentIds).toContain('agent1');
			expect(state.configs.get('agent1')).toEqual(config);
		});

		it('should not duplicate agent ID', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			let state = setAgentIds(initialState, ['agent1']);
			state = addAgentConfig(state, config);

			expect(state.agentIds.filter((id) => id === 'agent1')).toHaveLength(1);
		});

		it('should update config for existing agent', () => {
			const config1 = createMockAgentConfig('agent1', 'permanent');
			const config2 = { ...config1, name: 'Updated Agent' };

			let state = addAgentConfig(initialState, config1);
			state = addAgentConfig(state, config2);

			expect(state.configs.get('agent1')?.name).toBe('Updated Agent');
			expect(state.agentIds).toHaveLength(1);
		});
	});

	describe('removeAgent', () => {
		it('should remove agent', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			let state = addAgentConfig(initialState, config);

			state = removeAgent(state, 'agent1');

			expect(state.agentIds).not.toContain('agent1');
			expect(state.configs.has('agent1')).toBe(false);
		});

		it('should clear selection if removed agent was selected', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			let state = addAgentConfig(initialState, config);
			state = selectAgent(state, 'agent1');

			state = removeAgent(state, 'agent1');

			expect(state.selectedId).toBeNull();
		});

		it('should keep selection if different agent removed', () => {
			const config1 = createMockAgentConfig('agent1', 'permanent');
			const config2 = createMockAgentConfig('agent2', 'permanent');

			let state = addAgentConfig(initialState, config1);
			state = addAgentConfig(state, config2);
			state = selectAgent(state, 'agent1');

			state = removeAgent(state, 'agent2');

			expect(state.selectedId).toBe('agent1');
		});
	});

	describe('selectAgent', () => {
		it('should select an agent', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			let state = addAgentConfig(initialState, config);

			state = selectAgent(state, 'agent1');

			expect(state.selectedId).toBe('agent1');
		});

		it('should deselect with null', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			let state = addAgentConfig(initialState, config);
			state = selectAgent(state, 'agent1');

			state = selectAgent(state, null);

			expect(state.selectedId).toBeNull();
		});
	});

	describe('setAgentLoading', () => {
		it('should set loading state', () => {
			const state = setAgentLoading(initialState, true);
			expect(state.loading).toBe(true);
		});

		it('should clear error when loading', () => {
			let state = setAgentError(initialState, 'Error');
			state = setAgentLoading(state, true);
			expect(state.error).toBeNull();
		});
	});

	describe('setAgentError', () => {
		it('should set error message', () => {
			const state = setAgentError(initialState, 'Test error');
			expect(state.error).toBe('Test error');
			expect(state.loading).toBe(false);
		});
	});

	describe('getSelectedAgentConfig', () => {
		it('should return selected agent config', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			let state = addAgentConfig(initialState, config);
			state = selectAgent(state, 'agent1');

			const selected = getSelectedAgentConfig(state);

			expect(selected?.id).toBe('agent1');
		});

		it('should return undefined when nothing selected', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			const state = addAgentConfig(initialState, config);

			const selected = getSelectedAgentConfig(state);

			expect(selected).toBeUndefined();
		});
	});

	describe('getAgentConfig', () => {
		it('should return agent config by ID', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			const state = addAgentConfig(initialState, config);

			const result = getAgentConfig(state, 'agent1');

			expect(result?.id).toBe('agent1');
		});

		it('should return undefined for non-existent agent', () => {
			const result = getAgentConfig(initialState, 'nonexistent');
			expect(result).toBeUndefined();
		});
	});

	describe('getAgentsByLifecycle', () => {
		it('should filter agents by lifecycle', () => {
			const perm1 = createMockAgentConfig('perm1', 'permanent');
			const perm2 = createMockAgentConfig('perm2', 'permanent');
			const temp1 = createMockAgentConfig('temp1', 'temporary');

			let state = addAgentConfig(initialState, perm1);
			state = addAgentConfig(state, perm2);
			state = addAgentConfig(state, temp1);

			const permanent = getAgentsByLifecycle(state, 'permanent');
			const temporary = getAgentsByLifecycle(state, 'temporary');

			expect(permanent).toHaveLength(2);
			expect(temporary).toHaveLength(1);
		});
	});

	describe('hasAgent', () => {
		it('should return true for existing agent', () => {
			const config = createMockAgentConfig('agent1', 'permanent');
			const state = addAgentConfig(initialState, config);

			expect(hasAgent(state, 'agent1')).toBe(true);
		});

		it('should return false for non-existing agent', () => {
			expect(hasAgent(initialState, 'nonexistent')).toBe(false);
		});
	});

	describe('getAgentCount', () => {
		it('should return 0 for empty state', () => {
			expect(getAgentCount(initialState)).toBe(0);
		});

		it('should return correct count', () => {
			const config1 = createMockAgentConfig('agent1', 'permanent');
			const config2 = createMockAgentConfig('agent2', 'temporary');

			let state = addAgentConfig(initialState, config1);
			state = addAgentConfig(state, config2);

			expect(getAgentCount(state)).toBe(2);
		});
	});

	describe('getPermanentAgentCount', () => {
		it('should return count of permanent agents', () => {
			const perm = createMockAgentConfig('perm', 'permanent');
			const temp = createMockAgentConfig('temp', 'temporary');

			let state = addAgentConfig(initialState, perm);
			state = addAgentConfig(state, temp);

			expect(getPermanentAgentCount(state)).toBe(1);
		});
	});

	describe('getTemporaryAgentCount', () => {
		it('should return count of temporary agents', () => {
			const perm = createMockAgentConfig('perm', 'permanent');
			const temp = createMockAgentConfig('temp', 'temporary');

			let state = addAgentConfig(initialState, perm);
			state = addAgentConfig(state, temp);

			expect(getTemporaryAgentCount(state)).toBe(1);
		});
	});

	describe('getAllAgentConfigs', () => {
		it('should return all configs as array', () => {
			const config1 = createMockAgentConfig('agent1', 'permanent');
			const config2 = createMockAgentConfig('agent2', 'temporary');

			let state = addAgentConfig(initialState, config1);
			state = addAgentConfig(state, config2);

			const configs = getAllAgentConfigs(state);

			expect(configs).toHaveLength(2);
			expect(configs.map((c) => c.id)).toContain('agent1');
			expect(configs.map((c) => c.id)).toContain('agent2');
		});

		it('should return empty array for empty state', () => {
			const configs = getAllAgentConfigs(initialState);
			expect(configs).toEqual([]);
		});
	});
});
