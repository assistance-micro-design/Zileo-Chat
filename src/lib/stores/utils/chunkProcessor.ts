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
 * Chunk processor for stream-chunk state updates consumed by
 * `backgroundWorkflowsStore`.
 *
 * After the streamingStore removal (2026-05-09), only the data actually
 * read by other surfaces is tracked here:
 *   - token rollup (`tokensReceived`, `tokensSent`, `cachedTokens`,
 *     `cacheWriteTokens`)
 *   - in-progress cost (`partialCostUsd`)
 *   - sub-agent activity (`subAgents`) consumed by `workflowExecutor` to
 *     attach `SubAgentSummary` items to assistant messages.
 *
 * Tools, reasoning steps, tasks, errors and content are owned by
 * `executionBlocksStore` and `tokenStore`, not duplicated here.
 *
 * @module stores/utils/chunkProcessor
 */

import type { StreamChunk } from '$types/streaming';
import type { ActiveSubAgent } from '$types/background-workflow';

/**
 * Common state fields that can be updated by stream chunks.
 *
 * Both `WorkflowStreamState` and `RestoreFromChunksState` shapes extend this
 * minimal contract so the processor can run against either.
 */
export interface ChunkableState {
	subAgents: ActiveSubAgent[];
	tokensReceived: number;
	tokensSent: number;
	cachedTokens: number | null;
	cacheWriteTokens: number | null;
	/**
	 * Sum of `cost_usd` carried by every `response_block` chunk seen so far.
	 * `null` until the first chunk with a cost lands. The backend is the
	 * single source of truth for these numbers.
	 */
	partialCostUsd: number | null;
}

/**
 * Handler function signature for processing a chunk type.
 */
type ChunkHandler = (state: ChunkableState, chunk: StreamChunk) => ChunkableState;

/**
 * Handle sub_agent_start chunk - add new sub-agent with running status.
 */
function handleSubAgentStart(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		subAgents: [
			...s.subAgents,
			{
				id: c.sub_agent_id ?? 'unknown',
				name: c.sub_agent_name ?? 'Unknown Agent',
				parentAgentId: c.parent_agent_id ?? '',
				taskDescription: c.content ?? '',
				status: 'running' as const,
				startedAt: Date.now(),
				progress: 0
			}
		]
	};
}

/**
 * Handle sub_agent_complete chunk - mark sub-agent as completed with metrics.
 */
function handleSubAgentComplete(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		subAgents: s.subAgents.map((a) =>
			a.id === c.sub_agent_id
				? {
						...a,
						status: 'completed' as const,
						progress: 100,
						duration: c.duration,
						report: c.content,
						metrics: c.metrics
					}
				: a
		)
	};
}

/**
 * Handle sub_agent_error chunk - mark sub-agent as errored.
 */
function handleSubAgentError(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		subAgents: s.subAgents.map((a) =>
			a.id === c.sub_agent_id
				? {
						...a,
						status: 'error' as const,
						error: c.content ?? 'Unknown error',
						duration: c.duration
					}
				: a
		)
	};
}

/**
 * Handle response_block chunk - update token rollup and accumulate cost.
 *
 * Persists `tokens_input`, `cached_tokens` and `cache_write_tokens` so a
 * switch back to a still-running bg workflow can restore the full session
 * display, not just the output count.
 *
 * Accumulates `cost_usd` from each chunk into `partialCostUsd`. The cost is
 * computed by the backend pricing layer; the frontend only sums values it
 * receives — never multiplies tokens × prices.
 */
function handleResponseBlock(s: ChunkableState, c: StreamChunk): ChunkableState {
	const next: ChunkableState = {
		...s,
		tokensReceived: c.tokens_output ?? s.tokensReceived,
		tokensSent: c.tokens_input ?? s.tokensSent,
		cachedTokens: c.cached_tokens ?? s.cachedTokens,
		cacheWriteTokens: c.cache_write_tokens ?? s.cacheWriteTokens,
		partialCostUsd: s.partialCostUsd
	};
	if (typeof c.cost_usd === 'number') {
		next.partialCostUsd = (s.partialCostUsd ?? 0) + c.cost_usd;
	}
	return next;
}

/**
 * Handle iteration_progress chunk - cumulative tokens reported after every
 * LLM call inside the tool loop. Same shape as response_block (carries
 * tokens_input/output/cached/cache_write + optional cost_usd) but without
 * content. Symmetric with handleResponseBlock so the metrics bar updates
 * live during streaming, not only after completion.
 *
 * Chunks flagged `is_sub_agent: true` are ignored: a delegated agent runs
 * its own TokenTracker (resets to 0) and its cumulative chunk would stomp
 * the orchestrator's running totals. Sub-agent token rollup happens
 * server-side via aggregate_sub_agent_metrics and surfaces separately.
 */
function handleIterationProgress(s: ChunkableState, c: StreamChunk): ChunkableState {
	if (c.is_sub_agent) {
		return s;
	}
	const next: ChunkableState = {
		...s,
		tokensReceived: c.tokens_output ?? s.tokensReceived,
		tokensSent: c.tokens_input ?? s.tokensSent,
		cachedTokens: c.cached_tokens ?? s.cachedTokens,
		cacheWriteTokens: c.cache_write_tokens ?? s.cacheWriteTokens,
		partialCostUsd: s.partialCostUsd
	};
	if (typeof c.cost_usd === 'number') {
		next.partialCostUsd = (s.partialCostUsd ?? 0) + c.cost_usd;
	}
	return next;
}

/**
 * Registry mapping chunk types to their handler functions.
 *
 * Only the chunk types whose data is consumed by other surfaces are kept.
 * Tool start/complete, reasoning, thinking_block, error and task_* chunks
 * are handled by `executionBlocksStore` directly and intentionally ignored
 * here.
 */
const chunkHandlers: Record<string, ChunkHandler> = {
	sub_agent_start: handleSubAgentStart,
	sub_agent_complete: handleSubAgentComplete,
	sub_agent_error: handleSubAgentError,
	response_block: handleResponseBlock,
	iteration_progress: handleIterationProgress
};

/**
 * Applies a stream chunk to any state that extends ChunkableState.
 * Returns the updated state without mutating the input.
 *
 * This is a pure function (no side-effects). Store-specific effects
 * (e.g. tokenStore sync, isStreaming flag) must be handled by the caller.
 *
 * Unrecognized chunk types are silently ignored (state returned as-is).
 *
 * @param state - Current state (must extend ChunkableState)
 * @param chunk - Incoming stream chunk
 * @returns Updated state
 */
export function applyChunkToState<T extends ChunkableState>(state: T, chunk: StreamChunk): T {
	const handler = chunkHandlers[chunk.chunk_type];
	if (!handler) return state;
	const baseResult = handler(state, chunk);
	// Merge handler result onto original state to preserve extra fields
	// (e.g. workflowId, status, chunkHistory).
	return { ...state, ...baseResult };
}
