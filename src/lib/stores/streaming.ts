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
 * Streaming store for managing real-time workflow execution state.
 * Handles token streaming, tool execution tracking, and reasoning steps.
 *
 * @module stores/streaming
 */

import { writable, derived } from 'svelte/store';
import type { StreamChunk, WorkflowComplete } from '$types/streaming';
import { applyChunkToState } from './utils/chunkProcessor';

// ============================================================================
// Types
// ============================================================================

/**
 * Tool execution status during streaming
 */
export type ToolStatus = 'pending' | 'running' | 'completed' | 'error';

/**
 * Active tool being executed
 */
export interface ActiveTool {
	/** Tool name or identifier */
	name: string;
	/** Current execution status */
	status: ToolStatus;
	/** Timestamp when execution started */
	startedAt: number;
	/** Execution duration in milliseconds (when completed) */
	duration?: number;
	/** Error message if failed */
	error?: string;
}

/**
 * Reasoning step captured during streaming
 */
export interface ActiveReasoningStep {
	/** Step content */
	content: string;
	/** Timestamp when captured */
	timestamp: number;
	/** Step number (1-indexed) */
	stepNumber: number;
}

/**
 * Sub-agent status during streaming
 */
export type SubAgentStatus = 'starting' | 'running' | 'completed' | 'error';

/**
 * Active sub-agent being executed
 */
export interface ActiveSubAgent {
	/** Sub-agent ID */
	id: string;
	/** Sub-agent name */
	name: string;
	/** Parent agent ID */
	parentAgentId: string;
	/** Task description */
	taskDescription: string;
	/** Current execution status */
	status: SubAgentStatus;
	/** Timestamp when execution started */
	startedAt: number;
	/** Progress percentage (0-100) */
	progress: number;
	/** Status message (optional) */
	statusMessage?: string;
	/** Execution duration in milliseconds (when completed) */
	duration?: number;
	/** Report content (when completed) */
	report?: string;
	/** Error message (if failed) */
	error?: string;
	/** Execution metrics (when completed) */
	metrics?: {
		duration_ms: number;
		tokens_input: number;
		tokens_output: number;
	};
}

/**
 * Active task being tracked
 */
export interface ActiveTask {
	/** Task ID */
	id: string;
	/** Task name/description */
	name: string;
	/** Current execution status */
	status: 'pending' | 'in_progress' | 'completed' | 'blocked';
	/** Task priority (1-5) */
	priority: number;
	/** Timestamp when task was created */
	createdAt: number;
	/** Timestamp when task was last updated */
	updatedAt: number;
}

/**
 * Streaming state interface
 */
export interface StreamingState {
	/** Currently streaming workflow ID (null if not streaming) */
	workflowId: string | null;
	/** Accumulated content from token chunks */
	content: string;
	/** List of tools being executed */
	tools: ActiveTool[];
	/** Reasoning steps captured */
	reasoning: ActiveReasoningStep[];
	/** Active sub-agents being executed */
	subAgents: ActiveSubAgent[];
	/** Active tasks being tracked */
	tasks: ActiveTask[];
	/** Whether streaming is currently active */
	isStreaming: boolean;
	/** Whether streaming completed but activities not yet captured */
	completed: boolean;
	/** Total output tokens received */
	tokensReceived: number;
	/** Total input tokens reported by response_block chunks (Phase 13) */
	tokensSent: number;
	/** Cached input tokens reported by the latest response_block chunk */
	cachedTokens: number | null;
	/** Cache-write tokens reported by the latest response_block chunk */
	cacheWriteTokens: number | null;
	/**
	 * Running sum of `cost_usd` from response_block chunks (backend-computed).
	 * `null` until the first chunk with a cost arrives. Option A follow-up.
	 */
	partialCostUsd: number | null;
	/** Error message if streaming failed */
	error: string | null;
	/** Whether workflow was cancelled */
	cancelled: boolean;
}

// ============================================================================
// Initial State
// ============================================================================

/**
 * Initial streaming state
 */
const initialState: StreamingState = {
	workflowId: null,
	content: '',
	tools: [],
	reasoning: [],
	subAgents: [],
	tasks: [],
	isStreaming: false,
	completed: false,
	tokensReceived: 0,
	tokensSent: 0,
	cachedTokens: null,
	cacheWriteTokens: null,
	partialCostUsd: null,
	error: null,
	cancelled: false
};

// ============================================================================
// Store Implementation
// ============================================================================

/**
 * Internal writable store
 */
