/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Tests for executionBlocksStore - block-by-block execution display.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { get } from 'svelte/store';
import {
	executionBlocksStore,
	executionBlocks,
	isExecuting,
	spinnerContext,
	executionResponse,
	executionError,
	executionTasks
} from '../execution-blocks';
import type { StreamChunk } from '$types/streaming';
import type { ChatBlock } from '$types/chat-block';

describe('executionBlocksStore', () => {
	beforeEach(() => {
		executionBlocksStore.reset();
	});

	describe('start', () => {
		it('sets workflowId and isExecuting', () => {
			executionBlocksStore.start('wf-123');
			expect(get(isExecuting)).toBe(true);
			expect(get(executionBlocks)).toEqual([]);
			expect(get(executionResponse)).toBeNull();
		});
	});

	describe('processChunk - thinking_block', () => {
		it('adds thinking block from thinking_block chunk', () => {
			executionBlocksStore.start('wf-123');
			const chunk: StreamChunk = {
				workflow_id: 'wf-123',
				chunk_type: 'thinking_block',
				content: 'Let me analyze this problem...'
			};
			executionBlocksStore.processChunk(chunk);

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(1);
			expect(blocks[0].block_type).toBe('thinking');
			expect(blocks[0].data).toEqual({
				content: 'Let me analyze this problem...',
				source: 'model_thinking'
			});
			expect(blocks[0].sequence).toBe(1);
		});
	});

	describe('processChunk - tool_start', () => {
		it('updates spinner context on tool_start', () => {
			executionBlocksStore.start('wf-123');
			const chunk: StreamChunk = {
				workflow_id: 'wf-123',
				chunk_type: 'tool_start',
				tool: 'MemoryTool'
			};
			executionBlocksStore.processChunk(chunk);

			expect(get(spinnerContext)).toBe('MemoryTool');
		});
	});

	describe('processChunk - tool_call_complete', () => {
		it('adds tool call block from tool_call_complete chunk', () => {
			executionBlocksStore.start('wf-123');
			const chunk: StreamChunk = {
				workflow_id: 'wf-123',
				chunk_type: 'tool_call_complete',
				tool: 'SearchTool',
				duration: 1500,
				tool_input: '{"query":"test"}',
				tool_output: '{"results":[]}',
				tool_success: true
			};
			executionBlocksStore.processChunk(chunk);

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(1);
			expect(blocks[0].block_type).toBe('tool_call');
			expect(blocks[0].data).toEqual({
				tool_name: 'SearchTool',
				tool_type: 'local',
				input_params: '{"query":"test"}',
				output_result: '{"results":[]}',
				success: true,
				duration_ms: 1500
			});
		});

		it('clears spinner context after tool_call_complete', () => {
			executionBlocksStore.start('wf-123');

			// Set spinner with tool_start
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'tool_start',
				tool: 'SearchTool'
			});
			expect(get(spinnerContext)).toBe('SearchTool');

			// Complete tool
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'tool_call_complete',
				tool: 'SearchTool',
				duration: 500,
				tool_input: '{}',
				tool_output: '{}',
				tool_success: true
			});
			expect(get(spinnerContext)).toBeNull();
		});

		it('handles failed tool calls', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'tool_call_complete',
				tool: 'FailTool',
				duration: 200,
				tool_input: '{"x":1}',
				tool_output: '{"error":"timeout"}',
				tool_success: false
			});

			const blocks = get(executionBlocks);
			expect(blocks[0].data).toMatchObject({
				tool_name: 'FailTool',
				success: false
			});
		});
	});

	describe('processChunk - response_block', () => {
		it('sets response with content and tokens', () => {
			executionBlocksStore.start('wf-123');
			const chunk: StreamChunk = {
				workflow_id: 'wf-123',
				chunk_type: 'response_block',
				content: 'Here is the final response.',
				tokens_input: 150,
				tokens_output: 42
			};
			executionBlocksStore.processChunk(chunk);

			const response = get(executionResponse);
			expect(response).toEqual({
				content: 'Here is the final response.',
				tokensInput: 150,
				tokensOutput: 42
			});
		});
	});

	describe('processChunk - sub_agent_complete', () => {
		it('adds sub_agent block from sub_agent_complete chunk', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'sub_agent_complete',
				sub_agent_name: 'ResearchAgent',
				content: 'Research completed.',
				duration: 3000,
				metrics: {
					duration_ms: 3000,
					tokens_input: 500,
					tokens_output: 200
				}
			});

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(1);
			expect(blocks[0].block_type).toBe('sub_agent');
			expect(blocks[0].data).toMatchObject({
				agent_name: 'ResearchAgent',
				status: 'completed',
				duration_ms: 3000,
				tokens_input: 500,
				tokens_output: 200,
				report_summary: 'Research completed.'
			});
		});
	});

	describe('processChunk - error', () => {
		it('sets error and stops executing', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'error',
				content: 'API rate limit exceeded'
			});

			expect(get(executionError)).toBe('API rate limit exceeded');
			expect(get(isExecuting)).toBe(false);
		});
	});

	describe('sequence ordering', () => {
		it('increments sequence for each block added', () => {
			executionBlocksStore.start('wf-123');

			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'thinking_block',
				content: 'Step 1'
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'tool_call_complete',
				tool: 'Tool1',
				duration: 100,
				tool_input: '{}',
				tool_output: '{}',
				tool_success: true
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'thinking_block',
				content: 'Step 3'
			});

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(3);
			expect(blocks[0].sequence).toBe(1);
			expect(blocks[1].sequence).toBe(2);
			expect(blocks[2].sequence).toBe(3);
		});
	});

	describe('complete', () => {
		it('stops executing on complete', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.complete();

			expect(get(isExecuting)).toBe(false);
		});
	});

	describe('cancel', () => {
		it('stops executing and sets cancelled', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.cancel();

			expect(get(isExecuting)).toBe(false);
		});
	});

	describe('restoreFromBlocks', () => {
		it('restores blocks from persisted data', () => {
			const persisted: ChatBlock[] = [
				{
					block_type: 'thinking',
					sequence: 1,
					data: { content: 'Thinking...', source: 'model_thinking' }
				},
				{
					block_type: 'tool_call',
					sequence: 2,
					data: {
						tool_name: 'Search',
						tool_type: 'local',
						input_params: '{}',
						output_result: '{"results":[]}',
						success: true,
						duration_ms: 500
					}
				}
			];

			executionBlocksStore.restoreFromBlocks(persisted);

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(2);
			expect(blocks[0].block_type).toBe('thinking');
			expect(blocks[1].block_type).toBe('tool_call');
			expect(get(isExecuting)).toBe(false);
		});
	});

	describe('reset', () => {
		it('resets all state', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'thinking_block',
				content: 'test'
			});

			executionBlocksStore.reset();

			expect(get(executionBlocks)).toEqual([]);
			expect(get(isExecuting)).toBe(false);
			expect(get(spinnerContext)).toBeNull();
			expect(get(executionResponse)).toBeNull();
			expect(get(executionError)).toBeNull();
		});
	});

	describe('processChunk - reasoning', () => {
		it('adds thinking block with agent_flow source from reasoning chunk', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'reasoning',
				content: 'Analyzing the user request...'
			});

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(1);
			expect(blocks[0].block_type).toBe('thinking');
			expect(blocks[0].data).toEqual({
				content: 'Analyzing the user request...',
				source: 'agent_flow'
			});
			expect(blocks[0].sequence).toBe(1);
		});
	});

	describe('processChunk - task_create', () => {
		it('adds task to tasks array', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Analyze codebase',
				task_status: 'pending',
				task_priority: 2
			});

			const tasks = get(executionTasks);
			expect(tasks).toHaveLength(1);
			expect(tasks[0]).toMatchObject({
				id: 'task-001',
				name: 'Analyze codebase',
				status: 'pending',
				priority: 2
			});
		});

		it('adds task with agent name', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Research',
				task_status: 'pending',
				task_priority: 3,
				task_agent_name: 'ResearchAgent'
			});

			const tasks = get(executionTasks);
			expect(tasks[0].agent_name).toBe('ResearchAgent');
		});

		it('appends to existing tasks without replacing', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-002',
				task_name: 'Task 2',
				task_priority: 1
			});

			const tasks = get(executionTasks);
			expect(tasks).toHaveLength(2);
			expect(tasks[0].id).toBe('task-001');
			expect(tasks[1].id).toBe('task-002');
		});

		it('does not add blocks array entry (tasks are separate)', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});

			const blocks = get(executionBlocks);
			expect(blocks).toHaveLength(0);
		});
	});

	describe('processChunk - task_update', () => {
		it('updates task status', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_update',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_status: 'in_progress'
			});

			const tasks = get(executionTasks);
			expect(tasks[0].status).toBe('in_progress');
		});

		it('does not affect other tasks', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-002',
				task_name: 'Task 2',
				task_priority: 2
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_update',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_status: 'completed'
			});

			const tasks = get(executionTasks);
			expect(tasks[0].status).toBe('completed');
			expect(tasks[1].status).toBe('pending');
		});
	});

	describe('processChunk - task_complete', () => {
		it('marks task as completed', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_complete',
				task_id: 'task-001',
				task_name: 'Task 1',
				duration: 1500
			});

			const tasks = get(executionTasks);
			expect(tasks[0].status).toBe('completed');
			expect(tasks[0].duration_ms).toBe(1500);
		});
	});

	describe('tasks lifecycle', () => {
		it('tasks from different agents are tracked separately', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Plan',
				task_priority: 1,
				task_agent_name: 'PlannerAgent'
			});
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-002',
				task_name: 'Research',
				task_priority: 2,
				task_agent_name: 'ResearchAgent'
			});

			const tasks = get(executionTasks);
			expect(tasks).toHaveLength(2);
			expect(tasks[0].agent_name).toBe('PlannerAgent');
			expect(tasks[1].agent_name).toBe('ResearchAgent');
		});

		it('start resets tasks', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});
			expect(get(executionTasks)).toHaveLength(1);

			executionBlocksStore.start('wf-456');
			expect(get(executionTasks)).toHaveLength(0);
		});

		it('reset clears tasks', () => {
			executionBlocksStore.start('wf-123');
			executionBlocksStore.processChunk({
				workflow_id: 'wf-123',
				chunk_type: 'task_create',
				task_id: 'task-001',
				task_name: 'Task 1',
				task_priority: 3
			});

			executionBlocksStore.reset();
			expect(get(executionTasks)).toHaveLength(0);
		});
	});
});
