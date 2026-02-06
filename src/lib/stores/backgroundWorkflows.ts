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
 * @fileoverview Background workflows store for managing concurrent workflow executions.
 *
 * This store is the CENTRAL event dispatch point for all workflow streaming events.
 * It owns the global Tauri event listeners, routes chunks to the correct workflow
 * in its internal map, forwards chunks for the currently-viewed workflow to the
 * streaming store, and fires toast notifications on completion and user questions.
 *
 * @module stores/backgroundWorkflows
 */

import { writable, derived, get } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { StreamChunk, WorkflowComplete } from '$types/streaming';
import type { WorkflowStreamState, BackgroundWorkflowStatus } from '$types/background-workflow';
import type { UserQuestionStreamPayload } from '$types/user-question';
import type { ActiveTool, ActiveSubAgent, ActiveTask } from '$lib/stores/streaming';
import { toastStore } from './toast';
import { settings as validationSettings } from './validation-settings';

// ============================================================================
// Constants
// ============================================================================

/** Interval for cleaning up old completed executions (10 minutes) */
const CLEANUP_INTERVAL_MS = 10 * 60 * 1000;

/** Maximum concurrent workflows when validation mode is 'auto' */
const MAX_CONCURRENT_AUTO = 3;

/** Maximum concurrent workflows for other validation modes */
const MAX_CONCURRENT_OTHER = 1;

/** Tauri event names for workflow streaming */
const STREAM_EVENTS = {
	WORKFLOW_STREAM: 'workflow_stream',
	WORKFLOW_COMPLETE: 'workflow_complete'
} as const;

// ============================================================================
// Types
// ============================================================================

/**
 * Internal state shape for the background workflows store.
 */
interface BackgroundWorkflowsState {
	/** Map of workflow ID to its stream state */
	executions: Map<string, WorkflowStreamState>;
	/** The workflow ID currently being viewed in the UI (null if none) */
	viewedWorkflowId: string | null;
}

// ============================================================================
// Store internals
// ============================================================================

const initialState: BackgroundWorkflowsState = {
	executions: new Map(),
	viewedWorkflowId: null
};

const store = writable<BackgroundWorkflowsState>(initialState);

let unlisteners: UnlistenFn[] = [];
let isInitialized = false;
let cleanupTimer: ReturnType<typeof setInterval> | null = null;

/**
 * Callback for forwarding chunks to the streaming store when the
 * chunk belongs to the currently-viewed workflow. Set via setForwardCallbacks.
 */
let onChunkForViewed: ((chunk: StreamChunk) => void) | null = null;

/**
 * Callback for forwarding completion events to the streaming store when the
 * completion belongs to the currently-viewed workflow. Set via setForwardCallbacks.
 */
let onCompleteForViewed: ((complete: WorkflowComplete) => void) | null = null;

/**
 * Callback for dispatching user question events to the userQuestionStore.
 * Called for ALL user_question_start chunks (both viewed and non-viewed workflows).
 * Set via setForwardCallbacks to avoid circular imports.
 */
let onUserQuestion: ((payload: UserQuestionStreamPayload, workflowId: string, isViewed: boolean) => void) | null = null;

// ============================================================================
// Internal helpers
// ============================================================================

/**
 * Creates an initial WorkflowStreamState for a newly registered workflow.
 *
 * @param workflowId - Unique workflow identifier
 * @param agentId - Agent executing the workflow
 * @param workflowName - Human-readable workflow name
 * @returns Fresh WorkflowStreamState with running status
 */
function createInitialExecution(
	workflowId: string,
	agentId: string,
	workflowName: string
): WorkflowStreamState {
	return {
		workflowId,
		agentId,
		workflowName,
		status: 'running',
		content: '',
		tools: [],
		reasoning: [],
		subAgents: [],
		tasks: [],
		tokensReceived: 0,
		error: null,
		startedAt: Date.now(),
		completedAt: null,
		hasPendingQuestion: false
	};
}

/**
 * Applies a stream chunk to an existing execution state, returning the updated state.
 * This is a pure function that does not mutate the input.
 *
 * @param exec - Current execution state
 * @param chunk - Incoming stream chunk
 * @returns Updated execution state
 */
