// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Streaming store for managing real-time workflow execution state.
 * Handles token streaming, tool execution tracking, and reasoning steps.
 *
 * @module stores/streaming
 */

import { writable, derived, get } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { StreamChunk, WorkflowComplete } from '$types/streaming';

/**
 * Event names for Tauri streaming events (inlined to avoid runtime resolution issues)
 */
const STREAM_EVENTS = {
	WORKFLOW_STREAM: 'workflow_stream',
	WORKFLOW_COMPLETE: 'workflow_complete'
} as const;

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

/**
 * Event listener cleanup functions
 */
let unlisteners: UnlistenFn[] = [];

// ============================================================================
// Task Handler Functions
// ============================================================================

/**
 * Adds a new task to the active tasks list.
 *
 * @param chunk - Stream chunk containing task creation data
 */
function addTask(chunk: StreamChunk): void {
	store.update((s) => ({
		...s,
		tasks: [
			...s.tasks,
			{
				id: chunk.task_id!,
				name: chunk.task_name!,
				status: (chunk.task_status ?? 'pending') as ActiveTask['status'],
				priority: chunk.task_priority ?? 3,
				createdAt: Date.now(),
				updatedAt: Date.now()
			}
		]
	}));
}

/**
 * Updates the status of an existing task.
 *
 * @param chunk - Stream chunk containing task update data
 */
function updateTaskStatus(chunk: StreamChunk): void {
	store.update((s) => ({
		...s,
		tasks: s.tasks.map((t) =>
			t.id === chunk.task_id
				? { ...t, status: chunk.task_status as ActiveTask['status'], updatedAt: Date.now() }
				: t
		)
	}));
}

/**
 * Marks a task as completed.
 *
 * @param chunk - Stream chunk containing task completion data
 */
function completeTask(chunk: StreamChunk): void {
	store.update((s) => ({
		...s,
		tasks: s.tasks.map((t) =>
			t.id === chunk.task_id
				? { ...t, status: 'completed' as const, updatedAt: Date.now() }
				: t
		)
	}));
}

// ============================================================================
// Chunk Processing
// ============================================================================

/**
 * Process a stream chunk and update state accordingly.
 *
 * @param chunk - The stream chunk to process
 */
function processChunk(chunk: StreamChunk): void {
	store.update((s) => {
		// Ignore chunks for different workflows
		if (s.workflowId && chunk.workflow_id !== s.workflowId) {
			return s;
		}

		switch (chunk.chunk_type) {
			case 'token':
				return {
					...s,
					content: s.content + (chunk.content ?? ''),
					tokensReceived: s.tokensReceived + 1
				};

			case 'tool_start':
				return {
					...s,
					tools: [
						...s.tools,
						{
							name: chunk.tool ?? 'unknown',
							status: 'running' as ToolStatus,
							startedAt: Date.now()
						}
					]
				};

			case 'tool_end':
				return {
					...s,
					tools: s.tools.map((t) =>
						t.name === chunk.tool && t.status === 'running'
							? { ...t, status: 'completed' as ToolStatus, duration: chunk.duration }
							: t
					)
				};

			case 'reasoning':
				return {
					...s,
					reasoning: [
						...s.reasoning,
						{
							content: chunk.content ?? '',
							timestamp: Date.now(),
							stepNumber: s.reasoning.length + 1
						}
					]
				};

			case 'error':
				return {
					...s,
					error: chunk.content ?? 'Unknown error',
					isStreaming: false
				};

			case 'sub_agent_start':
				return {
					...s,
					subAgents: [
						...s.subAgents,
						{
							id: chunk.sub_agent_id ?? 'unknown',
							name: chunk.sub_agent_name ?? 'Unknown Agent',
							parentAgentId: chunk.parent_agent_id ?? '',
							taskDescription: chunk.content ?? '',
							status: 'running' as SubAgentStatus,
							startedAt: Date.now(),
							progress: 0
						}
					]
				};

			case 'sub_agent_progress':
				return {
					...s,
					subAgents: s.subAgents.map((a) =>
						a.id === chunk.sub_agent_id
							? {
									...a,
									progress: chunk.progress ?? a.progress,
									statusMessage: chunk.content ?? a.statusMessage
								}
							: a
					)
				};

			case 'sub_agent_complete':
				return {
					...s,
					subAgents: s.subAgents.map((a) =>
						a.id === chunk.sub_agent_id
							? {
									...a,
									status: 'completed' as SubAgentStatus,
									progress: 100,
									duration: chunk.duration,
									report: chunk.content,
									metrics: chunk.metrics
								}
							: a
					)
				};

			case 'sub_agent_error':
				return {
					...s,
					subAgents: s.subAgents.map((a) =>
						a.id === chunk.sub_agent_id
							? {
									...a,
									status: 'error' as SubAgentStatus,
									error: chunk.content ?? 'Unknown error',
									duration: chunk.duration
								}
							: a
					)
				};

			case 'task_create':
				addTask(chunk);
				return s;

			case 'task_update':
				updateTaskStatus(chunk);
				return s;

			case 'task_complete':
				completeTask(chunk);
				return s;

			default:
				return s;
		}
	});
}

