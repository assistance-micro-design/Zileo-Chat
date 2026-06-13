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

import { describe, it, expect, beforeEach, vi, type Mock } from 'vitest';
import { get } from 'svelte/store';
import { skillStore, skills, skillError } from '../skills';
import type { SkillSummary } from '$types/skill';

// Mock Tauri's invoke function
vi.mock('$lib/tauri', () => ({
	tauriInvoke: vi.fn()
}));

import { tauriInvoke as invoke } from '$lib/tauri';

const mockInvoke = invoke as Mock;

const makeSkill = (id: string, enabled: boolean): SkillSummary => ({
	id,
	name: `skill-${id}`,
	description: `Description ${id}`,
	category: 'custom',
	enabled,
	content_length: 100,
	updated_at: '2026-06-13T10:00:00Z'
});

/** Route invoke mock by command name; `list_skills` returns the seeded list. */
function mockBackend(list: SkillSummary[], updateBehaviour: 'ok' | 'fail' = 'ok'): void {
	mockInvoke.mockImplementation((command: string) => {
		if (command === 'list_skills') {
			return Promise.resolve(list);
		}
		if (command === 'update_skill') {
			return updateBehaviour === 'ok'
				? Promise.resolve({})
				: Promise.reject(new Error('backend failure'));
		}
		return Promise.resolve(undefined);
	});
}

describe('Skill Store - toggleEnabled', () => {
	beforeEach(() => {
		mockInvoke.mockReset();
		skillStore.reset();
	});

	it('flips the flag in place without reloading or reordering the list', async () => {
		const seeded = [makeSkill('a', true), makeSkill('b', true), makeSkill('c', true)];
		mockBackend(seeded);
		await skillStore.loadSkills();

		mockInvoke.mockClear();
		await skillStore.toggleEnabled('a', false);

		// Only the targeted skill update was sent; no list reload (which would
		// re-sort by updated_at and make the toggled row jump).
		expect(mockInvoke).toHaveBeenCalledTimes(1);
		expect(mockInvoke).toHaveBeenCalledWith('update_skill', {
			skillId: 'a',
			config: { enabled: false }
		});

		const items = get(skills);
		expect(items.map((s) => s.id)).toEqual(['a', 'b', 'c']); // order preserved
		expect(items.find((s) => s.id === 'a')?.enabled).toBe(false);
		expect(items.find((s) => s.id === 'b')?.enabled).toBe(true);
	});

	it('reverts the optimistic update and records the error when the backend fails', async () => {
		const seeded = [makeSkill('a', true), makeSkill('b', true)];
		mockBackend(seeded, 'fail');
		await skillStore.loadSkills();

		await expect(skillStore.toggleEnabled('a', false)).rejects.toThrow();

		const items = get(skills);
		expect(items.find((s) => s.id === 'a')?.enabled).toBe(true); // reverted
		expect(get(skillError)).toBeTruthy();
	});
});