function updateExecutionFromChunk(
	exec: WorkflowStreamState,
	chunk: StreamChunk
): WorkflowStreamState {
	const updated = { ...exec };

	switch (chunk.chunk_type) {
		case 'token':
			updated.content += chunk.content ?? '';
			updated.tokensReceived += 1;
			break;

		case 'tool_start':
			updated.tools = [
				...updated.tools,
				{
					name: chunk.tool ?? 'unknown',
					status: 'running' as const,
					startedAt: Date.now()
				}
			];
			break;

		case 'tool_end':
			updated.tools = updated.tools.map((t: ActiveTool) =>
				t.name === chunk.tool && t.status === 'running'
					? { ...t, status: 'completed' as const, duration: chunk.duration }
					: t
			);
			break;

		case 'reasoning':
			updated.reasoning = [
				...updated.reasoning,
				{
					content: chunk.content ?? '',
					timestamp: Date.now(),
					stepNumber: updated.reasoning.length + 1
				}
			];
			break;

		case 'error':
			updated.error = chunk.content ?? 'Unknown error';
			break;

		case 'sub_agent_start':
			updated.subAgents = [
				...updated.subAgents,
				{
					id: chunk.sub_agent_id ?? 'unknown',
					name: chunk.sub_agent_name ?? 'Unknown Agent',
					parentAgentId: chunk.parent_agent_id ?? '',
					taskDescription: chunk.content ?? '',
					status: 'running' as const,
					startedAt: Date.now(),
					progress: 0
				}
			];
			break;

		case 'sub_agent_progress':
			updated.subAgents = updated.subAgents.map((a: ActiveSubAgent) =>
				a.id === chunk.sub_agent_id
					? {
							...a,
							progress: chunk.progress ?? a.progress,
							statusMessage: chunk.content ?? a.statusMessage
						}
					: a
			);
			break;

		case 'sub_agent_complete':
			updated.subAgents = updated.subAgents.map((a: ActiveSubAgent) =>
				a.id === chunk.sub_agent_id
					? {
							...a,
							status: 'completed' as const,
							progress: 100,
							duration: chunk.duration,
							report: chunk.content,
							metrics: chunk.metrics
						}
					: a
			);
			break;

		case 'sub_agent_error':
			updated.subAgents = updated.subAgents.map((a: ActiveSubAgent) =>
				a.id === chunk.sub_agent_id
					? {
							...a,
							status: 'error' as const,
							error: chunk.content ?? 'Unknown error',
							duration: chunk.duration
						}
					: a
			);
			break;

		case 'task_create':
			updated.tasks = [
				...updated.tasks,
				{
					id: chunk.task_id!,
					name: chunk.task_name!,
					status: (chunk.task_status ?? 'pending') as ActiveTask['status'],
					priority: chunk.task_priority ?? 3,
					createdAt: Date.now(),
					updatedAt: Date.now()
				}
			];
			break;

		case 'task_update':
			updated.tasks = updated.tasks.map((t: ActiveTask) =>
				t.id === chunk.task_id
					? { ...t, status: chunk.task_status as ActiveTask['status'], updatedAt: Date.now() }
					: t
			);
			break;

		case 'task_complete':
			updated.tasks = updated.tasks.map((t: ActiveTask) =>
				t.id === chunk.task_id
					? { ...t, status: 'completed' as const, updatedAt: Date.now() }
					: t
			);
			break;

		case 'user_question_start':
			updated.hasPendingQuestion = true;
			break;

		case 'user_question_complete':
			updated.hasPendingQuestion = false;
			break;
	}

	return updated;
}

/**
 * Handles an incoming stream chunk from the Tauri event listener.
 * Updates the background state, forwards to the streaming store if the
 * chunk belongs to the viewed workflow, and fires toast notifications
 * for user questions on non-viewed workflows.
 *
 * @param chunk - Incoming stream chunk
 */
function handleStreamChunk(chunk: StreamChunk): void {
	const state = get(store);
	const exec = state.executions.get(chunk.workflow_id);
	if (!exec) return; // Not a tracked background workflow

	// Update background state
	const updated = updateExecutionFromChunk(exec, chunk);
	store.update((s) => {
		const newExecs = new Map(s.executions);
		newExecs.set(chunk.workflow_id, updated);
		return { ...s, executions: newExecs };
	});

	// Forward to streaming store if this is the viewed workflow
	if (chunk.workflow_id === state.viewedWorkflowId && onChunkForViewed) {
		onChunkForViewed(chunk);
	}

	// Handle user question for all workflows
	if (chunk.chunk_type === 'user_question_start' && chunk.user_question) {
		const isViewed = chunk.workflow_id === state.viewedWorkflowId;

		// Dispatch to userQuestionStore (queues question + opens modal if viewed)
		if (onUserQuestion) {
			onUserQuestion(chunk.user_question, chunk.workflow_id, isViewed);
		}

		// Toast notification for non-viewed workflows
		if (!isViewed) {
			toastStore.addUserQuestion(
				chunk.workflow_id,
				exec.workflowName,
				chunk.user_question.question
			);
		}
	}
}

/**
 * Handles a workflow completion event from the Tauri event listener.
 * Updates the execution status, forwards to the streaming store if viewed,
 * fires toast notifications, and dismisses any pending user-question toasts.
 *
 * @param complete - Workflow completion event
 */
