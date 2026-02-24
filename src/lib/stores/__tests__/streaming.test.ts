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
 * Unit tests for the streaming store.
 * Tests token accumulation, tool tracking, reasoning steps, and state management.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

// Mock Tauri's event API
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {})
}));

import { streamingStore, activeSubAgents } from '../streaming';

/** Helper: check if streaming state has visible activities */
function hasActivities(): boolean {
	const s = streamingStore.getState();
	return (
		s.isStreaming ||
		(s.completed &&
			(s.tools.length > 0 ||
				s.reasoning.length > 0 ||
				s.subAgents.length > 0 ||
				s.tasks.length > 0))
	);
}

describe('streamingStore', () => {
	beforeEach(async () => {
		// Reset store before each test
		await streamingStore.reset();
	});

	describe('initial state', () => {
		it('should have correct initial values', () => {
			const state = streamingStore.getState();
			expect(state.isStreaming).toBe(false);
			expect(state.content).toBe('');
			expect(state.tools).toEqual([]);
			expect(state.reasoning).toEqual([]);
			expect(state.error).toBe(null);
			expect(state.cancelled).toBe(false);
			expect(state.tokensReceived).toBe(0);
		});
	});

	describe('appendToken', () => {
		it('should append tokens to content', () => {
			streamingStore.appendToken('Hello');
			expect(streamingStore.getState().content).toBe('Hello');

			streamingStore.appendToken(' World');
			expect(streamingStore.getState().content).toBe('Hello World');
		});

		it('should increment token count', () => {
			streamingStore.appendToken('a');
			streamingStore.appendToken('b');
			streamingStore.appendToken('c');
			expect(streamingStore.getState().tokensReceived).toBe(3);
		});
	});

	describe('tool tracking', () => {
		it('should track tool start', () => {
			streamingStore.addToolStart('MemoryTool');

			const tools = streamingStore.getState().tools;
			expect(tools).toHaveLength(1);
			expect(tools[0].name).toBe('MemoryTool');
			expect(tools[0].status).toBe('running');
			expect(tools[0].startedAt).toBeDefined();
		});

		it('should complete tool with duration', () => {
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 150);

			const tools = streamingStore.getState().tools;
			expect(tools[0].status).toBe('completed');
			expect(tools[0].duration).toBe(150);
		});

		it('should track multiple tools', () => {
			streamingStore.addToolStart('MemoryTool');
			streamingStore.addToolStart('TodoTool');
			streamingStore.completeToolEnd('MemoryTool', 100);

			const tools = streamingStore.getState().tools;
			const running = tools.filter((t) => t.status === 'running');
			const completed = tools.filter((t) => t.status === 'completed');

			expect(running).toHaveLength(1);
			expect(running[0].name).toBe('TodoTool');
			expect(completed).toHaveLength(1);
			expect(completed[0].name).toBe('MemoryTool');
		});

		it('should fail tool with error', () => {
			streamingStore.addToolStart('MemoryTool');
			streamingStore.failTool('MemoryTool', 'Connection failed');

			const tools = streamingStore.getState().tools;
			expect(tools[0].status).toBe('error');
			expect(tools[0].error).toBe('Connection failed');
		});
	});

	describe('reasoning steps', () => {
		it('should add reasoning steps', () => {
			streamingStore.addReasoning('Analyzing request...');
			streamingStore.addReasoning('Planning response...');

			const steps = streamingStore.getState().reasoning;
			expect(steps).toHaveLength(2);
			expect(steps[0].content).toBe('Analyzing request...');
			expect(steps[0].stepNumber).toBe(1);
			expect(steps[1].content).toBe('Planning response...');
			expect(steps[1].stepNumber).toBe(2);
		});
	});

	describe('error handling', () => {
		it('should set error and stop streaming', () => {
			// Manually set streaming state first
			streamingStore.appendToken('Test');

			streamingStore.setError('Network error');

			const state = streamingStore.getState();
			expect(state.error).toBe('Network error');
			expect(state.isStreaming).toBe(false);
		});
	});

	describe('completion', () => {
		it('should mark as completed while keeping streaming activities visible', () => {
			streamingStore.appendToken('Test');
			streamingStore.addToolStart('MemoryTool');
			streamingStore.complete();

			const state = streamingStore.getState();
			expect(state.completed).toBe(true);
			expect(hasActivities()).toBe(true);
		});

		it('should keep activities visible until explicitly reset', async () => {
			streamingStore.appendToken('Test');
			streamingStore.addReasoning('Step 1');
			streamingStore.complete();

			expect(streamingStore.getState().reasoning).toHaveLength(1);
			expect(hasActivities()).toBe(true);

			// After reset, activities are cleared
			await streamingStore.reset();
			expect(streamingStore.getState().reasoning).toHaveLength(0);
			expect(hasActivities()).toBe(false);
		});
	});

	describe('cancellation', () => {
		it('should cancel streaming', () => {
			streamingStore.appendToken('Test');
			streamingStore.cancel();

			const state = streamingStore.getState();
			expect(state.cancelled).toBe(true);
			expect(state.isStreaming).toBe(false);
		});
	});

	describe('getContent', () => {
		it('should return current content', () => {
			streamingStore.appendToken('Hello');
			streamingStore.appendToken(' World');

			expect(streamingStore.getContent()).toBe('Hello World');
		});
	});

	describe('getState', () => {
		it('should return current state snapshot', () => {
			streamingStore.appendToken('Test');
			streamingStore.addToolStart('MemoryTool');

			const state = streamingStore.getState();
			expect(state.content).toBe('Test');
			expect(state.tools).toHaveLength(1);
			expect(state.tokensReceived).toBe(1);
		});
	});

	describe('reset', () => {
		it('should reset to initial state', async () => {
			streamingStore.appendToken('Test');
			streamingStore.addToolStart('MemoryTool');
			streamingStore.addReasoning('Step 1');
			streamingStore.setError('Error');

			await streamingStore.reset();

			const state = streamingStore.getState();
			expect(state.content).toBe('');
			expect(state.tools).toEqual([]);
			expect(state.reasoning).toEqual([]);
			expect(state.error).toBe(null);
			expect(state.tokensReceived).toBe(0);
		});
	});

	describe('sub-agent initial state', () => {
		it('should have empty sub-agents initially', () => {
			const subAgents = get(activeSubAgents);
			expect(subAgents).toEqual([]);
			// Use direct checks instead of deprecated helper stores
			expect(subAgents.filter((a) => a.status === 'running')).toEqual([]);
			expect(subAgents.filter((a) => a.status === 'completed')).toEqual([]);
			expect(subAgents.filter((a) => a.status === 'error')).toEqual([]);
			expect(subAgents.some((a) => a.status === 'running')).toBe(false);
			expect(subAgents.length).toBe(0);
		});
	});

	describe('sub-agent state includes subAgents in state', () => {
		it('should include subAgents in getState()', () => {
			const state = streamingStore.getState();
			expect(state.subAgents).toBeDefined();
			expect(Array.isArray(state.subAgents)).toBe(true);
		});
	});

	describe('reset should clear sub-agents', () => {
		it('should reset sub-agents to empty array', async () => {
			await streamingStore.reset();
			const subAgents = get(activeSubAgents);
			expect(subAgents).toEqual([]);
			expect(subAgents.length > 0).toBe(false);
		});
	});
});
