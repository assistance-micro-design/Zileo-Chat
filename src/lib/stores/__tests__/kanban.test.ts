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
import { kanbanStore, kanbanCards, kanbanCardsByColumn, kanbanError } from '../kanban';
import { tauriInvoke } from '$lib/tauri';
import type { KanbanCard, KanbanColumn } from '$types/kanban';

vi.mock('$lib/tauri');

function card(
	id: string,
	column: KanbanColumn,
	columnOrder: number,
	extra: Partial<KanbanCard> = {}
): KanbanCard {
	return {
		id,
		title: `card-${id}`,
		description: '',
		kanban_agent_id: 'ka',
		target_agent_id: 'ta',
		variables: '{}',
		status: 'ready',
		column,
		column_order: columnOrder,
		created_at: '2026-01-01T00:00:00Z',
		updated_at: '2026-01-01T00:00:00Z',
		...extra
	};
}

describe('kanbanStore.loadCards', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		kanbanStore.reset();
	});
	afterEach(() => kanbanStore.reset());

	it('passes the agent filter (camelCase) and populates the list', async () => {
		const cards = [card('a', 'todo', 0)];
		vi.mocked(tauriInvoke).mockResolvedValue(cards);

		await kanbanStore.loadCards('agent-1');

		expect(tauriInvoke).toHaveBeenCalledWith('list_kanban_cards', { kanbanAgentId: 'agent-1' });
		expect(get(kanbanCards)).toEqual(cards);
	});

	it('passes null when no agent filter is given', async () => {
		vi.mocked(tauriInvoke).mockResolvedValue([]);

		await kanbanStore.loadCards();

		expect(tauriInvoke).toHaveBeenCalledWith('list_kanban_cards', { kanbanAgentId: null });
	});

	it('captures backend errors into the error store and stops loading', async () => {
		vi.mocked(tauriInvoke).mockRejectedValue(new Error('boom'));

		await kanbanStore.loadCards();

		expect(get(kanbanError)).toBe('boom');
		expect(get(kanbanCards)).toEqual([]);
	});
});

describe('kanbanStore.moveCard', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		kanbanStore.reset();
	});
	afterEach(() => kanbanStore.reset());

	it('replaces the moved card in place, leaving siblings untouched', async () => {
		vi.mocked(tauriInvoke).mockResolvedValueOnce([card('a', 'todo', 0), card('b', 'todo', 1)]);
		await kanbanStore.loadCards();

		const moved = card('a', 'doing', 0);
		vi.mocked(tauriInvoke).mockResolvedValueOnce(moved);

		const result = await kanbanStore.moveCard('a', 'doing', 0);

		expect(tauriInvoke).toHaveBeenLastCalledWith('move_kanban_card', {
			cardId: 'a',
			newColumn: 'doing',
			newOrder: 0
		});
		expect(result).toEqual(moved);
		const items = get(kanbanCards);
		expect(items.find((c) => c.id === 'a')?.column).toBe('doing');
		expect(items.find((c) => c.id === 'b')?.column).toBe('todo');
	});

	it('rethrows and records the error when the backend rejects the transition', async () => {
		vi.mocked(tauriInvoke).mockRejectedValueOnce(new Error('forbidden transition'));

		await expect(kanbanStore.moveCard('a', 'doing', 0)).rejects.toThrow('forbidden transition');
		expect(get(kanbanError)).toBe('forbidden transition');
	});
});

describe('kanbanStore.upsertLocal', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		kanbanStore.reset();
	});
	afterEach(() => kanbanStore.reset());

	it('updates an existing card in place without duplicating it', async () => {
		vi.mocked(tauriInvoke).mockResolvedValueOnce([card('a', 'todo', 0)]);
		await kanbanStore.loadCards();

		kanbanStore.upsertLocal(card('a', 'done', 5));

		const items = get(kanbanCards);
		expect(items).toHaveLength(1);
		expect(items[0]?.column).toBe('done');
		expect(items[0]?.column_order).toBe(5);
	});

	it('appends a card not yet present in the list', async () => {
		vi.mocked(tauriInvoke).mockResolvedValueOnce([card('a', 'todo', 0)]);
		await kanbanStore.loadCards();

		kanbanStore.upsertLocal(card('b', 'doing', 0));

		const items = get(kanbanCards);
		expect(items.map((c) => c.id).sort()).toEqual(['a', 'b']);
	});
});

describe('kanbanCardsByColumn', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		kanbanStore.reset();
	});
	afterEach(() => kanbanStore.reset());

	it('groups cards by column and sorts each group by column_order', async () => {
		vi.mocked(tauriInvoke).mockResolvedValueOnce([
			card('a', 'todo', 2),
			card('b', 'todo', 0),
			card('c', 'doing', 1),
			card('d', 'todo', 1)
		]);
		await kanbanStore.loadCards();

		const groups = get(kanbanCardsByColumn);
		expect(groups.todo.map((c) => c.id)).toEqual(['b', 'd', 'a']);
		expect(groups.doing.map((c) => c.id)).toEqual(['c']);
		expect(groups.review).toEqual([]);
		expect(groups.done).toEqual([]);
	});
});
