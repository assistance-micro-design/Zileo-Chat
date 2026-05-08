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
