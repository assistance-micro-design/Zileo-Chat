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

import {
	streamingStore,
	isStreaming,
	streamContent,
	activeTools,
	reasoningSteps,
	streamError,
	isCancelled,
	isCompleted,
	hasStreamingActivities,
	tokensReceived,
	runningTools,
	completedTools,
	activeSubAgents,
	runningSubAgents,
	completedSubAgents,
	erroredSubAgents,
	hasRunningSubAgents,
	subAgentCount,
	hasActiveSubAgents
} from '../streaming';

describe('streamingStore', () => {
	beforeEach(async () => {
		// Reset store before each test
		await streamingStore.reset();
	});

	describe('initial state', () => {
		it('should have correct initial values', () => {
			expect(get(isStreaming)).toBe(false);
			expect(get(streamContent)).toBe('');
			expect(get(activeTools)).toEqual([]);
			expect(get(reasoningSteps)).toEqual([]);
			expect(get(streamError)).toBe(null);
			expect(get(isCancelled)).toBe(false);
			expect(get(tokensReceived)).toBe(0);
		});
	});

	describe('appendToken', () => {
		it('should append tokens to content', () => {
			streamingStore.appendToken('Hello');
			expect(get(streamContent)).toBe('Hello');

			streamingStore.appendToken(' World');
			expect(get(streamContent)).toBe('Hello World');
		});

		it('should increment token count', () => {
			streamingStore.appendToken('a');
			streamingStore.appendToken('b');
			streamingStore.appendToken('c');
			expect(get(tokensReceived)).toBe(3);
		});
	});

	describe('tool tracking', () => {
		it('should track tool start', () => {
			streamingStore.addToolStart('MemoryTool');

			const tools = get(activeTools);
			expect(tools).toHaveLength(1);
			expect(tools[0].name).toBe('MemoryTool');
			expect(tools[0].status).toBe('running');
			expect(tools[0].startedAt).toBeDefined();
		});

		it('should complete tool with duration', () => {
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 150);

			const tools = get(activeTools);
			expect(tools[0].status).toBe('completed');
			expect(tools[0].duration).toBe(150);
		});

		it('should track multiple tools', () => {
			streamingStore.addToolStart('MemoryTool');
			streamingStore.addToolStart('TodoTool');
			streamingStore.completeToolEnd('MemoryTool', 100);

			const running = get(runningTools);
			const completed = get(completedTools);

			expect(running).toHaveLength(1);
			expect(running[0].name).toBe('TodoTool');
			expect(completed).toHaveLength(1);
			expect(completed[0].name).toBe('MemoryTool');
		});

		it('should fail tool with error', () => {
			streamingStore.addToolStart('MemoryTool');
			streamingStore.failTool('MemoryTool', 'Connection failed');

			const tools = get(activeTools);
			expect(tools[0].status).toBe('error');
			expect(tools[0].error).toBe('Connection failed');
		});
	});

	describe('reasoning steps', () => {
		it('should add reasoning steps', () => {
			streamingStore.addReasoning('Analyzing request...');
			streamingStore.addReasoning('Planning response...');

			const steps = get(reasoningSteps);
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

			expect(get(streamError)).toBe('Network error');
			expect(get(isStreaming)).toBe(false);
		});
	});

	describe('completion', () => {
		it('should mark as completed while keeping streaming activities visible', () => {
			streamingStore.appendToken('Test');
			streamingStore.addToolStart('MemoryTool');
			streamingStore.complete();

			// isStreaming stays true until reset, but completed is set
			expect(get(isCompleted)).toBe(true);
			// hasStreamingActivities should be true because we have activities
			expect(get(hasStreamingActivities)).toBe(true);
		});

		it('should keep activities visible until explicitly reset', async () => {
			streamingStore.appendToken('Test');
			streamingStore.addReasoning('Step 1');
			streamingStore.complete();

			// Activities should still be accessible
			expect(get(reasoningSteps)).toHaveLength(1);
			expect(get(hasStreamingActivities)).toBe(true);

			// After reset, activities are cleared
			await streamingStore.reset();
			expect(get(reasoningSteps)).toHaveLength(0);
			expect(get(hasStreamingActivities)).toBe(false);
		});
	});

	describe('cancellation', () => {
		it('should cancel streaming', () => {
			streamingStore.appendToken('Test');
			streamingStore.cancel();

			expect(get(isCancelled)).toBe(true);
			expect(get(isStreaming)).toBe(false);
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

			expect(get(streamContent)).toBe('');
			expect(get(activeTools)).toEqual([]);
			expect(get(reasoningSteps)).toEqual([]);
			expect(get(streamError)).toBe(null);
			expect(get(tokensReceived)).toBe(0);
		});
	});

	describe('sub-agent initial state', () => {
		it('should have empty sub-agents initially', () => {
			expect(get(activeSubAgents)).toEqual([]);
			expect(get(runningSubAgents)).toEqual([]);
			expect(get(completedSubAgents)).toEqual([]);
			expect(get(erroredSubAgents)).toEqual([]);
			expect(get(hasRunningSubAgents)).toBe(false);
			expect(get(subAgentCount)).toBe(0);
			expect(get(hasActiveSubAgents)).toBe(false);
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
			expect(get(activeSubAgents)).toEqual([]);
			expect(get(hasActiveSubAgents)).toBe(false);
		});
	});
});
