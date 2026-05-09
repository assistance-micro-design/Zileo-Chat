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
 * Execution blocks store for managing block-by-block execution display.
 *
 * Replaces the streaming store's content accumulation with discrete blocks
 * (thinking, tool calls, sub-agents) that appear as each step completes.
 *
 * @module stores/execution-blocks
 */

import { writable, derived } from 'svelte/store';
import type { StreamChunk } from '$types/streaming';
import type { ChatBlock, ChatBlockType, ThinkingBlockData, ToolCallBlockData, SubAgentBlockData, TodoTaskDisplay } from '$types/chat-block';

/**
 * State interface for the execution blocks store.
 */
export interface ExecutionBlocksState {
	/** Currently executing workflow ID */
	workflowId: string | null;
	/** Blocks received in real-time */
	blocks: ChatBlock[];
	/** Active tasks from TodoTool (separate from blocks, displayed after spinner) */
	tasks: TodoTaskDisplay[];
	/** Whether execution is currently active */
	isExecuting: boolean;
	/** Contextual spinner text (e.g., "MemoryTool") */
	spinnerContext: string | null;
	/** Auto-incrementing sequence counter */
	nextSequence: number;
}

const initialState: ExecutionBlocksState = {
	workflowId: null,
	blocks: [],
	tasks: [],
	isExecuting: false,
	spinnerContext: null,
	nextSequence: 1
};

const store = writable<ExecutionBlocksState>(initialState);

/**
 * Create a ChatBlock with the given type and data.
 */
function createBlock(
	type: ChatBlockType,
	sequence: number,
	data: ThinkingBlockData | ToolCallBlockData | SubAgentBlockData
): ChatBlock {
	return { block_type: type, sequence, data };
}

/**
 * Process a thinking_block chunk into a ThinkingBlock.
 */
function handleThinkingBlock(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	const data: ThinkingBlockData = {
		content: chunk.content ?? '',
		source: 'model_thinking'
	};
	const block = createBlock('thinking', state.nextSequence, data);
	return {
		...state,
		blocks: [...state.blocks, block],
		nextSequence: state.nextSequence + 1,
		spinnerContext: null
	};
}

/**
 * Process a tool_start chunk - update spinner context.
 */
function handleToolStart(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	return {
		...state,
		spinnerContext: chunk.tool ?? null
	};
}

/**
 * Process a tool_call_complete chunk into a ToolCallBlock.
 */
function handleToolCallComplete(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	const data: ToolCallBlockData = {
		tool_name: chunk.tool ?? 'unknown',
		tool_type: chunk.tool_type ?? 'local',
		server_name: chunk.server_name,
		input_params: chunk.tool_input ?? '{}',
		output_result: chunk.tool_output ?? '{}',
		success: chunk.tool_success ?? false,
		duration_ms: chunk.duration ?? 0
	};
	const block = createBlock('tool_call', state.nextSequence, data);
	return {
		...state,
		blocks: [...state.blocks, block],
		nextSequence: state.nextSequence + 1,
		spinnerContext: null
	};
}

/**
 * Process a response_block chunk - clear the active spinner.
 *
 * The chunk content/tokens are consumed by tokenStore and bgWorkflows; this
 * store only needs to drop the spinner so the response renders cleanly.
 */
function handleResponseBlock(state: ExecutionBlocksState, _chunk: StreamChunk): ExecutionBlocksState {
	return {
		...state,
		spinnerContext: null
	};
}

/**
 * Process a sub_agent_complete chunk into a SubAgentBlock.
 */
function handleSubAgentComplete(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	// Dedup: skip if a sub_agent block with the same sub_agent_id already exists
	const subAgentId = chunk.sub_agent_id;
	if (subAgentId) {
		const alreadyExists = state.blocks.some(
			(b) => b.block_type === 'sub_agent' && (b.data as SubAgentBlockData)._sub_agent_id === subAgentId
		);
		if (alreadyExists) return { ...state, spinnerContext: null };
	}

	const data: SubAgentBlockData = {
		agent_name: chunk.sub_agent_name ?? 'Unknown Agent',
		status: 'completed',
		duration_ms: chunk.duration ?? chunk.metrics?.duration_ms,
		tokens_input: chunk.metrics?.tokens_input,
		tokens_output: chunk.metrics?.tokens_output,
		report_summary: chunk.content,
		_sub_agent_id: subAgentId
	};
	const block = createBlock('sub_agent', state.nextSequence, data);
	return {
		...state,
		blocks: [...state.blocks, block],
		nextSequence: state.nextSequence + 1,
		spinnerContext: null
	};
}

/**
 * Process a sub_agent_error chunk into a SubAgentBlock with error status.
 */
function handleSubAgentError(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	// Dedup: skip if a sub_agent block with the same sub_agent_id already exists
	const subAgentId = chunk.sub_agent_id;
	if (subAgentId) {
		const alreadyExists = state.blocks.some(
			(b) => b.block_type === 'sub_agent' && (b.data as SubAgentBlockData)._sub_agent_id === subAgentId
		);
		if (alreadyExists) return { ...state, spinnerContext: null };
	}

	const data: SubAgentBlockData = {
		agent_name: chunk.sub_agent_name ?? 'Unknown Agent',
		status: 'error',
		duration_ms: chunk.duration,
		report_summary: chunk.content,
		_sub_agent_id: subAgentId
	};
	const block = createBlock('sub_agent', state.nextSequence, data);
	return {
		...state,
		blocks: [...state.blocks, block],
		nextSequence: state.nextSequence + 1,
		spinnerContext: null
	};
}

/**
 * Process a reasoning chunk into a ThinkingBlock with agent_flow source.
 */
