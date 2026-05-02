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

import type {
	ActiveTool,
	ActiveReasoningStep,
	ActiveSubAgent,
	ActiveTask
} from '$lib/stores/streaming';

/**
 * Possible statuses for a background workflow execution.
 */
export type BackgroundWorkflowStatus = 'running' | 'completed' | 'error' | 'cancelled';

/**
 * Complete state snapshot of a workflow stream running in the background.
 *
 * Captures all streaming data (content, tools, reasoning, sub-agents, tasks)
 * so the user can review progress without being on the active workflow view.
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
	/** Accumulated text content from token chunks */
	content: string;
	/** Active tool executions */
	tools: ActiveTool[];
	/** Reasoning steps captured during execution */
	reasoning: ActiveReasoningStep[];
	/** Sub-agent activity */
	subAgents: ActiveSubAgent[];
	/** Task tracking */
	tasks: ActiveTask[];
	/** Total output tokens received so far (response_block.tokens_output) */
	tokensReceived: number;
	/**
	 * Cumulative input/prompt tokens reported by `response_block` chunks.
	 * Phase 13: kept on the bg execution itself so switching back to a still-
	 * running workflow restores the FULL session display, not just outputs.
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
	 * layer; the frontend only stores the running total. Phase 13 + Option A.
	 */
	partialCostUsd: number | null;
	/** Error message if status is 'error' */
	error: string | null;
	/** Timestamp (ms) when the workflow started */
	startedAt: number;
	/** Timestamp (ms) when the workflow completed, or null if still running */
	completedAt: number | null;
	/** Whether the workflow is waiting for user input */
	hasPendingQuestion: boolean;
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
