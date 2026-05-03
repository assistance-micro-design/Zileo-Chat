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
 * User question store for managing interactive question-answer sessions during workflow execution.
 * Works in coordination with backgroundWorkflowsStore, which owns all event listeners and
 * dispatches question events to this store with workflow context.
 * @module stores/user-question
 */

import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type {
	UserQuestion,
	UserQuestionResponse,
	UserQuestionStreamPayload
} from '$types/user-question';
import { getErrorMessage } from '$lib/utils/error';
import { backgroundWorkflowsStore } from './background-workflows';
import { toastStore } from './toast';

/**
 * Maximum number of pending questions to prevent memory issues
 */
const MAX_PENDING_QUESTIONS = 50;

/**
 * State interface for the user question store
 */
export interface UserQuestionState {
	/** List of pending questions waiting for user response */
	pendingQuestions: UserQuestion[];
	/** Currently displayed question in modal */
	currentQuestion: UserQuestion | null;
	/** Modal visibility state */
	isModalOpen: boolean;
	/** Submission loading state */
	isSubmitting: boolean;
	/** Error message if any */
	error: string | null;
}

/**
 * Initial state for the user question store
 */
const initialState: UserQuestionState = {
	pendingQuestions: [],
	currentQuestion: null,
	isModalOpen: false,
	isSubmitting: false,
	error: null
};

/**
 * Internal writable store
 */
const store = writable<UserQuestionState>(initialState);

/**
 * User question store with actions for managing interactive question-answer flows.
 * Event listeners are owned by backgroundWorkflowsStore; this store receives
 * question events via handleQuestionForWorkflow.
 */
