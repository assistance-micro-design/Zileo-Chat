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
 * Unit tests for the streaming store.
 * Tests production code paths: start, processChunkDirect, processCompleteDirect,
 * restoreFrom, reset, cleanup.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import type { StreamChunk, WorkflowComplete } from '$types/streaming';

// Mock Tauri's event API
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {})
}));

import { streamingStore, activeSubAgents } from '../streaming';

describe('streamingStore', () => {
	beforeEach(async () => {
		streamingStore.reset();
	});

	describe('initial state', () => {
		it('should have correct initial values', () => {
			const state = get(streamingStore);
			expect(state.isStreaming).toBe(false);
			expect(state.content).toBe('');
			expect(state.tools).toEqual([]);
			expect(state.reasoning).toEqual([]);
			expect(state.subAgents).toEqual([]);
			expect(state.tasks).toEqual([]);
			expect(state.error).toBe(null);
			expect(state.cancelled).toBe(false);
			expect(state.tokensReceived).toBe(0);
			expect(state.workflowId).toBe(null);
		});
	});

	describe('start', () => {
		it('should set workflowId and mark as streaming', async () => {
			await streamingStore.start('wf-1');
			const state = get(streamingStore);
			expect(state.workflowId).toBe('wf-1');
			expect(state.isStreaming).toBe(true);
			expect(state.content).toBe('');
		});

		it('should reset previous state when starting new workflow', async () => {
			streamingStore.processChunkDirect({
				chunk_type: 'reasoning',
				content: 'old reasoning'
			} as StreamChunk);

			await streamingStore.start('wf-2');
			const state = get(streamingStore);
			expect(state.reasoning).toEqual([]);
			expect(state.workflowId).toBe('wf-2');
		});
	});

	describe('processChunkDirect', () => {
		it('should append reasoning steps', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'reasoning',
				content: 'Analyzing request...'
			} as StreamChunk);

			streamingStore.processChunkDirect({
				chunk_type: 'reasoning',
				content: 'Planning response...'
			} as StreamChunk);

			const state = get(streamingStore);
			expect(state.reasoning).toHaveLength(2);
			expect(state.reasoning[0].content).toBe('Analyzing request...');
			expect(state.reasoning[0].stepNumber).toBe(1);
			expect(state.reasoning[1].content).toBe('Planning response...');
			expect(state.reasoning[1].stepNumber).toBe(2);
		});

		it('should track tool start and end', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'tool_start',
				tool: 'MemoryTool'
			} as StreamChunk);

			let state = get(streamingStore);
			expect(state.tools).toHaveLength(1);
			expect(state.tools[0].name).toBe('MemoryTool');
			expect(state.tools[0].status).toBe('running');

			streamingStore.processChunkDirect({
				chunk_type: 'tool_call_complete',
				tool: 'MemoryTool',
				duration: 150
			} as StreamChunk);

			state = get(streamingStore);
			expect(state.tools[0].status).toBe('completed');
			expect(state.tools[0].duration).toBe(150);
		});

		it('should track multiple tools independently', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'tool_start',
				tool: 'MemoryTool'
			} as StreamChunk);
			streamingStore.processChunkDirect({
				chunk_type: 'tool_start',
				tool: 'TodoTool'
			} as StreamChunk);
			streamingStore.processChunkDirect({
				chunk_type: 'tool_call_complete',
				tool: 'MemoryTool',
				duration: 100
			} as StreamChunk);

			const state = get(streamingStore);
			const running = state.tools.filter((t) => t.status === 'running');
			const completed = state.tools.filter((t) => t.status === 'completed');

			expect(running).toHaveLength(1);
			expect(running[0].name).toBe('TodoTool');
			expect(completed).toHaveLength(1);
			expect(completed[0].name).toBe('MemoryTool');
		});

		it('should set error and stop streaming on error chunk', async () => {
			await streamingStore.start('wf-1');

			streamingStore.processChunkDirect({
				chunk_type: 'error',
				content: 'Network error'
			} as StreamChunk);

			const state = get(streamingStore);
			expect(state.error).toBe('Network error');
			expect(state.isStreaming).toBe(false);
		});

		it('should handle sub-agent lifecycle', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Research Agent',
				parent_agent_id: 'agent-1',
				content: 'Research task'
			} as StreamChunk);

			let state = get(streamingStore);
			expect(state.subAgents).toHaveLength(1);
			expect(state.subAgents[0].name).toBe('Research Agent');
			expect(state.subAgents[0].status).toBe('running');

			streamingStore.processChunkDirect({
				chunk_type: 'sub_agent_complete',
				sub_agent_id: 'sa-1',
				content: 'Research complete',
				duration: 5000
			} as StreamChunk);

			state = get(streamingStore);
			expect(state.subAgents[0].status).toBe('completed');
			expect(state.subAgents[0].progress).toBe(100);
		});

		it('should handle task lifecycle', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'task_create',
				task_id: 't-1',
				task_name: 'Analyze data',
				task_status: 'pending',
				task_priority: 2
			} as StreamChunk);

			let state = get(streamingStore);
			expect(state.tasks).toHaveLength(1);
			expect(state.tasks[0].id).toBe('t-1');
			expect(state.tasks[0].name).toBe('Analyze data');

			streamingStore.processChunkDirect({
				chunk_type: 'task_complete',
				task_id: 't-1'
			} as StreamChunk);

			state = get(streamingStore);
			expect(state.tasks[0].status).toBe('completed');
		});
	});

	describe('processCompleteDirect', () => {
		it('should mark as completed on success', async () => {
			await streamingStore.start('wf-1');

			streamingStore.processCompleteDirect({
				workflow_id: 'wf-1',
				status: 'completed'
			} as WorkflowComplete);

			const state = get(streamingStore);
			expect(state.completed).toBe(true);
			expect(state.isStreaming).toBe(true); // isStreaming stays true until reset
		});

		it('should set error and stop streaming on error completion', async () => {
			await streamingStore.start('wf-1');

			streamingStore.processCompleteDirect({
				workflow_id: 'wf-1',
				status: 'error',
				error: 'Backend failure'
			} as WorkflowComplete);

			const state = get(streamingStore);
			expect(state.completed).toBe(true);
			expect(state.error).toBe('Backend failure');
			expect(state.isStreaming).toBe(false);
		});

		it('should mark as cancelled on cancellation', async () => {
			await streamingStore.start('wf-1');

			streamingStore.processCompleteDirect({
				workflow_id: 'wf-1',
				status: 'cancelled'
			} as WorkflowComplete);

			const state = get(streamingStore);
			expect(state.completed).toBe(true);
			expect(state.cancelled).toBe(true);
			expect(state.isStreaming).toBe(false);
		});
	});

	describe('restoreFrom', () => {
		it('should restore state from background workflow', () => {
			streamingStore.restoreFrom({
				workflowId: 'wf-bg',
				content: 'restored content',
				tools: [{ name: 'Tool1', status: 'completed', startedAt: 1000, duration: 50 }],
				reasoning: [{ content: 'step 1', timestamp: 1000, stepNumber: 1 }],
				subAgents: [],
				tasks: [],
				tokensReceived: 42,
				error: null,
				status: 'running'
			});

			const state = get(streamingStore);
			expect(state.workflowId).toBe('wf-bg');
			expect(state.content).toBe('restored content');
			expect(state.tools).toHaveLength(1);
			expect(state.reasoning).toHaveLength(1);
			expect(state.tokensReceived).toBe(42);
			expect(state.isStreaming).toBe(true);
			expect(state.completed).toBe(false);
		});

		it('should restore completed state correctly', () => {
			streamingStore.restoreFrom({
				workflowId: 'wf-done',
				content: 'done',
				tools: [],
				reasoning: [],
				subAgents: [],
				tasks: [],
				tokensReceived: 10,
				error: null,
				status: 'completed'
			});

			const state = get(streamingStore);
			expect(state.isStreaming).toBe(false);
			expect(state.completed).toBe(true);
			expect(state.cancelled).toBe(false);
		});

		it('should restore cancelled state correctly', () => {
			streamingStore.restoreFrom({
				workflowId: 'wf-cancel',
				content: '',
				tools: [],
				reasoning: [],
				subAgents: [],
				tasks: [],
				tokensReceived: 0,
				error: null,
				status: 'cancelled'
			});

			const state = get(streamingStore);
			expect(state.isStreaming).toBe(false);
			expect(state.cancelled).toBe(true);
		});
	});

	describe('reset', () => {
		it('should reset to initial state', async () => {
			await streamingStore.start('wf-1');
			streamingStore.processChunkDirect({
				chunk_type: 'reasoning',
				content: 'Step 1'
			} as StreamChunk);
			streamingStore.processChunkDirect({
				chunk_type: 'tool_start',
				tool: 'MemoryTool'
			} as StreamChunk);
			streamingStore.processChunkDirect({
				chunk_type: 'error',
				content: 'Error'
			} as StreamChunk);

			streamingStore.reset();

			const state = get(streamingStore);
			expect(state.content).toBe('');
			expect(state.tools).toEqual([]);
			expect(state.reasoning).toEqual([]);
			expect(state.error).toBe(null);
			expect(state.tokensReceived).toBe(0);
			expect(state.workflowId).toBe(null);
		});
	});

	describe('activeSubAgents derived store', () => {
		it('should have empty sub-agents initially', () => {
			const subAgents = get(activeSubAgents);
			expect(subAgents).toEqual([]);
		});

		it('should reflect sub-agents from processChunkDirect', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Agent A',
				parent_agent_id: 'p-1',
				content: 'task'
			} as StreamChunk);

			const subAgents = get(activeSubAgents);
			expect(subAgents).toHaveLength(1);
			expect(subAgents[0].name).toBe('Agent A');
		});

		it('should be cleared after reset', () => {
			streamingStore.processChunkDirect({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Agent A',
				parent_agent_id: 'p-1',
				content: 'task'
			} as StreamChunk);

			streamingStore.reset();
			expect(get(activeSubAgents)).toEqual([]);
		});
	});
});
