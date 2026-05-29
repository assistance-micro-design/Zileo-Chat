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
 * Unit tests for the user-question store.
 *
 * Regression coverage for H1 (audit 2026-05-02): backend Tauri commands
 * `submit_user_response` and `skip_question` must receive the `workflowId`
 * argument so that the resulting `user_question_complete` chunk carries
 * a non-empty workflow_id (otherwise the frontend dispatcher silently
 * drops it via `executions.get("")`).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import type { UserQuestionStreamPayload } from '$types/user-question';

const invokeMock = vi.fn();

vi.mock('$lib/tauri', () => ({
	tauriInvoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args)
}));

vi.mock('../background-workflows', () => ({
	backgroundWorkflowsStore: {
		setHasPendingQuestion: vi.fn()
	}
}));

vi.mock('../toast', () => ({
	toastStore: {
		dismissForWorkflow: vi.fn()
	}
}));

import { userQuestionStore } from '../user-question';

const WORKFLOW_ID = 'wf-h1-test';
const QUESTION_ID = 'q-h1-test';

function queueQuestion(): void {
	const payload: UserQuestionStreamPayload = {
		questionId: QUESTION_ID,
		question: 'Continue?',
		questionType: 'checkbox',
		options: undefined,
		textPlaceholder: undefined,
		textRequired: false,
		context: undefined
	};
	userQuestionStore.handleQuestionForWorkflow(payload, WORKFLOW_ID, true);
}

describe('userQuestionStore — H1 workflowId propagation', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
		userQuestionStore.cleanup();
	});

	it('submitResponse forwards workflowId to submit_user_response', async () => {
		queueQuestion();

		await userQuestionStore.submitResponse({
			questionId: QUESTION_ID,
			selectedOptions: ['yes'],
			textResponse: undefined
		});

		expect(invokeMock).toHaveBeenCalledTimes(1);
		const [cmd, args] = invokeMock.mock.calls[0]!;
		expect(cmd).toBe('submit_user_response');
		expect(args).toEqual({
			questionId: QUESTION_ID,
			workflowId: WORKFLOW_ID,
			selectedOptions: ['yes'],
			textResponse: undefined
		});
	});

	it('skipQuestion forwards workflowId to skip_question', async () => {
		queueQuestion();

		await userQuestionStore.skipQuestion(QUESTION_ID);

		expect(invokeMock).toHaveBeenCalledTimes(1);
		const [cmd, args] = invokeMock.mock.calls[0]!;
		expect(cmd).toBe('skip_question');
		expect(args).toEqual({
			questionId: QUESTION_ID,
			workflowId: WORKFLOW_ID
		});
	});
});

describe('userQuestionStore — openForWorkflow (toast "go to workflow" on any route)', () => {
	beforeEach(() => {
		invokeMock.mockReset();
		invokeMock.mockResolvedValue(undefined);
		userQuestionStore.cleanup();
	});

	// Locks the behavior the toast "go to workflow" button now relies on
	// (ToastContainer.handleNavigate → openForWorkflow): a question raised by a
	// worker on a route OTHER than /agent is buffered (modal closed), and the
	// toast button must be able to open the root-mounted modal in place so the
	// user can answer — previously a dead end off /agent.
	it('opens the modal for a queued non-viewed question', () => {
		const payload: UserQuestionStreamPayload = {
			questionId: QUESTION_ID,
			question: 'Continue?',
			questionType: 'checkbox',
			options: undefined,
			textPlaceholder: undefined,
			textRequired: false,
			context: undefined
		};
		// isViewed = false → queued only, modal stays closed.
		userQuestionStore.handleQuestionForWorkflow(payload, WORKFLOW_ID, false);
		expect(get(userQuestionStore).isModalOpen).toBe(false);

		userQuestionStore.openForWorkflow(WORKFLOW_ID);

		const state = get(userQuestionStore);
		expect(state.isModalOpen).toBe(true);
		expect(state.currentQuestion?.id).toBe(QUESTION_ID);
		expect(state.currentQuestion?.workflowId).toBe(WORKFLOW_ID);
	});

	// Safety: plain completion toasts (no queued question) must not pop an empty
	// modal — openForWorkflow is a no-op when nothing is queued for the workflow.
	it('is a no-op when no question is queued for the workflow', () => {
		userQuestionStore.openForWorkflow('wf-with-no-questions');

		const state = get(userQuestionStore);
		expect(state.isModalOpen).toBe(false);
		expect(state.currentQuestion).toBeNull();
	});
});
