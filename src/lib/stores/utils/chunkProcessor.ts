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
 * Shared chunk processor for stream chunk state updates.
 *
 * Extracts the common chunk-to-state-delta logic used by both
 * streaming.ts and backgroundWorkflows.ts into a single source of truth.
 *
 * @module stores/utils/chunkProcessor
 */

import type { StreamChunk } from '$types/streaming';
import type { ActiveTool, ActiveReasoningStep, ActiveSubAgent, ActiveTask } from '../streaming';

/**
 * Common state fields that can be updated by stream chunks.
 * Both StreamingState and WorkflowStreamState extend this shape.
 *
 * Includes input/cache token fields so that switching back to a still-running
 * background workflow restores the full session display from its bg state,
 * not only the output count. `partialCostUsd` accumulates the per-iteration
 * backend-computed cost so the in-progress display stays accurate without
 * the frontend multiplying tokens by prices itself.
 */
export interface ChunkableState {
	content: string;
	tools: ActiveTool[];
	reasoning: ActiveReasoningStep[];
	subAgents: ActiveSubAgent[];
	tasks: ActiveTask[];
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
	error: string | null;
}

/**
 * Handler function signature for processing a chunk type.
 * Operates on ChunkableState base type; callers merge the result
 * back into their extended state to preserve extra fields.
 */
type ChunkHandler = (state: ChunkableState, chunk: StreamChunk) => ChunkableState;

/**
 * Handle tool_start chunk - add new tool with running status.
 */
function handleToolStart(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		tools: [
			...s.tools,
			{
				name: c.tool ?? 'unknown',
				status: 'running' as const,
				startedAt: Date.now()
			}
		]
	};
}

/**
 * Handle reasoning chunk - add new reasoning step.
 */
function handleReasoning(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		reasoning: [
			...s.reasoning,
			{
				content: c.content ?? '',
				timestamp: Date.now(),
				stepNumber: s.reasoning.length + 1
			}
		]
	};
}

/**
 * Handle error chunk - set error message.
 *
 * Note: streaming.ts adds isStreaming=false on top of this.
 */
function handleError(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		error: c.content ?? 'Unknown error'
	};
}

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
 * Handle task_create chunk - add new task.
 */
function handleTaskCreate(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		tasks: [
			...s.tasks,
			{
				id: c.task_id ?? '',
				name: c.task_name ?? '',
				status: (c.task_status ?? 'pending') as ActiveTask['status'],
				priority: c.task_priority ?? 3,
				createdAt: Date.now(),
				updatedAt: Date.now()
			}
		]
	};
}

/**
 * Handle task_update chunk - update task status.
 */
function handleTaskUpdate(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		tasks: s.tasks.map((t) =>
			t.id === c.task_id
				? { ...t, status: c.task_status as ActiveTask['status'], updatedAt: Date.now() }
				: t
		)
	};
}

/**
 * Handle task_complete chunk - mark task as completed.
 */
function handleTaskComplete(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		tasks: s.tasks.map((t) =>
			t.id === c.task_id ? { ...t, status: 'completed' as const, updatedAt: Date.now() } : t
		)
	};
}

/**
 * Handle thinking_block chunk - add as reasoning step (backward compat).
 * The executionBlocksStore handles the full block display separately.
 */
function handleThinkingBlock(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		reasoning: [
			...s.reasoning,
			{
				content: c.content ?? '',
				timestamp: Date.now(),
				stepNumber: s.reasoning.length + 1
			}
		]
	};
}

/**
 * Handle tool_call_complete chunk - mark tool as completed (backward compat).
 * The executionBlocksStore handles the full block display separately.
 */
function handleToolCallComplete(s: ChunkableState, c: StreamChunk): ChunkableState {
	return {
		...s,
		tools: s.tools.map((t) =>
			t.name === c.tool && t.status === 'running'
				? { ...t, status: 'completed' as const, duration: c.duration }
				: t
		)
	};
}

/**
 * Handle response_block chunk - set final content and token counts (backward compat).
 * The executionBlocksStore handles the full response display separately.
 *
 * Also persists `tokens_input`, `cached_tokens` and `cache_write_tokens` so a
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
		content: c.content ?? s.content,
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
 */
function handleIterationProgress(s: ChunkableState, c: StreamChunk): ChunkableState {
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
 */
const chunkHandlers: Record<string, ChunkHandler> = {
	tool_start: handleToolStart,
	reasoning: handleReasoning,
	error: handleError,
	sub_agent_start: handleSubAgentStart,
	sub_agent_complete: handleSubAgentComplete,
	sub_agent_error: handleSubAgentError,
	task_create: handleTaskCreate,
	task_update: handleTaskUpdate,
	task_complete: handleTaskComplete,
	thinking_block: handleThinkingBlock,
	tool_call_complete: handleToolCallComplete,
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
	// Merge handler result onto original state to preserve extra fields (e.g. isStreaming, workflowId)
	return { ...state, ...baseResult };
}
