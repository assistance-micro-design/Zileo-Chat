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
 * Unit tests for the shared chunk processor.
 * Tests chunk types and verifies state transformations are correct.
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import type { StreamChunk } from '$types/streaming';
import { applyChunkToState, type ChunkableState } from '../utils/chunkProcessor';

// Mock Tauri event API (required by transitive streaming import)
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {})
}));

/**
 * Creates a fresh ChunkableState for testing.
 */
function createState(): ChunkableState {
	return {
		content: '',
		tools: [],
		reasoning: [],
		subAgents: [],
		tasks: [],
		tokensReceived: 0,
		error: null
	};
}

/**
 * Helper to create a StreamChunk with defaults.
 */
function makeChunk(overrides: Partial<StreamChunk>): StreamChunk {
	return {
		workflow_id: 'wf-1',
		chunk_type: 'reasoning',
		...overrides
	};
}

describe('applyChunkToState', () => {
	let state: ChunkableState;

	beforeEach(() => {
		state = createState();
	});

	describe('tool_start', () => {
		it('should add a running tool', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'tool_start',
				tool: 'MemoryTool'
			}));

			expect(result.tools).toHaveLength(1);
			expect(result.tools[0].name).toBe('MemoryTool');
			expect(result.tools[0].status).toBe('running');
			expect(result.tools[0].startedAt).toBeDefined();
		});

		it('should default to unknown when tool name missing', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'tool_start'
			}));

			expect(result.tools[0].name).toBe('unknown');
		});
	});

	describe('tool_end', () => {
		it('should mark running tool as completed with duration', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'tool_start',
				tool: 'MemoryTool'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'tool_end',
				tool: 'MemoryTool',
				duration: 150
			}));

			expect(s.tools[0].status).toBe('completed');
			expect(s.tools[0].duration).toBe(150);
		});

		it('should not affect non-matching tools', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'tool_start',
				tool: 'ToolA'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'tool_start',
				tool: 'ToolB'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'tool_end',
				tool: 'ToolA',
				duration: 100
			}));

			expect(s.tools[0].status).toBe('completed');
			expect(s.tools[1].status).toBe('running');
		});
	});

	describe('reasoning', () => {
		it('should add reasoning step with incrementing step number', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'reasoning',
				content: 'Analyzing...'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'reasoning',
				content: 'Planning...'
			}));

			expect(s.reasoning).toHaveLength(2);
			expect(s.reasoning[0].content).toBe('Analyzing...');
			expect(s.reasoning[0].stepNumber).toBe(1);
			expect(s.reasoning[1].content).toBe('Planning...');
			expect(s.reasoning[1].stepNumber).toBe(2);
		});
	});

	describe('error', () => {
		it('should set error message', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'error',
				content: 'Connection failed'
			}));

			expect(result.error).toBe('Connection failed');
		});

		it('should default to Unknown error when content missing', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'error'
			}));

			expect(result.error).toBe('Unknown error');
		});
	});

	describe('sub_agent_start', () => {
		it('should add a running sub-agent', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Research Agent',
				parent_agent_id: 'parent-1',
				content: 'Researching topic'
			}));

			expect(result.subAgents).toHaveLength(1);
			expect(result.subAgents[0]).toMatchObject({
				id: 'sa-1',
				name: 'Research Agent',
				parentAgentId: 'parent-1',
				taskDescription: 'Researching topic',
				status: 'running',
				progress: 0
			});
		});
	});

	describe('sub_agent_progress', () => {
		it('should update progress and status message', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Agent'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'sub_agent_progress',
				sub_agent_id: 'sa-1',
				progress: 50,
				content: 'Halfway done'
			}));

			expect(s.subAgents[0].progress).toBe(50);
			expect(s.subAgents[0].statusMessage).toBe('Halfway done');
		});

		it('should not affect unrelated sub-agents', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Agent A'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-2',
				sub_agent_name: 'Agent B'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'sub_agent_progress',
				sub_agent_id: 'sa-1',
				progress: 75
			}));

			expect(s.subAgents[0].progress).toBe(75);
			expect(s.subAgents[1].progress).toBe(0);
		});
	});

	describe('sub_agent_complete', () => {
		it('should mark sub-agent as completed with metrics', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Agent'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'sub_agent_complete',
				sub_agent_id: 'sa-1',
				content: 'Final report',
				duration: 5000,
				metrics: { duration_ms: 5000, tokens_input: 100, tokens_output: 200 }
			}));

			expect(s.subAgents[0]).toMatchObject({
				status: 'completed',
				progress: 100,
				duration: 5000,
				report: 'Final report',
				metrics: { duration_ms: 5000, tokens_input: 100, tokens_output: 200 }
			});
		});
	});

	describe('sub_agent_error', () => {
		it('should mark sub-agent as errored', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sa-1',
				sub_agent_name: 'Agent'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'sub_agent_error',
				sub_agent_id: 'sa-1',
				content: 'Timeout',
				duration: 3000
			}));

			expect(s.subAgents[0]).toMatchObject({
				status: 'error',
				error: 'Timeout',
				duration: 3000
			});
		});
	});

	describe('task_create', () => {
		it('should add a new task', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'task_create',
				task_id: 't-1',
				task_name: 'Research',
				task_status: 'pending',
				task_priority: 2
			}));

			expect(result.tasks).toHaveLength(1);
			expect(result.tasks[0]).toMatchObject({
				id: 't-1',
				name: 'Research',
				status: 'pending',
				priority: 2
			});
		});

		it('should default priority to 3', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'task_create',
				task_id: 't-1',
				task_name: 'Task'
			}));

			expect(result.tasks[0].priority).toBe(3);
		});
	});

	describe('task_update', () => {
		it('should update task status', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'task_create',
				task_id: 't-1',
				task_name: 'Research'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'task_update',
				task_id: 't-1',
				task_status: 'in_progress'
			}));

			expect(s.tasks[0].status).toBe('in_progress');
		});
	});

	describe('task_complete', () => {
		it('should mark task as completed', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'task_create',
				task_id: 't-1',
				task_name: 'Research'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'task_complete',
				task_id: 't-1'
			}));

			expect(s.tasks[0].status).toBe('completed');
		});
	});

	describe('unknown chunk type', () => {
		it('should return state unchanged for unrecognized types', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'user_question_start' as StreamChunk['chunk_type']
			}));

			expect(result).toEqual(state);
		});
	});

	describe('immutability', () => {
		it('should not mutate the input state', () => {
			const original = createState();
			const frozen = { ...original };

			applyChunkToState(original, makeChunk({
				chunk_type: 'reasoning',
				content: 'test reasoning'
			}));

			expect(original.reasoning.length).toBe(frozen.reasoning.length);
			expect(original.content).toBe(frozen.content);
		});
	});

	describe('thinking_block (SA-019)', () => {
		it('should add reasoning step from thinking_block chunk', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'thinking_block',
				content: 'Let me analyze...'
			}));

			expect(result.reasoning).toHaveLength(1);
			expect(result.reasoning[0].content).toBe('Let me analyze...');
			expect(result.reasoning[0].stepNumber).toBe(1);
		});
	});

	describe('tool_call_complete (SA-019)', () => {
		it('should mark matching running tool as completed', () => {
			let s = applyChunkToState(state, makeChunk({
				chunk_type: 'tool_start',
				tool: 'SearchTool'
			}));
			s = applyChunkToState(s, makeChunk({
				chunk_type: 'tool_call_complete',
				tool: 'SearchTool',
				duration: 500,
				tool_input: '{"q":"test"}',
				tool_output: '{"r":[]}',
				tool_success: true
			}));

			expect(s.tools[0].status).toBe('completed');
			expect(s.tools[0].duration).toBe(500);
		});
	});

	describe('response_block (SA-019)', () => {
		it('should set content and token count from response_block', () => {
			const result = applyChunkToState(state, makeChunk({
				chunk_type: 'response_block',
				content: 'Final answer.',
				tokens_input: 100,
				tokens_output: 50
			}));

			expect(result.content).toBe('Final answer.');
			expect(result.tokensReceived).toBe(50);
		});
	});

	describe('extended state preservation', () => {
		it('should preserve extra fields in extended state types', () => {
			interface ExtendedState extends ChunkableState {
				workflowId: string;
				isStreaming: boolean;
			}

			const extended: ExtendedState = {
				...createState(),
				workflowId: 'wf-123',
				isStreaming: true
			};

			const result = applyChunkToState(extended, makeChunk({
				chunk_type: 'reasoning',
				content: 'Analyzing...'
			}));

			expect(result.workflowId).toBe('wf-123');
			expect(result.isStreaming).toBe(true);
			expect(result.reasoning).toHaveLength(1);
		});
	});
});