function handleStreamComplete(complete: WorkflowComplete): void {
	const state = get(store);
	const exec = state.executions.get(complete.workflow_id);
	if (!exec) return;

	const statusMap: Record<string, BackgroundWorkflowStatus> = {
		completed: 'completed',
		error: 'error',
		cancelled: 'cancelled'
	};

	store.update((s) => {
		const newExecs = new Map(s.executions);
		const updated: WorkflowStreamState = {
			...exec,
			status: (statusMap[complete.status] ?? 'error') as BackgroundWorkflowStatus,
			completedAt: Date.now(),
			error: complete.error ?? null
		};
		newExecs.set(complete.workflow_id, updated);
		return { ...s, executions: newExecs };
	});

	// Forward to streaming store if viewed
	if (complete.workflow_id === state.viewedWorkflowId && onCompleteForViewed) {
		onCompleteForViewed(complete);
	}

	// Dismiss any user-question toasts for this workflow BEFORE adding completion toast
	toastStore.dismissForWorkflow(complete.workflow_id);

	// Toast notification for completion and error states
	if (complete.status === 'completed' || complete.status === 'error') {
		toastStore.addWorkflowComplete(complete.workflow_id, exec.workflowName, complete.status);
	}
}

/**
 * Removes completed executions that finished more than CLEANUP_INTERVAL_MS ago.
 * Called periodically by the cleanup timer.
 */
function cleanupOldExecutions(): void {
	const cutoff = Date.now() - CLEANUP_INTERVAL_MS;
	store.update((s) => {
		const newExecs = new Map(s.executions);
		for (const [id, exec] of newExecs) {
			if (exec.status !== 'running' && exec.completedAt && exec.completedAt < cutoff) {
				newExecs.delete(id);
			}
		}
		return { ...s, executions: newExecs };
	});
}

// ============================================================================
// Public store API
// ============================================================================

/**
 * Background workflows store.
 *
 * Central dispatch point for all workflow streaming events. Manages concurrent
 * workflow executions, routes events to the streaming store for the viewed
 * workflow, and triggers toast notifications for background events.
 */
