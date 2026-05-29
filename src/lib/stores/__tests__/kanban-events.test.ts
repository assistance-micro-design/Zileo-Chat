/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	kanbanEventsStore,
	analyzingCardIds,
	boardDirtySeq,
	pendingVerdict,
	pendingNeedsImprovement
} from '../kanban-events';
import { tauriListen } from '$lib/tauri';

vi.mock('$lib/tauri');

type Handler = (event: { payload: unknown }) => void;

describe('kanbanEventsStore', () => {
	const handlers: Record<string, Handler> = {};
	let listenCallCount: number;
	let unlistenCount: number;

	/** Invoke a captured listener, asserting it was registered. */
	function fire(event: string, payload: unknown): void {
		const handler = handlers[event];
		if (!handler) throw new Error(`no handler registered for ${event}`);
		handler({ payload });
	}

	beforeEach(() => {
		for (const key of Object.keys(handlers)) delete handlers[key];
		listenCallCount = 0;
		unlistenCount = 0;
		vi.mocked(tauriListen).mockImplementation(async (eventName, handler) => {
			handlers[eventName] = handler as unknown as Handler;
			listenCallCount++;
			return () => {
				unlistenCount++;
			};
		});
		kanbanEventsStore.destroy();
	});

	afterEach(() => {
		kanbanEventsStore.destroy();
		vi.clearAllMocks();
	});

	it('registers all five lifecycle listeners on init', async () => {
		await kanbanEventsStore.init();
		expect(Object.keys(handlers).sort()).toEqual(
			[
				'kanban:analyzing',
				'kanban:auto_analyzed',
				'kanban:cards_purged',
				'kanban:needs_improvement',
				'workflow_complete'
			].sort()
		);
	});

	it('is idempotent: concurrent and repeat init calls register the listeners once', async () => {
		await Promise.all([kanbanEventsStore.init(), kanbanEventsStore.init()]);
		await kanbanEventsStore.init();
		expect(listenCallCount).toBe(5);
	});

	it('auto_analyzed clears the analyzing flag, bumps the board and buffers the verdict', async () => {
		await kanbanEventsStore.init();

		fire('kanban:analyzing', { card_id: 'c1' });
		expect(get(analyzingCardIds)).toEqual(['c1']);

		fire('kanban:auto_analyzed', { card_id: 'c1', verdict: 'approve', reasoning: 'good' });

		expect(get(analyzingCardIds)).toEqual([]);
		expect(get(boardDirtySeq)).toBe(1);
		expect(get(pendingVerdict)).toEqual({ cardId: 'c1', verdict: 'approve', reasoning: 'good' });
	});

	it('needs_improvement buffers the suggested edit and bumps the board', async () => {
		await kanbanEventsStore.init();

		fire('kanban:needs_improvement', {
			card_id: 'c2',
			reasoning: 'tweak the prompt',
			suggested_prompt_edit: 'new prompt'
		});

		expect(get(pendingNeedsImprovement)).toEqual({
			cardId: 'c2',
			reasoning: 'tweak the prompt',
			suggestedPromptEdit: 'new prompt'
		});
		expect(get(boardDirtySeq)).toBe(1);
	});

	it('ignores lifecycle events carrying no card_id', async () => {
		await kanbanEventsStore.init();

		fire('kanban:auto_analyzed', {});
		fire('kanban:needs_improvement', {});

		expect(get(boardDirtySeq)).toBe(0);
		expect(get(pendingVerdict)).toBeNull();
		expect(get(pendingNeedsImprovement)).toBeNull();
	});

	it('workflow_complete and cards_purged each bump the board dirty counter', async () => {
		await kanbanEventsStore.init();

		fire('workflow_complete', {});
		fire('kanban:cards_purged', { card_ids: ['x'] });

		expect(get(boardDirtySeq)).toBe(2);
	});

	it('clearVerdict and clearNeedsImprovement drain the buffers', async () => {
		await kanbanEventsStore.init();
		fire('kanban:auto_analyzed', { card_id: 'c1', verdict: 'reject', reasoning: 'no' });
		fire('kanban:needs_improvement', {
			card_id: 'c2',
			reasoning: 'r',
			suggested_prompt_edit: null
		});

		kanbanEventsStore.clearVerdict();
		kanbanEventsStore.clearNeedsImprovement();

		expect(get(pendingVerdict)).toBeNull();
		expect(get(pendingNeedsImprovement)).toBeNull();
	});

	it('destroy unlistens every registered listener and resets state', async () => {
		await kanbanEventsStore.init();
		fire('kanban:analyzing', { card_id: 'c1' });

		kanbanEventsStore.destroy();

		expect(unlistenCount).toBe(5);
		expect(get(analyzingCardIds)).toEqual([]);
		expect(get(boardDirtySeq)).toBe(0);
	});
});
