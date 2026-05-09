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

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriInvoke } from '$lib/tauri';
import { BlockService } from './block.service';
import type { Message } from '$types/message';

vi.mock('$lib/tauri', () => ({
	tauriInvoke: vi.fn()
}));

const invokeMock = vi.mocked(tauriInvoke);

function makeMessage(overrides: Partial<Message> = {}): Message {
	return {
		id: overrides.id ?? 'm1',
		workflow_id: overrides.workflow_id ?? 'wf1',
		role: overrides.role ?? 'assistant',
		content: overrides.content ?? '',
		tokens: overrides.tokens ?? 0,
		timestamp: overrides.timestamp ?? new Date().toISOString(),
		...overrides
	} as Message;
}

describe('BlockService.loadForMessages', () => {
	beforeEach(() => {
		invokeMock.mockReset();
	});

	it('returns an empty map when given no messages', async () => {
		const result = await BlockService.loadForMessages([]);
		expect(result.size).toBe(0);
		expect(invokeMock).not.toHaveBeenCalled();
	});

	it('invokes load_workflow_blocks once with the workflow id', async () => {
		invokeMock.mockResolvedValueOnce({});
		const messages = [
			makeMessage({ id: 'm1', workflow_id: 'wf-abc', role: 'user' }),
			makeMessage({ id: 'm2', workflow_id: 'wf-abc', role: 'assistant' }),
			makeMessage({ id: 'm3', workflow_id: 'wf-abc', role: 'assistant' })
		];

		await BlockService.loadForMessages(messages);

		expect(invokeMock).toHaveBeenCalledTimes(1);
		expect(invokeMock).toHaveBeenCalledWith('load_workflow_blocks', { workflowId: 'wf-abc' });
	});

	it('maps the batched response to the same shape as the legacy per-message API', async () => {
		const grouped = {
			'msg-assistant-1': [
				{
					block_type: 'thinking',
					sequence: 1,
					data: { content: 'analyzing', source: 'model_thinking' }
				}
			],
			'msg-assistant-2': []
		};
		invokeMock.mockResolvedValueOnce(grouped);

		const result = await BlockService.loadForMessages([
			makeMessage({ id: 'msg-assistant-1', workflow_id: 'wf-x', role: 'assistant' })
		]);

		expect(result.size).toBe(1);
		expect(result.get('msg-assistant-1')?.length).toBe(1);
		expect(result.get('msg-assistant-2')).toBeUndefined();
	});

	it('returns an empty map without throwing when the batched call fails', async () => {
		invokeMock.mockRejectedValueOnce(new Error('boom'));

		const result = await BlockService.loadForMessages([
			makeMessage({ id: 'm1', workflow_id: 'wf-y', role: 'assistant' })
		]);

		expect(result.size).toBe(0);
	});
});
