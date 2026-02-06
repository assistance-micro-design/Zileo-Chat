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
 * Streaming store for managing real-time workflow execution state.
 * Handles token streaming, tool execution tracking, and reasoning steps.
 *
 * @module stores/streaming
 */

import { writable, derived, get } from 'svelte/store';
import type { StreamChunk, WorkflowComplete } from '$types/streaming';
import { tokenStore } from './tokens';

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
	/** Total tokens received */
	tokensReceived: number;
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
 * Type for chunk handler functions that process stream chunks
 */
type ChunkHandler = (state: StreamingState, chunk: StreamChunk) => StreamingState;

/**
 * Handle token chunk - append content, increment counter, and sync with tokenStore.
 * Now supports real-time token updates via tokens_delta/tokens_total fields.
 */
function handleToken(s: StreamingState, c: StreamChunk): StreamingState {
	const newTokensReceived = s.tokensReceived + 1;

	// Sync with tokenStore for real-time display
	// Use tokens_total from backend if available, otherwise use received count
	const outputTokens = c.tokens_total ?? newTokensReceived;
	tokenStore.updateStreamingTokens(outputTokens);

	return {
		...s,
		content: s.content + (c.content ?? ''),
		tokensReceived: newTokensReceived
	};
}

/**
 * Handle tool_start chunk - add new tool with running status
 */
function handleToolStart(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		tools: [
			...s.tools,
			{
				name: c.tool ?? 'unknown',
				status: 'running' as ToolStatus,
				startedAt: Date.now()
			}
		]
	};
}

/**
 * Handle tool_end chunk - mark tool as completed with duration
 */
function handleToolEnd(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		tools: s.tools.map((t) =>
			t.name === c.tool && t.status === 'running'
				? { ...t, status: 'completed' as ToolStatus, duration: c.duration }
				: t
		)
	};
}

/**
 * Handle reasoning chunk - add new reasoning step
 */
function handleReasoning(s: StreamingState, c: StreamChunk): StreamingState {
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
 * Handle error chunk - set error message and stop streaming
 */
function handleError(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		error: c.content ?? 'Unknown error',
		isStreaming: false
	};
}

/**
 * Handle sub_agent_start chunk - add new sub-agent with running status
 */
function handleSubAgentStart(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		subAgents: [
			...s.subAgents,
			{
				id: c.sub_agent_id ?? 'unknown',
				name: c.sub_agent_name ?? 'Unknown Agent',
				parentAgentId: c.parent_agent_id ?? '',
				taskDescription: c.content ?? '',
				status: 'running' as SubAgentStatus,
				startedAt: Date.now(),
				progress: 0
			}
		]
	};
}

/**
 * Handle sub_agent_progress chunk - update sub-agent progress and status message
 */
function handleSubAgentProgress(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		subAgents: s.subAgents.map((a) =>
			a.id === c.sub_agent_id
				? {
						...a,
						progress: c.progress ?? a.progress,
						statusMessage: c.content ?? a.statusMessage
					}
				: a
		)
	};
}

/**
 * Handle sub_agent_complete chunk - mark sub-agent as completed with metrics
 */