export const backgroundWorkflowsStore = {
	subscribe: store.subscribe,

	/**
	 * Initialize the store by registering Tauri event listeners and starting
	 * the cleanup timer. Safe to call multiple times (will destroy and re-init).
	 */
	async init(): Promise<void> {
		if (isInitialized) {
			await this.destroy();
		}

		const unlistenChunk = await listen<StreamChunk>(
			STREAM_EVENTS.WORKFLOW_STREAM,
			(event) => {
				handleStreamChunk(event.payload);
			}
		);

		const unlistenComplete = await listen<WorkflowComplete>(
			STREAM_EVENTS.WORKFLOW_COMPLETE,
			(event) => {
				handleStreamComplete(event.payload);
			}
		);

		unlisteners = [unlistenChunk, unlistenComplete];
		cleanupTimer = setInterval(cleanupOldExecutions, CLEANUP_INTERVAL_MS);
		isInitialized = true;
	},

	/**
	 * Set callbacks for forwarding stream events to the streaming store
	 * and user question events to the userQuestionStore.
	 * Called by the agent page when it initializes.
	 *
	 * @param chunkCb - Callback to forward StreamChunk events for the viewed workflow
	 * @param completeCb - Callback to forward WorkflowComplete events for the viewed workflow
	 * @param userQuestionCb - Callback to handle user_question_start chunks for all workflows
	 */
	setForwardCallbacks(
		chunkCb: (chunk: StreamChunk) => void,
		completeCb: (complete: WorkflowComplete) => void,
		userQuestionCb: (payload: UserQuestionStreamPayload, workflowId: string, isViewed: boolean) => void
	): void {
		onChunkForViewed = chunkCb;
		onCompleteForViewed = completeCb;
		onUserQuestion = userQuestionCb;
	},

	/**
	 * Register a new workflow execution for background tracking.
	 * Must be called before the workflow starts emitting events.
	 *
	 * @param workflowId - Unique workflow identifier
	 * @param agentId - Agent executing the workflow
	 * @param workflowName - Human-readable workflow name
	 */
	register(workflowId: string, agentId: string, workflowName: string): void {
		store.update((s) => {
			const newExecs = new Map(s.executions);
			newExecs.set(workflowId, createInitialExecution(workflowId, agentId, workflowName));
			return { ...s, executions: newExecs };
		});
	},

	/**
	 * Check whether a new workflow can be started based on the current
	 * number of running workflows and the configured concurrency limit.
	 *
	 * @returns true if a new workflow can be started
	 */
	canStart(): boolean {
		const state = get(store);
		const runningCount = Array.from(state.executions.values()).filter(
			(e) => e.status === 'running'
		).length;
		const validationState = get(validationSettings);
		const maxConcurrent =
			validationState?.mode === 'auto' ? MAX_CONCURRENT_AUTO : MAX_CONCURRENT_OTHER;
		return runningCount < maxConcurrent;
	},

	/**
	 * Get the maximum number of concurrent workflows allowed based on
	 * the current validation settings.
	 *
	 * @returns Maximum concurrent workflow count
	 */
	getMaxConcurrent(): number {
		const validationState = get(validationSettings);
		return validationState?.mode === 'auto' ? MAX_CONCURRENT_AUTO : MAX_CONCURRENT_OTHER;
	},

	/**
	 * Get the currently-viewed workflow ID.
	 *
	 * @returns The viewed workflow ID, or null if none
	 */
	getViewedWorkflowId(): string | null {
		return get(store).viewedWorkflowId;
	},

	/**
	 * Set the currently-viewed workflow ID. Chunks for this workflow will
	 * be forwarded to the streaming store via the registered callbacks.
	 *
	 * @param workflowId - Workflow ID to mark as viewed, or null to clear
	 */
	setViewed(workflowId: string | null): void {
		store.update((s) => ({ ...s, viewedWorkflowId: workflowId }));
	},

	/**
	 * Get the current execution state for a specific workflow.
	 *
	 * @param workflowId - Workflow ID to look up
	 * @returns Execution state or undefined if not tracked
	 */
	getExecution(workflowId: string): WorkflowStreamState | undefined {
		return get(store).executions.get(workflowId);
	},

	/**
	 * Update the pending question flag for a workflow.
	 *
	 * @param workflowId - Workflow ID to update
	 * @param value - Whether a user question is pending
	 */
	setHasPendingQuestion(workflowId: string, value: boolean): void {
		store.update((s) => {
			const exec = s.executions.get(workflowId);
			if (!exec) return s;
			const newExecs = new Map(s.executions);
			newExecs.set(workflowId, { ...exec, hasPendingQuestion: value });
			return { ...s, executions: newExecs };
		});
	},

	/**
	 * Remove a workflow execution from tracking entirely.
	 *
	 * @param workflowId - Workflow ID to remove
	 */
	remove(workflowId: string): void {
		store.update((s) => {
			const newExecs = new Map(s.executions);
			newExecs.delete(workflowId);
			return { ...s, executions: newExecs };
		});
	},

	/**
	 * Tear down the store by unregistering all Tauri event listeners,
	 * stopping the cleanup timer, clearing callbacks, and resetting state.
	 */
	async destroy(): Promise<void> {
		for (const unlisten of unlisteners) {
			unlisten();
		}
		unlisteners = [];
		if (cleanupTimer) {
			clearInterval(cleanupTimer);
			cleanupTimer = null;
		}
		onChunkForViewed = null;
		onCompleteForViewed = null;
		onUserQuestion = null;
		isInitialized = false;
		store.set(initialState);
	}
};

// ============================================================================
// Derived stores
// ============================================================================

/** All currently running workflow executions */
export const runningWorkflows = derived(store, ($s) =>
	Array.from($s.executions.values()).filter((e) => e.status === 'running')
);

/** All completed, errored, or cancelled workflow executions */
export const recentlyCompletedWorkflows = derived(store, ($s) =>
	Array.from($s.executions.values()).filter((e) => e.status !== 'running')
);

/** Count of currently running workflows */
export const runningCount = derived(store, ($s) =>
	Array.from($s.executions.values()).filter((e) => e.status === 'running').length
);

/** Whether a new workflow can be started (reactive) */
export const canStartNew = derived([store, validationSettings], ([$s, $vs]) => {
	const running = Array.from($s.executions.values()).filter(
		(e) => e.status === 'running'
	).length;
	const maxConcurrent =
		$vs?.mode === 'auto' ? MAX_CONCURRENT_AUTO : MAX_CONCURRENT_OTHER;
	return running < maxConcurrent;
});

/** The execution state for the currently-viewed workflow, or null */
export const viewedExecution = derived(store, ($s) =>
	$s.viewedWorkflowId ? ($s.executions.get($s.viewedWorkflowId) ?? null) : null
);

/** Set of workflow IDs that are currently running */
export const runningWorkflowIds = derived(runningWorkflows, ($rw) =>
	new Set($rw.map((e) => e.workflowId))
);

/** Set of workflow IDs that have completed (any terminal status) */
export const recentlyCompletedIds = derived(recentlyCompletedWorkflows, ($rc) =>
	new Set($rc.map((e) => e.workflowId))
);

/** Set of workflow IDs that have a pending user question */
export const questionPendingIds = derived(store, ($s) =>
	new Set(
		Array.from($s.executions.values())
			.filter((e) => e.hasPendingQuestion)
			.map((e) => e.workflowId)
	)
);