function handleReasoning(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	const data: ThinkingBlockData = {
		content: chunk.content ?? '',
		source: 'agent_flow'
	};
	const block = createBlock('thinking', state.nextSequence, data);
	return {
		...state,
		blocks: [...state.blocks, block],
		nextSequence: state.nextSequence + 1,
		spinnerContext: null
	};
}

/**
 * Process an error chunk - terminate execution.
 *
 * The error content is consumed by other surfaces (toasts, bgWorkflows);
 * this store only needs to flip out of the executing state.
 */
function handleError(state: ExecutionBlocksState, _chunk: StreamChunk): ExecutionBlocksState {
	return {
		...state,
		isExecuting: false,
		spinnerContext: null
	};
}

/**
 * Process a task_create chunk - add new task to the tasks array.
 * Tasks are tracked separately from blocks and displayed after the spinner.
 *
 * Drops the chunk if `task_id` is missing or empty: tasks without a stable id
 * cannot be matched by subsequent task_update / task_complete chunks, so
 * tracking them would create phantom rows that never resolve.
 */
function handleTaskCreate(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	if (!chunk.task_id) {
		return state;
	}
	const task: TodoTaskDisplay = {
		id: chunk.task_id,
		name: chunk.task_name ?? '',
		status: (chunk.task_status as TodoTaskDisplay['status']) ?? 'pending',
		priority: chunk.task_priority ?? 3,
		agent_name: chunk.task_agent_name
	};
	return {
		...state,
		tasks: [...state.tasks, task]
	};
}

/**
 * Process a task_update chunk - update task status in the tasks array.
 */
function handleTaskUpdate(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	return {
		...state,
		tasks: state.tasks.map((t) =>
			t.id === chunk.task_id
				? { ...t, status: (chunk.task_status as TodoTaskDisplay['status']) ?? t.status }
				: t
		)
	};
}

/**
 * Process a task_complete chunk - mark task as completed with optional duration.
 */
function handleTaskComplete(state: ExecutionBlocksState, chunk: StreamChunk): ExecutionBlocksState {
	return {
		...state,
		tasks: state.tasks.map((t) =>
			t.id === chunk.task_id
				? { ...t, status: 'completed' as const, duration_ms: chunk.duration }
				: t
		)
	};
}

/**
 * Chunk type to handler mapping for the execution blocks store.
 */
const chunkHandlers: Partial<Record<string, (state: ExecutionBlocksState, chunk: StreamChunk) => ExecutionBlocksState>> = {
	thinking_block: handleThinkingBlock,
	reasoning: handleReasoning,
	tool_start: handleToolStart,
	tool_call_complete: handleToolCallComplete,
	response_block: handleResponseBlock,
	sub_agent_complete: handleSubAgentComplete,
	sub_agent_error: handleSubAgentError,
	task_create: handleTaskCreate,
	task_update: handleTaskUpdate,
	task_complete: handleTaskComplete,
	error: handleError
};

/**
 * Execution blocks store for block-by-block display.
 *
 * Manages real-time blocks during execution and supports restoration
 * from persisted blocks when loading a conversation.
 */
export const executionBlocksStore = {
	subscribe: store.subscribe,

	/**
	 * Start execution for a workflow.
	 * Resets state and marks as executing.
	 *
	 * @param workflowId - The workflow ID being executed
	 */
	start(workflowId: string): void {
		store.set({
			...initialState,
			workflowId,
			isExecuting: true
		});
	},

	/**
	 * Process an incoming stream chunk.
	 * Routes to the appropriate handler based on chunk_type.
	 * Unrecognized chunk types are silently ignored.
	 *
	 * @param chunk - The stream chunk to process
	 */
	processChunk(chunk: StreamChunk): void {
		const handler = chunkHandlers[chunk.chunk_type];
		if (!handler) return;
		store.update((s) => handler(s, chunk));
	},

	/**
	 * Mark execution as complete.
	 */
	complete(): void {
		store.update((s) => ({
			...s,
			isExecuting: false,
			spinnerContext: null
		}));
	},

	/**
	 * Mark execution as cancelled.
	 */
	cancel(): void {
		store.update((s) => ({
			...s,
			isExecuting: false,
			spinnerContext: null
		}));
	},

	/**
	 * Reconstruct execution blocks by replaying buffered raw chunks.
	 *
	 * Used when switching BACK to a workflow that is still running in the
	 * background (H3 audit 2026-05-02). Without this, `start()` resets state
	 * on every selection so the execution area appears empty until the next
	 * chunk arrives. By replaying the buffered chunk history through the same
	 * `processChunk` path, we recreate the exact block sequence the user
	 * would have seen if they had stayed on this workflow.
	 *
	 * @param workflowId - ID of the workflow being restored
	 * @param chunks - Buffered raw stream chunks for this workflow
	 */
	restoreFromChunks(workflowId: string, chunks: StreamChunk[]): void {
		store.set({
			...initialState,
			workflowId,
			isExecuting: true
		});
		for (const chunk of chunks) {
			store.update((s) => {
				const handler = chunkHandlers[chunk.chunk_type];
				return handler ? handler(s, chunk) : s;
			});
		}
	},

	/**
	 * Reset to initial state.
	 */
	reset(): void {
		store.set(initialState);
	}
};

/** Current execution blocks */
export const executionBlocks = derived(store, (s) => s.blocks);

/** Whether execution is active */
export const isExecuting = derived(store, (s) => s.isExecuting);

/** Current spinner context text */
export const spinnerContext = derived(store, (s) => s.spinnerContext);

/** Current workflow ID being executed */
export const executionWorkflowId = derived(store, (s) => s.workflowId);

/** Active tasks from TodoTool (displayed after spinner in ChatContainer) */
export const executionTasks = derived(store, (s) => s.tasks);
