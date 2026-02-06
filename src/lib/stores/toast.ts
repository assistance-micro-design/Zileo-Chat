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
 * @fileoverview Toast notification store for surfacing background workflow events.
 *
 * Provides a simple pub/sub store for adding, dismissing, and querying
 * toast notifications. Supports auto-dismiss for transient toasts and
 * persistent toasts for user-question events.
 *
 * @module stores/toast
 */

import { writable, derived } from 'svelte/store';
import type { Toast } from '$types/background-workflow';
import { t } from '$lib/i18n';

/** Maximum number of toasts visible at once */
const MAX_VISIBLE_TOASTS = 5;

/** Default auto-dismiss duration in milliseconds */
const DEFAULT_DURATION = 5000;

interface ToastState {
	toasts: Toast[];
}

const initialState: ToastState = {
	toasts: []
};

const store = writable<ToastState>(initialState);

/** Store for toast-initiated navigation requests (workflow ID to navigate to) */
const navigationStore = writable<string | null>(null);

/**
 * Toast notification store.
 *
 * Manages the lifecycle of toast notifications: creation, auto-dismissal,
 * manual dismissal, and bulk operations for workflow-related toasts.
 */
export const toastStore = {
	subscribe: store.subscribe,

	/**
	 * Add a new toast notification.
	 *
	 * @param toast - Toast data (id and createdAt are generated automatically)
	 * @returns The generated toast ID
	 */
	add(toast: Omit<Toast, 'id' | 'createdAt'>): string {
		const id = crypto.randomUUID();
		const newToast: Toast = {
			...toast,
			id,
			createdAt: Date.now()
		};

		store.update((s) => ({
			toasts: [...s.toasts, newToast].slice(-MAX_VISIBLE_TOASTS)
		}));

		// Auto-dismiss non-persistent toasts
		if (!toast.persistent) {
			setTimeout(() => {
				toastStore.dismiss(id);
			}, toast.duration || DEFAULT_DURATION);
		}

		return id;
	},

	/**
	 * Add a toast for a completed or failed background workflow.
	 *
	 * @param workflowId - The workflow that completed
	 * @param workflowName - Human-readable workflow name
	 * @param status - Whether the workflow completed successfully or with error
	 */
	addWorkflowComplete(workflowId: string, workflowName: string, status: 'completed' | 'error'): void {
		const isError = status === 'error';
		toastStore.add({
			type: isError ? 'error' : 'success',
			title: isError ? t('toast_workflow_failed') : t('toast_workflow_completed'),
			message: workflowName,
			workflowId,
			persistent: false,
			duration: DEFAULT_DURATION
		});
	},

	/**
	 * Add a persistent toast for a workflow that requires user input.
	 *
	 * @param workflowId - The workflow awaiting a response
	 * @param workflowName - Human-readable workflow name
	 * @param question - The question text to display
	 */
	addUserQuestion(workflowId: string, workflowName: string, question: string): void {
		toastStore.add({
			type: 'user-question',
			title: t('toast_question_pending'),
			message: `${workflowName}: ${question}`,
			workflowId,
			persistent: true,
			duration: 0
		});
	},

	/**
	 * Dismiss a single toast by ID.
	 *
	 * @param toastId - The toast to dismiss
	 */
	dismiss(toastId: string): void {
		store.update((s) => ({
			toasts: s.toasts.filter((t) => t.id !== toastId)
		}));
	},

	/**
	 * Dismiss all toasts associated with a specific workflow.
	 *
	 * @param workflowId - The workflow whose toasts should be removed
	 */
	dismissForWorkflow(workflowId: string): void {
		store.update((s) => ({
			toasts: s.toasts.filter((t) => t.workflowId !== workflowId)
		}));
	},

	/**
	 * Clear all toasts.
	 */
	clear(): void {
		store.set(initialState);
	},

	/**
	 * Request navigation to a specific workflow (triggered by toast action button).
	 * The agent page listens to the navigationTarget store and calls selectWorkflow.
	 *
	 * @param workflowId - The workflow to navigate to
	 */
	requestNavigation(workflowId: string): void {
		navigationStore.set(workflowId);
	},

	/**
	 * Clear the pending navigation request (called after the agent page has handled it).
	 */
	clearNavigation(): void {
		navigationStore.set(null);
	}
};

/** All current toasts */
export const toasts = derived(store, ($s) => $s.toasts);

/** Toasts limited to the visible maximum */
export const visibleToasts = derived(store, ($s) => $s.toasts.slice(-MAX_VISIBLE_TOASTS));

/** Whether any toasts are present */
export const hasToasts = derived(store, ($s) => $s.toasts.length > 0);

/** Pending navigation target (workflow ID) from a toast action button click */
export const navigationTarget = derived(navigationStore, ($n) => $n);
