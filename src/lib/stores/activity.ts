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
 * Activity store for managing workflow activity events.
 * Provides reactive state management for historical and streaming activities.
 * @module stores/activity
 */

import { writable, derived, get } from 'svelte/store';
import type { WorkflowActivityEvent, ActivityFilter } from '$types/activity';
import { ActivityService } from '$lib/services/activity.service';
import { streamingStore } from './streaming';
import {
	activeToolToActivity,
	activeReasoningToActivity,
	activeSubAgentToActivity,
	activeTaskToActivity
} from '$lib/utils/activity';

/**
 * State interface for the activity store
 */
interface ActivityState {
	/** Historical activities loaded from database */
	historical: WorkflowActivityEvent[];
	/** Current activity filter */
	filter: ActivityFilter;
	/** Loading state indicator */
	loading: boolean;
	/** Error message if any */
	error: string | null;
	/** Current workflow ID being tracked */
	currentWorkflowId: string | null;
}

/**
 * Initial state for the activity store
 */
const initialState: ActivityState = {
	historical: [],
	filter: 'all',
	loading: false,
	error: null,
	currentWorkflowId: null
};

/**
 * Internal writable store
 */
const store = writable<ActivityState>(initialState);

/**
 * Guard to prevent duplicate activity captures for the same workflow execution.
 * Tracks the workflow ID of the last successful capture. Reset when the store is reset.
 * This prevents the race condition where captureStreamingActivities could be called
 * multiple times before streamingStore.reset() clears the streaming state (SA-011 H-001).
 */
let lastCapturedWorkflowId: string | null = null;

/**
 * Activity store with actions for managing workflow activity events.
 * Combines historical activities from database with live streaming activities.
 */
export const activityStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Load historical activities for a workflow from database.
	 * Fetches all activity types (tools, thinking, sub-agents, tasks) in parallel.
	 *
	 * @param workflowId - The workflow ID to load activities for
	 */
	async loadHistorical(workflowId: string): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null, currentWorkflowId: workflowId }));

		try {
			const data = await ActivityService.loadAll(workflowId);
			const activities = ActivityService.convertToActivities(data);
			store.update((s) => ({ ...s, historical: activities, loading: false }));
		} catch (e) {
			const error = e instanceof Error ? e.message : String(e);
			store.update((s) => ({ ...s, error, loading: false }));
		}
	},

	/**
	 * Set the activity filter type.
	 *
	 * @param filter - The filter to apply
	 */
	setFilter(filter: ActivityFilter): void {
		store.update((s) => ({ ...s, filter }));
	},

	/**
	 * Capture current streaming activities and merge into historical.
	 * Called when streaming completes to persist live activities.
	 *
	 * This converts all active streaming state (tools, reasoning, sub-agents, tasks)
	 * into WorkflowActivityEvent format and prepends them to historical activities.
	 *
	 * Includes a guard to prevent duplicate captures for the same workflow execution
	 * (SA-011 H-001). Returns false if capture was skipped (duplicate or empty).
	 *
	 * @param workflowId - The workflow ID being captured (used for dedup guard)
	 * @returns true if activities were captured, false if skipped
	 */
	captureStreamingActivities(workflowId: string): boolean {
		// Guard: prevent duplicate capture for the same workflow execution
		if (workflowId === lastCapturedWorkflowId) {
			return false;
		}

		const streamingState = get(streamingStore);

		const newActivities: WorkflowActivityEvent[] = [
			...streamingState.tools.map((t, i) => activeToolToActivity(t, i)),
			...streamingState.reasoning.map((r, i) => activeReasoningToActivity(r, i)),
			...streamingState.subAgents.map((a, i) => activeSubAgentToActivity(a, i)),
			...streamingState.tasks.map((t, i) => activeTaskToActivity(t, i))
		];

		// Guard: skip if no streaming activities to capture
		if (newActivities.length === 0) {
			return false;
		}

		// Mark as captured for this workflow
		lastCapturedWorkflowId = workflowId;

		const currentState = get(store);

		// Merge and sort by timestamp (most recent first)
		const merged = [...newActivities, ...currentState.historical].sort(
			(a, b) => b.timestamp - a.timestamp
		);

		store.update((s) => ({ ...s, historical: merged }));
		return true;
	},

	/**
	 * Reset store to initial state.
	 * Clears all activities, errors, and the capture guard.
	 */
	reset(): void {
		lastCapturedWorkflowId = null;
		store.set(initialState);
	},

	/**
	 * Clear error message.
	 */
	clearError(): void {
		store.update((s) => ({ ...s, error: null }));
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: historical activities from database
 */
export const historicalActivities = derived(store, ($s) => $s.historical);

/**
 * Derived store: current activity filter
 */
export const activityFilter = derived(store, ($s) => $s.filter);

/**
 * Derived store: loading state
 */
export const activityLoading = derived(store, ($s) => $s.loading);

/**
 * Derived store: error message
 */
export const activityError = derived(store, ($s) => $s.error);

/**
 * Derived store: combined activities (historical + current streaming).
 *
 * When streaming is active, converts streaming state to activities and combines
 * with historical. When not streaming, returns only historical activities.
 */
export const allActivities = derived([store, streamingStore], ([$activity, $streaming]) => {
	// Convert streaming state to activities (may be empty if not streaming)
	const streamingActivities: WorkflowActivityEvent[] = [
		...$streaming.tools.map((t, i) => activeToolToActivity(t, i)),
		...$streaming.reasoning.map((r, i) => activeReasoningToActivity(r, i)),
		...$streaming.subAgents.map((a, i) => activeSubAgentToActivity(a, i)),
		...$streaming.tasks.map((t, i) => activeTaskToActivity(t, i))
	];

	// Combine streaming + historical
	const combined = [...streamingActivities, ...$activity.historical];

	// Deduplicate by ID to handle race conditions between capture and reset
	// This is robust regardless of timing between captureStreamingActivities() and reset()
	const seen = new Set<string>();
	const deduplicated = combined.filter((activity) => {
		if (seen.has(activity.id)) return false;
		seen.add(activity.id);
		return true;
	});

	// Sort by timestamp (most recent first)
	return deduplicated.sort((a, b) => b.timestamp - a.timestamp);
});

/**
 * Derived store: filtered activities based on current filter.
 *
 * Applies the current filter to combined activities (historical + streaming).
 */
export const filteredActivities = derived([allActivities, activityFilter], ([$all, $filter]) => {
	if ($filter === 'all') return $all;

	const typeMap: Record<ActivityFilter, string[]> = {
		all: [],
		tools: ['tool_start', 'tool_complete', 'tool_error'],
		agents: ['sub_agent_start', 'sub_agent_progress', 'sub_agent_complete', 'sub_agent_error'],
		reasoning: ['reasoning'],
		todos: ['task_create', 'task_update', 'task_complete']
	};

	const types = typeMap[$filter];
	return $all.filter((a) => types.includes(a.type));
});
