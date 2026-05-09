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
 * Unit tests for the shared chunk processor.
 *
 * Only the chunk types whose data is actually read by other surfaces are
 * tested:
 *   - sub_agent_*: drives the SubAgentSummary attachment in workflowExecutor
 *   - response_block / iteration_progress: drives token rollup, in-progress
 *     cost (`partialCostUsd`) and (when restored) the metrics bar.
 *
 * Tool/reasoning/task/error/thinking chunk types are now handled directly by
 * `executionBlocksStore` and intentionally NOT processed here, so they are
 * not exercised in this file.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import type { StreamChunk } from '$types/streaming';
import { applyChunkToState, type ChunkableState } from '../utils/chunkProcessor';

/**
 * Creates a fresh ChunkableState for testing.
 */
function createState(): ChunkableState {
	return {
		subAgents: [],
		tokensReceived: 0,
		tokensSent: 0,
		cachedTokens: null,
		cacheWriteTokens: null,
		partialCostUsd: null
	};
}

/**
 * Helper to create a StreamChunk with defaults.
 */
function makeChunk(overrides: Partial<StreamChunk>): StreamChunk {
	return {
		workflow_id: 'wf-1',
		chunk_type: 'response_block',
		...overrides
	};
}

describe('applyChunkToState', () => {
	let state: ChunkableState;

	beforeEach(() => {
		state = createState();
	});

	describe('sub_agent_start', () => {
		it('appends a running sub-agent', () => {
			const result = applyChunkToState(
				state,
				makeChunk({
					chunk_type: 'sub_agent_start',
					sub_agent_id: 'sa-1',
					sub_agent_name: 'Researcher',
					parent_agent_id: 'orchestrator',
					content: 'find the docs'
				})
			);

			expect(result.subAgents).toHaveLength(1);
			expect(result.subAgents[0]).toMatchObject({
				id: 'sa-1',
				name: 'Researcher',
				parentAgentId: 'orchestrator',
				taskDescription: 'find the docs',
				status: 'running',
				progress: 0
			});
		});

		it('falls back to safe defaults when fields are missing', () => {
			const result = applyChunkToState(
				state,
				makeChunk({ chunk_type: 'sub_agent_start' })
			);

			expect(result.subAgents).toHaveLength(1);
			expect(result.subAgents[0]).toMatchObject({
				id: 'unknown',
				name: 'Unknown Agent',
				parentAgentId: '',
				taskDescription: ''
			});
		});
	});

	describe('sub_agent_complete', () => {
		it('marks the matching sub-agent as completed with metrics', () => {
			const startState = applyChunkToState(
				state,
				makeChunk({
					chunk_type: 'sub_agent_start',
					sub_agent_id: 'sa-2',
					sub_agent_name: 'Coder'
				})
			);

			const result = applyChunkToState(
				startState,
				makeChunk({
					chunk_type: 'sub_agent_complete',
					sub_agent_id: 'sa-2',
					content: 'done',
					duration: 1234,
					metrics: { duration_ms: 1234, tokens_input: 50, tokens_output: 25 }
				})
			);

			expect(result.subAgents[0]).toMatchObject({
				id: 'sa-2',
				status: 'completed',
				progress: 100,
				duration: 1234,
				report: 'done',
				metrics: { duration_ms: 1234, tokens_input: 50, tokens_output: 25 }
			});
		});

		it('leaves other sub-agents untouched', () => {
			let s = applyChunkToState(
				state,
				makeChunk({ chunk_type: 'sub_agent_start', sub_agent_id: 'sa-a' })
			);
			s = applyChunkToState(
				s,
				makeChunk({ chunk_type: 'sub_agent_start', sub_agent_id: 'sa-b' })
			);

			const result = applyChunkToState(
				s,
				makeChunk({ chunk_type: 'sub_agent_complete', sub_agent_id: 'sa-b' })
			);

			expect(result.subAgents.find((a) => a.id === 'sa-a')?.status).toBe('running');
			expect(result.subAgents.find((a) => a.id === 'sa-b')?.status).toBe('completed');
		});
	});

	describe('sub_agent_error', () => {
		it('marks the matching sub-agent as errored', () => {
			const startState = applyChunkToState(
				state,
				makeChunk({ chunk_type: 'sub_agent_start', sub_agent_id: 'sa-x' })
			);

			const result = applyChunkToState(
				startState,
				makeChunk({
					chunk_type: 'sub_agent_error',
					sub_agent_id: 'sa-x',
					content: 'API down',
					duration: 200
				})
			);

			expect(result.subAgents[0]).toMatchObject({
				id: 'sa-x',
				status: 'error',
				error: 'API down',
				duration: 200
			});
		});
	});

	describe('response_block', () => {
		it('updates token rollup from the chunk', () => {
			const result = applyChunkToState(
				state,
				makeChunk({
					chunk_type: 'response_block',
					tokens_input: 120,
					tokens_output: 45,
					cached_tokens: 30,
					cache_write_tokens: 10
				})
			);

			expect(result.tokensSent).toBe(120);
			expect(result.tokensReceived).toBe(45);
			expect(result.cachedTokens).toBe(30);
			expect(result.cacheWriteTokens).toBe(10);
		});

		it('accumulates cost_usd into partialCostUsd', () => {
			const first = applyChunkToState(
				state,
				makeChunk({ chunk_type: 'response_block', cost_usd: 0.0012 })
			);
			expect(first.partialCostUsd).toBeCloseTo(0.0012);

			const second = applyChunkToState(
				first,
				makeChunk({ chunk_type: 'response_block', cost_usd: 0.0008 })
			);
			expect(second.partialCostUsd).toBeCloseTo(0.002);
		});

		it('preserves existing token counts when fields are absent', () => {
			const seeded: ChunkableState = {
				...state,
				tokensReceived: 99,
				tokensSent: 100,
				cachedTokens: 5,
				cacheWriteTokens: 1
			};

			const result = applyChunkToState(
				seeded,
				makeChunk({ chunk_type: 'response_block' })
			);
			expect(result.tokensReceived).toBe(99);
			expect(result.tokensSent).toBe(100);
			expect(result.cachedTokens).toBe(5);
			expect(result.cacheWriteTokens).toBe(1);
		});
	});

	describe('iteration_progress', () => {
		it('updates cumulative tokens and accumulates cost', () => {
			const result = applyChunkToState(
				state,
				makeChunk({
					chunk_type: 'iteration_progress',
					tokens_input: 200,
					tokens_output: 80,
					cached_tokens: 12,
					cache_write_tokens: 3,
					cost_usd: 0.005
				})
			);

			expect(result.tokensSent).toBe(200);
			expect(result.tokensReceived).toBe(80);
			expect(result.cachedTokens).toBe(12);
			expect(result.cacheWriteTokens).toBe(3);
			expect(result.partialCostUsd).toBeCloseTo(0.005);
		});

		it('ignores chunks flagged is_sub_agent', () => {
			const seeded: ChunkableState = {
				...state,
				tokensSent: 50,
				tokensReceived: 25,
				partialCostUsd: 0.001
			};

			const result = applyChunkToState(
				seeded,
				makeChunk({
					chunk_type: 'iteration_progress',
					tokens_input: 999,
					tokens_output: 999,
					cost_usd: 0.999,
					is_sub_agent: true
				})
			);

			expect(result.tokensSent).toBe(50);
			expect(result.tokensReceived).toBe(25);
			expect(result.partialCostUsd).toBeCloseTo(0.001);
		});
	});

	describe('unrecognized chunk types', () => {
		it('returns the state unchanged when no handler matches', () => {
			const seeded: ChunkableState = {
				...state,
				tokensSent: 7,
				partialCostUsd: 0.0001
			};

			// `tool_start`, `reasoning`, `error`, `task_*`, `thinking_block`,
			// `tool_call_complete` are intentionally unhandled in the new
			// chunk processor — they belong to executionBlocksStore.
			const result = applyChunkToState(
				seeded,
				makeChunk({ chunk_type: 'tool_start', tool: 'MemoryTool' })
			);

			expect(result).toBe(seeded);
		});
	});
});