export const userQuestionStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Initializes the store by resetting state.
	 * Event listeners are managed by backgroundWorkflowsStore.
	 */
	init(): void {
		store.set(initialState);
	},

	/**
	 * Handles a question event with workflow-aware modal behavior.
	 * If the question belongs to the currently viewed workflow, opens the modal.
	 * Otherwise, only queues the question (toast notification is handled by backgroundWorkflowsStore).
	 *
	 * @param payload - Question data from streaming event
	 * @param workflowId - ID of the workflow this question belongs to
	 * @param isViewed - Whether the workflow is currently being viewed in the UI
	 */
	handleQuestionForWorkflow(
		payload: UserQuestionStreamPayload,
		workflowId: string,
		isViewed: boolean
	): void {
		const question: UserQuestion = {
			id: payload.questionId,
			workflowId,
			agentId: '',
			question: payload.question,
			questionType: payload.questionType,
			options: payload.options,
			textPlaceholder: payload.textPlaceholder,
			textRequired: payload.textRequired,
			context: payload.context,
			status: 'pending',
			createdAt: new Date().toISOString()
		};

		store.update((s) => {
			const newPending = [...s.pendingQuestions, question].slice(-MAX_PENDING_QUESTIONS);

			if (isViewed) {
				return {
					...s,
					pendingQuestions: newPending,
					currentQuestion: s.currentQuestion ?? question,
					isModalOpen: true,
					error: null
				};
			}

			// Non-viewed workflow: queue only, do not open modal
			return {
				...s,
				pendingQuestions: newPending
			};
		});
	},

	/**
	 * Returns pending questions filtered by workflow ID.
	 *
	 * @param workflowId - Workflow ID to filter by
	 * @returns List of pending questions for the specified workflow
	 */
	getQuestionsForWorkflow(workflowId: string): UserQuestion[] {
		return get(store).pendingQuestions.filter((q) => q.workflowId === workflowId);
	},

	/**
	 * Opens the question modal for a specific workflow.
	 * Sets currentQuestion to the first pending question for that workflow and opens modal.
	 * Used when the user clicks "Go to workflow" on a toast notification.
	 *
	 * @param workflowId - Workflow ID to open questions for
	 */
	openForWorkflow(workflowId: string): void {
		store.update((s) => {
			const workflowQuestions = s.pendingQuestions.filter(
				(q) => q.workflowId === workflowId
			);
			if (workflowQuestions.length === 0) return s;

			return {
				...s,
				currentQuestion: workflowQuestions[0],
				isModalOpen: true,
				error: null
			};
		});
	},

	/**
	 * Submits user's response to a question.
	 * Removes question from pending queue and advances to next question if available.
	 * Updates backgroundWorkflowsStore and toastStore for the affected workflow.
	 * @param response - User's answer (selected options and/or text)
	 */
	async submitResponse(response: UserQuestionResponse): Promise<void> {
		store.update((s) => ({ ...s, isSubmitting: true, error: null }));

		// Capture workflowId before removing the question
		const answeredWorkflowId =
			get(store).pendingQuestions.find((q) => q.id === response.questionId)?.workflowId ?? '';

		try {
			await invoke('submit_user_response', {
				questionId: response.questionId,
				workflowId: answeredWorkflowId,
				selectedOptions: response.selectedOptions,
				textResponse: response.textResponse
			});

			store.update((s) => {
				const remaining = s.pendingQuestions.filter((q) => q.id !== response.questionId);
				return {
					...s,
					pendingQuestions: remaining,
					currentQuestion: remaining[0] ?? null,
					isModalOpen: remaining.length > 0,
					isSubmitting: false
				};
			});

			// Update background workflows and toast state
			if (answeredWorkflowId) {
				const remainingForWorkflow = this.getQuestionsForWorkflow(answeredWorkflowId);
				const hasMore = remainingForWorkflow.length > 0;
				backgroundWorkflowsStore.setHasPendingQuestion(answeredWorkflowId, hasMore);
				if (!hasMore) {
					toastStore.dismissForWorkflow(answeredWorkflowId);
				}
			}
		} catch (e) {
			const message = getErrorMessage(e);
			store.update((s) => ({
				...s,
				isSubmitting: false,
				error: message
			}));
		}
	},

	/**
	 * Skips a question without providing an answer.
	 * Removes question from pending queue and advances to next question if available.
	 * Updates backgroundWorkflowsStore and toastStore for the affected workflow.
	 * @param questionId - ID of the question to skip
	 */
	async skipQuestion(questionId: string): Promise<void> {
		store.update((s) => ({ ...s, isSubmitting: true, error: null }));

		// Capture workflowId before removing the question
		const skippedWorkflowId =
			get(store).pendingQuestions.find((q) => q.id === questionId)?.workflowId ?? '';

		try {
			await invoke('skip_question', { questionId, workflowId: skippedWorkflowId });

			store.update((s) => {
				const remaining = s.pendingQuestions.filter((q) => q.id !== questionId);
				return {
					...s,
					pendingQuestions: remaining,
					currentQuestion: remaining[0] ?? null,
					isModalOpen: remaining.length > 0,
					isSubmitting: false
				};
			});

			// Update background workflows and toast state
			if (skippedWorkflowId) {
				const remainingForWorkflow = this.getQuestionsForWorkflow(skippedWorkflowId);
				const hasMore = remainingForWorkflow.length > 0;
				backgroundWorkflowsStore.setHasPendingQuestion(skippedWorkflowId, hasMore);
				if (!hasMore) {
					toastStore.dismissForWorkflow(skippedWorkflowId);
				}
			}
		} catch (e) {
			const message = getErrorMessage(e);
			store.update((s) => ({
				...s,
				isSubmitting: false,
				error: message
			}));
		}
	},

	/**
	 * Closes the question modal without removing questions from queue.
	 * User can reopen modal later if questions are still pending.
	 */
	closeModal(): void {
		store.update((s) => ({ ...s, isModalOpen: false }));
	},

	/**
	 * Clears the current error message.
	 */
	clearError(): void {
		store.update((s) => ({ ...s, error: null }));
	},

	/**
	 * Resets store to initial state.
	 * No event listeners to clean up since backgroundWorkflowsStore owns them.
	 */
	cleanup(): void {
		store.set(initialState);
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: list of pending questions
 */
export const pendingQuestions = derived(store, ($s) => $s.pendingQuestions);

/**
 * Derived store: currently displayed question
 */
export const currentQuestion = derived(store, ($s) => $s.currentQuestion);

/**
 * Derived store: modal visibility state
 */
export const isModalOpen = derived(store, ($s) => $s.isModalOpen);

/**
 * Derived store: submission loading state
 */
export const isSubmitting = derived(store, ($s) => $s.isSubmitting);

/**
 * Derived store: error message
 */
export const userQuestionError = derived(store, ($s) => $s.error);

/**
 * Derived store: number of pending questions
 */
export const pendingCount = derived(store, ($s) => $s.pendingQuestions.length);