const store = writable<StreamingState>(initialState);


// ============================================================================
// Chunk Processing
// ============================================================================

/**
 * Process a stream chunk with streaming-specific side-effects.
 * Delegates common state updates to applyChunkToState, then applies:
 * - error: sets isStreaming to false
 *
 * @param state - Current streaming state
 * @param chunk - Incoming stream chunk
 * @returns Updated streaming state
 */
function processChunk(state: StreamingState, chunk: StreamChunk): StreamingState {
	// Apply common state update
	const updated = applyChunkToState(state, chunk);

	// Streaming-specific side-effects
	if (chunk.chunk_type === 'error') {
		return { ...updated, isStreaming: false };
	}

	return updated;
}

/**
 * Streaming store with actions for managing real-time workflow execution.
 * Event listeners are now owned by backgroundWorkflowsStore which forwards
 * chunks/completions for the currently viewed workflow via processChunkDirect/processCompleteDirect.
 */
export const streamingStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Starts streaming for a workflow.
	 * Resets state and marks as streaming. Listeners are managed by backgroundWorkflowsStore.
	 *
	 * @param workflowId - The workflow ID to stream
	 */
	async start(workflowId: string): Promise<void> {
		// Reset state with new workflow
		store.set({
			...initialState,
			workflowId,
			isStreaming: true
		});
	},

	/**
	 * Process a stream chunk directly (called by backgroundWorkflowsStore for viewed workflow).
	 * Skips workflow_id filtering since backgroundWorkflowsStore already handles routing.
	 *
	 * @param chunk - The stream chunk to process
	 */
	processChunkDirect(chunk: StreamChunk): void {
		store.update((s) => processChunk(s, chunk));
	},

	/**
	 * Process a workflow completion directly (called by backgroundWorkflowsStore for viewed workflow).
	 * Unlike the event-based processComplete, this skips workflow_id filtering.
	 *
	 * @param complete - The workflow completion event
	 */
	processCompleteDirect(complete: WorkflowComplete): void {
		store.update((s) => {
			const updates: Partial<StreamingState> = {
				completed: true
			};

			if (complete.status === 'error') {
				updates.error = complete.error ?? 'Unknown error';
				updates.isStreaming = false;
			} else if (complete.status === 'cancelled') {
				updates.cancelled = true;
				updates.isStreaming = false;
			}

			return { ...s, ...updates };
		});
	},

	/**
	 * Restore streaming state from a background workflow execution.
	 * Used when switching to view a running background workflow.
	 *
	 * Phase 13: hydrates `tokensSent`, `cachedTokens` and `cacheWriteTokens`
	 * from the bg state so the session display reflects what the running
	 * workflow has actually consumed (not just the output count).
	 *
	 * @param bgState - The background workflow state to restore from
	 */
	restoreFrom(bgState: {
		workflowId: string;
		content: string;
		tools: ActiveTool[];
		reasoning: ActiveReasoningStep[];
		subAgents: ActiveSubAgent[];
		tasks: ActiveTask[];
		tokensReceived: number;
		tokensSent?: number;
		cachedTokens?: number | null;
		cacheWriteTokens?: number | null;
		partialCostUsd?: number | null;
		error: string | null;
		status: string;
	}): void {
		const isRunning = bgState.status === 'running';
		store.set({
			workflowId: bgState.workflowId,
			content: bgState.content,
			tools: bgState.tools,
			reasoning: bgState.reasoning,
			subAgents: bgState.subAgents,
			tasks: bgState.tasks,
			isStreaming: isRunning,
			completed: !isRunning,
			tokensReceived: bgState.tokensReceived,
			tokensSent: bgState.tokensSent ?? 0,
			cachedTokens: bgState.cachedTokens ?? null,
			cacheWriteTokens: bgState.cacheWriteTokens ?? null,
			partialCostUsd: bgState.partialCostUsd ?? null,
			error: bgState.error,
			cancelled: bgState.status === 'cancelled'
		});
	},

	/**
	 * Cleanup any resources.
	 * Event listeners are managed by backgroundWorkflowsStore, not this store.
	 */
	cleanup(): void {
		// No-op: listeners are now managed by backgroundWorkflowsStore
	},

	/**
	 * Resets the store to initial state.
	 */
	reset(): void {
		store.set(initialState);
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: all active sub-agents.
 * Used by workflowExecutor.service.ts for sub-agent metrics capture.
 */
export const activeSubAgents = derived(store, (s) => s.subAgents);
