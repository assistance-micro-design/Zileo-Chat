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
 * User question store for managing interactive question-answer sessions during workflow execution.
 * Listens to streaming events and manages pending questions via modal UI.
 * @module stores/userQuestion
 */

import { writable, derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
	UserQuestion,
	UserQuestionResponse,
	UserQuestionStreamPayload
} from '$types/user-question';
import type { StreamChunk } from '$types/streaming';

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
 * Event listener cleanup function
 */
let unlistenStream: UnlistenFn | null = null;

/**
 * Tracks whether the store has been initialized with event listeners
 */
let isInitialized = false;

/**
 * User question store with actions for managing interactive question-answer flows.
 * Automatically listens to streaming events during workflow execution.
 */
export const userQuestionStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Initializes the store by setting up streaming event listener.
	 * Should be called once on app initialization.
	 */
	async init(): Promise<void> {
		// Safety check: cleanup existing listener if already initialized
		if (isInitialized) {
			console.warn('[userQuestion] Store already initialized, cleaning up first');
			this.cleanup();
		}

		// Listen to streaming events for user_question_start
		unlistenStream = await listen<StreamChunk>('workflow_stream', (event) => {
			const chunk = event.payload;
			if (chunk.chunk_type === 'user_question_start' && chunk.user_question) {
				this.handleQuestionStart(chunk.user_question);
			}
		});

		isInitialized = true;
	},

	/**
	 * Handles incoming user_question_start streaming event.
	 * Creates a new UserQuestion and adds it to pending queue.
	 * @param payload - Question data from streaming event
	 */
	handleQuestionStart(payload: UserQuestionStreamPayload): void {
		const question: UserQuestion = {
			id: payload.questionId,
			workflowId: '',
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

		store.update((s) => ({
			...s,
			pendingQuestions: [...s.pendingQuestions, question],
			currentQuestion: s.currentQuestion ?? question,
			isModalOpen: true,
			error: null
		}));
	},

	/**
	 * Submits user's response to a question.
	 * Removes question from pending queue and advances to next question if available.
	 * @param response - User's answer (selected options and/or text)
	 * @throws Error if submission fails
	 */
	async submitResponse(response: UserQuestionResponse): Promise<void> {
		console.log('[userQuestionStore] submitResponse called:', response);
		store.update((s) => ({ ...s, isSubmitting: true, error: null }));

		try {
			console.log('[userQuestionStore] Invoking submit_user_response...');
			await invoke('submit_user_response', {
				questionId: response.questionId,
				selectedOptions: response.selectedOptions,
				textResponse: response.textResponse
			});
			console.log('[userQuestionStore] submit_user_response success');

			store.update((s) => {
				const remaining = s.pendingQuestions.filter((q) => q.id !== response.questionId);
				console.log('[userQuestionStore] Updated state, remaining questions:', remaining.length);
				return {
					...s,
					pendingQuestions: remaining,
					currentQuestion: remaining[0] ?? null,
					isModalOpen: remaining.length > 0,
					isSubmitting: false
				};
			});
		} catch (e) {
			console.error('[userQuestionStore] submitResponse error:', e);
			store.update((s) => ({
				...s,
				isSubmitting: false,
				error: String(e)
			}));
		}
	},

	/**
	 * Skips a question without providing an answer.
	 * Removes question from pending queue and advances to next question if available.
	 * @param questionId - ID of the question to skip
	 * @throws Error if skip operation fails
	 */
	async skipQuestion(questionId: string): Promise<void> {
		console.log('[userQuestionStore] skipQuestion called:', questionId);
		store.update((s) => ({ ...s, isSubmitting: true, error: null }));

		try {
			console.log('[userQuestionStore] Invoking skip_question...');
			await invoke('skip_question', { questionId });
			console.log('[userQuestionStore] skip_question success');

			store.update((s) => {
				const remaining = s.pendingQuestions.filter((q) => q.id !== questionId);
				console.log('[userQuestionStore] Updated state, remaining questions:', remaining.length);
				return {
					...s,
					pendingQuestions: remaining,
					currentQuestion: remaining[0] ?? null,
					isModalOpen: remaining.length > 0,
					isSubmitting: false
				};
			});
		} catch (e) {
			console.error('[userQuestionStore] skipQuestion error:', e);
			store.update((s) => ({
				...s,
				isSubmitting: false,
				error: String(e)
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
	 * Cleans up event listeners and resets store to initial state.
	 * Should be called on app cleanup/unmount.
	 */
	cleanup(): void {
		if (unlistenStream) {
			unlistenStream();
			unlistenStream = null;
		}
		isInitialized = false;
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