/**
 * Process workflow completion event.
 *
 * @param complete - The workflow completion event
 */
function processComplete(complete: WorkflowComplete): void {
	store.update((s) => {
		// Ignore completions for different workflows
		if (s.workflowId && complete.workflow_id !== s.workflowId) {
			return s;
		}

		// Mark as completed but keep isStreaming true until activities are captured
		// This prevents the UI from switching to empty historicalActivities prematurely
		const updates: Partial<StreamingState> = {
			completed: true
		};

		if (complete.status === 'error') {
			updates.error = complete.error ?? 'Unknown error';
			updates.isStreaming = false; // Stop streaming on error
		} else if (complete.status === 'cancelled') {
			updates.cancelled = true;
			updates.isStreaming = false; // Stop streaming on cancel
		}

		return { ...s, ...updates };
	});
}

/**
 * Streaming store with actions for managing real-time workflow execution.
 */
export const streamingStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Starts streaming for a workflow.
	 * Resets state and sets up event listeners.
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

		// Cleanup any existing listeners
		await this.cleanup();

		// Setup new listeners
		const unlistenChunk = await listen<StreamChunk>(STREAM_EVENTS.WORKFLOW_STREAM, (event) => {
			processChunk(event.payload);
		});

		const unlistenComplete = await listen<WorkflowComplete>(
			STREAM_EVENTS.WORKFLOW_COMPLETE,
			(event) => {
				processComplete(event.payload);
			}
		);

		unlisteners = [unlistenChunk, unlistenComplete];
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
	 * Cleanup event listeners.
	 * Should be called when unmounting or stopping streaming.
	 */
	async cleanup(): Promise<void> {
		for (const unlisten of unlisteners) {
			unlisten();
		}
		unlisteners = [];
	},

	/**
	 * Resets the store to initial state.
	 * Also cleans up event listeners.
	 */
	async reset(): Promise<void> {
		await this.cleanup();
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

/**
 * Derived store: whether there are any running tools
 */
export const hasRunningTools = derived(store, (s) => s.tools.some((t) => t.status === 'running'));

// ============================================================================
// Sub-Agent Derived Stores
// ============================================================================

/**
 * Derived store: all active sub-agents
 */
export const activeSubAgents = derived(store, (s) => s.subAgents);

/**
 * Derived store: sub-agents currently running
 */
export const runningSubAgents = derived(store, (s) =>
	s.subAgents.filter((a) => a.status === 'running')
);

/**
 * Derived store: sub-agents that have completed
 */
export const completedSubAgents = derived(store, (s) =>
	s.subAgents.filter((a) => a.status === 'completed')
);

/**
 * Derived store: sub-agents that have errored
 */
export const erroredSubAgents = derived(store, (s) =>
	s.subAgents.filter((a) => a.status === 'error')
);

/**
 * Derived store: whether there are any running sub-agents
 */
export const hasRunningSubAgents = derived(store, (s) =>
	s.subAgents.some((a) => a.status === 'running')
);

/**
 * Derived store: total count of sub-agents
 */
export const subAgentCount = derived(store, (s) => s.subAgents.length);

/**
 * Derived store: whether there are any active sub-agents
 */
export const hasActiveSubAgents = derived(store, (s) => s.subAgents.length > 0);

// ============================================================================
// Task Derived Stores
// ============================================================================

/**
 * Derived store: all active tasks
 */
export const activeTasks = derived(store, (s) => s.tasks);

/**
 * Derived store: tasks with pending status
 */
export const pendingTasks = derived(store, (s) =>
	s.tasks.filter((t) => t.status === 'pending')
);

/**
 * Derived store: tasks currently in progress
 */
export const runningTasks = derived(store, (s) =>
	s.tasks.filter((t) => t.status === 'in_progress')
);

/**
 * Derived store: tasks that have completed
 */
export const completedTasks = derived(store, (s) =>
	s.tasks.filter((t) => t.status === 'completed')
);

/**
 * Derived store: whether there are any active tasks
 */
export const hasActiveTasks = derived(store, (s) => s.tasks.length > 0);
