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
 * @fileoverview Types for background workflow execution and toast notifications.
 *
 * Defines the state shape for workflows running in the background
 * and the toast notification system used to surface workflow events.
 *
 * @module types/background-workflow
 */

import type { StreamChunk } from '$types/streaming';

/**
 * Sub-agent status during streaming. Distinct from the persisted
 * `SubAgentStatus` (in `$types/sub-agent`) which adds `pending` and
 * `cancelled` for DB rows; the streaming flavor only carries the values
 * the UI sees in real time.
 */
export type ActiveSubAgentStatus = 'starting' | 'running' | 'completed' | 'error';

/**
 * Active sub-agent captured from streaming chunks (sub_agent_start /
 * sub_agent_complete / sub_agent_error). Used both by the bg-workflows
 * timeline and by `workflowExecutor` to attach `SubAgentSummary` items
 * to assistant messages.
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
	status: ActiveSubAgentStatus;
	/** Timestamp when execution started */
	startedAt: number;
	/** Progress percentage (0-100) */
	progress: number;
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
 * Possible statuses for a background workflow execution.
 */
export type BackgroundWorkflowStatus = 'running' | 'completed' | 'error' | 'cancelled';

/**
 * State snapshot of a workflow stream running in the background.
 *
 * Holds the data that survives a workflow switch and that the UI consumes
 * after the streamingStore removal: token rollup, cost, sub-agent activity
 * and the chunk replay buffer. Per-block UI state (tools/reasoning/tasks/
 * content/error) is carried by `executionBlocksStore` and `tokenStore`,
 * not duplicated here.
 */
export interface WorkflowStreamState {
	/** Unique workflow identifier */
	workflowId: string;
	/** Agent executing the workflow */
	agentId: string;
	/** Human-readable workflow name */
	workflowName: string;
	/** Current execution status */
	status: BackgroundWorkflowStatus;
	/** Sub-agent activity */
	subAgents: ActiveSubAgent[];
	/** Total output tokens received so far (response_block.tokens_output) */
	tokensReceived: number;
	/**
	 * Cumulative input/prompt tokens reported by `response_block` chunks.
	 * Kept on the bg execution itself so switching back to a still-running
	 * workflow restores the FULL session display, not just outputs.
	 */
	tokensSent: number;
	/**
	 * Cached input tokens reported by the latest `response_block` chunk.
	 * `null` when the provider does not expose cache metrics.
	 */
	cachedTokens: number | null;
	/**
	 * Cache-write tokens reported by the latest `response_block` chunk.
	 * `null` when the provider does not expose cache-write metrics.
	 */
	cacheWriteTokens: number | null;
	/**
	 * Sum of `cost_usd` carried by every `response_block` chunk. `null` until
	 * the first chunk with a cost lands. Computed by the backend pricing
	 * layer; the frontend only stores the running total.
	 */
	partialCostUsd: number | null;
	/** Timestamp (ms) when the workflow started */
	startedAt: number;
	/** Timestamp (ms) when the workflow completed, or null if still running */
	completedAt: number | null;
	/** Whether the workflow is waiting for user input */
	hasPendingQuestion: boolean;
	/**
	 * Buffer of raw stream chunks received for this workflow.
	 *
	 * Used to reconstruct `executionBlocks` when the user switches BACK to a
	 * still-running workflow that was started in the background — without it
	 * the execution area appears empty until the next chunk arrives because
	 * `executionBlocksStore.start()` resets state on every switch (H3 audit
	 * 2026-05-02). Soft-capped at `MAX_CHUNK_HISTORY` to keep memory bounded
	 * on long-running workflows.
	 */
	chunkHistory: StreamChunk[];
}

/**
 * Visual variant for toast notifications.
 */
export type ToastType = 'success' | 'error' | 'info' | 'warning' | 'user-question';

/**
 * A toast notification displayed to the user.
 *
 * Toasts can be transient (auto-dismiss after duration) or persistent
 * (require manual dismissal, e.g. user-question toasts).
 */
export interface Toast {
	/** Unique toast identifier */
	id: string;
	/** Visual variant determining icon and border color */
	type: ToastType;
	/** Short heading text */
	title: string;
	/** Descriptive body text */
	message: string;
	/** Associated workflow ID for navigation (if applicable) */
	workflowId?: string;
	/** Whether the toast stays visible until manually dismissed */
	persistent: boolean;
	/** Auto-dismiss duration in milliseconds (0 for persistent toasts) */
	duration: number;
	/** Timestamp (ms) when the toast was created */
	createdAt: number;
}