function handleSubAgentComplete(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		subAgents: s.subAgents.map((a) =>
			a.id === c.sub_agent_id
				? {
						...a,
						status: 'completed' as SubAgentStatus,
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
 * Handle sub_agent_error chunk - mark sub-agent as errored with error message
 */
function handleSubAgentError(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		subAgents: s.subAgents.map((a) =>
			a.id === c.sub_agent_id
				? {
						...a,
						status: 'error' as SubAgentStatus,
						error: c.content ?? 'Unknown error',
						duration: c.duration
					}
				: a
		)
	};
}

/**
 * Handle task_create chunk - add new task
 */
function handleTaskCreate(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		tasks: [
			...s.tasks,
			{
				id: c.task_id!,
				name: c.task_name!,
				status: (c.task_status ?? 'pending') as ActiveTask['status'],
				priority: c.task_priority ?? 3,
				createdAt: Date.now(),
				updatedAt: Date.now()
			}
		]
	};
}

/**
 * Handle task_update chunk - update task status
 */
function handleTaskUpdate(s: StreamingState, c: StreamChunk): StreamingState {
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
 * Handle task_complete chunk - mark task as completed
 */
function handleTaskComplete(s: StreamingState, c: StreamChunk): StreamingState {
	return {
		...s,
		tasks: s.tasks.map((t) =>
			t.id === c.task_id ? { ...t, status: 'completed' as const, updatedAt: Date.now() } : t
		)
	};
}

/**
 * Chunk handler registry mapping chunk types to their handler functions
 */
const chunkHandlers: Record<string, ChunkHandler> = {
	token: handleToken,
	tool_start: handleToolStart,
	tool_end: handleToolEnd,
	reasoning: handleReasoning,
	error: handleError,
	sub_agent_start: handleSubAgentStart,
	sub_agent_progress: handleSubAgentProgress,
	sub_agent_complete: handleSubAgentComplete,
	sub_agent_error: handleSubAgentError,
	task_create: handleTaskCreate,
	task_update: handleTaskUpdate,
	task_complete: handleTaskComplete
};

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
	 * Unlike the event-based processChunk, this skips workflow_id filtering.
	 *
	 * @param chunk - The stream chunk to process
	 */
	processChunkDirect(chunk: StreamChunk): void {
		store.update((s) => {
			const handler = chunkHandlers[chunk.chunk_type];
			return handler ? handler(s, chunk) : s;
		});
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
	 * @param state - The background workflow state to restore from
	 */
	restoreFrom(bgState: {
		workflowId: string;
		content: string;
		tools: ActiveTool[];
		reasoning: ActiveReasoningStep[];
		subAgents: ActiveSubAgent[];
		tasks: ActiveTask[];
		tokensReceived: number;
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
			error: bgState.error,
			cancelled: bgState.status === 'cancelled'
		});
	},

	/**
	 * Appends a token to the streaming content.
	 *
	 * @param content - Token content to append
	 */
	appendToken(content: string): void {
		store.update((s) => ({
			...s,
			content: s.content + content,
			tokensReceived: s.tokensReceived + 1
		}));
	},

	/**
	 * Marks a tool as started.
	 *
	 * @param toolName - Name of the tool
	 */
	addToolStart(toolName: string): void {
		store.update((s) => ({
			...s,
			tools: [
				...s.tools,
				{
					name: toolName,
					status: 'running' as ToolStatus,
					startedAt: Date.now()
				}
			]
		}));
	},

	/**
	 * Marks a tool as completed.
	 *
	 * @param toolName - Name of the tool
	 * @param duration - Execution duration in milliseconds
	 */
	completeToolEnd(toolName: string, duration: number): void {
		store.update((s) => ({
			...s,
			tools: s.tools.map((t) =>
				t.name === toolName && t.status === 'running'
					? { ...t, status: 'completed' as ToolStatus, duration }
					: t
			)
		}));
	},

	/**
	 * Marks a tool as failed.
	 *
	 * @param toolName - Name of the tool
	 * @param error - Error message
	 */
	failTool(toolName: string, error: string): void {
		store.update((s) => ({
			...s,
			tools: s.tools.map((t) =>
				t.name === toolName && t.status === 'running'
					? { ...t, status: 'error' as ToolStatus, error }
					: t
			)
		}));
	},

	/**
	 * Adds a reasoning step.
	 *
	 * @param content - Reasoning content
	 */
	addReasoning(content: string): void {
		store.update((s) => ({
			...s,
			reasoning: [
				...s.reasoning,
				{
					content,
					timestamp: Date.now(),
					stepNumber: s.reasoning.length + 1
				}
			]
		}));
	},

	/**
	 * Sets an error state.
	 *
	 * @param error - Error message
	 */
	setError(error: string): void {
		store.update((s) => ({
			...s,
			error,
			isStreaming: false
		}));
	},

	/**
	 * Marks streaming as complete.
	 * Sets completed flag but keeps isStreaming true until reset.
	 */
	complete(): void {
		store.update((s) => ({ ...s, completed: true }));
	},

	/**
	 * Marks streaming as cancelled.
	 */
	cancel(): void {
		store.update((s) => ({
			...s,
			isStreaming: false,
			cancelled: true
		}));
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
	},

	/**
	 * Gets the current streaming content.
	 * Useful for extracting final content after streaming.
	 *
	 * @returns Current accumulated content
	 */
	getContent(): string {
		return get(store).content;
	},

	/**
	 * Gets the current state snapshot.
	 *
	 * @returns Current streaming state
	 */
	getState(): StreamingState {
		return get(store);
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: whether streaming is active
 */
export const isStreaming = derived(store, (s) => s.isStreaming);

/**
 * Derived store: current streaming content
 */
export const streamContent = derived(store, (s) => s.content);

/**
 * Derived store: active tools being executed
 */
export const activeTools = derived(store, (s) => s.tools);

/**
 * Derived store: tools currently running
 * Use direct check for boolean: runningTools.length > 0 (replaces hasRunningTools)
 */
export const runningTools = derived(store, (s) => s.tools.filter((t) => t.status === 'running'));

/**
 * Derived store: tools that have completed
 */
export const completedTools = derived(store, (s) =>
	s.tools.filter((t) => t.status === 'completed')
);

/**
 * Derived store: reasoning steps captured
 */
export const reasoningSteps = derived(store, (s) => s.reasoning);

/**
 * Derived store: current error message
 */
export const streamError = derived(store, (s) => s.error);

/**
 * Derived store: whether workflow was cancelled
 */
export const isCancelled = derived(store, (s) => s.cancelled);

/**
 * Derived store: whether streaming has completed (but activities may not yet be captured)
 */
export const isCompleted = derived(store, (s) => s.completed);

/**
 * Derived store: whether activities should be shown from streaming store
 * True when streaming is active OR when completed but not yet reset
 */
export const hasStreamingActivities = derived(
	store,
	(s) => s.isStreaming || (s.completed && (s.tools.length > 0 || s.reasoning.length > 0 || s.subAgents.length > 0 || s.tasks.length > 0))
);

/**
 * Derived store: total tokens received
 */
export const tokensReceived = derived(store, (s) => s.tokensReceived);

/**
 * Derived store: current workflow ID
 */
export const currentWorkflowId = derived(store, (s) => s.workflowId);

// ============================================================================
// Sub-Agent Derived Stores
// ============================================================================

/**
 * Derived store: all active sub-agents
 * Use direct checks for filtering/counting:
 * - Running: activeSubAgents.filter(a => a.status === 'running')
 * - Completed: activeSubAgents.filter(a => a.status === 'completed')
 * - Errored: activeSubAgents.filter(a => a.status === 'error')
 * - Has any: activeSubAgents.length > 0
 * - Count: activeSubAgents.length
 */
export const activeSubAgents = derived(store, (s) => s.subAgents);

// ============================================================================
// Task Derived Stores
// ============================================================================

/**
 * Derived store: all active tasks
 * Use direct checks for filtering/counting:
 * - Pending: activeTasks.filter(t => t.status === 'pending')
 * - In progress: activeTasks.filter(t => t.status === 'in_progress')
 * - Completed: activeTasks.filter(t => t.status === 'completed')
 * - Has any: activeTasks.length > 0
 * - Count: activeTasks.length
 */
export const activeTasks = derived(store, (s) => s.tasks);
