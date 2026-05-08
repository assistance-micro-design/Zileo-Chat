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

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('$lib/tauri', () => ({
	tauriInvoke: vi.fn()
}));

vi.mock('$lib/services/localStorage.service', () => ({
	LocalStorage: {
		get: vi.fn(() => []),
		set: vi.fn()
	},
	STORAGE_KEYS: {
		EXPANDED_FOLDER_IDS: 'expanded_folder_ids'
	}
}));

import { folderStore } from '../folders';
import { tauriInvoke as invoke } from '$lib/tauri';

describe('folderStore error handling', () => {
	beforeEach(() => {
		folderStore.reset();
		vi.resetAllMocks();
	});

	it('createFolder sets error state and re-throws when invoke rejects', async () => {
		vi.mocked(invoke).mockRejectedValueOnce(new Error('backend down'));

		await expect(folderStore.createFolder('docs', '#fff')).rejects.toThrow('backend down');

		const state = get(folderStore);
		expect(state.error).toBe('backend down');
		expect(state.loading).toBe(false);
	});

	it('renameFolder sets error state and re-throws when invoke rejects', async () => {
		vi.mocked(invoke).mockRejectedValueOnce(new Error('not found'));
		await expect(folderStore.renameFolder('id-1', 'new')).rejects.toThrow('not found');
		expect(get(folderStore).error).toBe('not found');
		expect(get(folderStore).loading).toBe(false);
	});

	it('updateColor sets error state and re-throws when invoke rejects', async () => {
		vi.mocked(invoke).mockRejectedValueOnce(new Error('color invalid'));
		await expect(folderStore.updateColor('id-1', '#zzz')).rejects.toThrow('color invalid');
		expect(get(folderStore).error).toBe('color invalid');
	});

	it('deleteFolder sets error state and re-throws when invoke rejects', async () => {
		vi.mocked(invoke).mockRejectedValueOnce(new Error('cannot delete'));
		await expect(folderStore.deleteFolder('id-1')).rejects.toThrow('cannot delete');
		expect(get(folderStore).error).toBe('cannot delete');
	});

	it('reorderFolders sets error state and re-throws when invoke rejects', async () => {
		vi.mocked(invoke).mockRejectedValueOnce(new Error('reorder failed'));
		await expect(folderStore.reorderFolders(['a', 'b'])).rejects.toThrow('reorder failed');
		expect(get(folderStore).error).toBe('reorder failed');
	});

	it('clears error on the next successful action', async () => {
		vi.mocked(invoke).mockRejectedValueOnce(new Error('boom'));
		await expect(folderStore.deleteFolder('id-1')).rejects.toThrow();
		expect(get(folderStore).error).toBe('boom');

		// Next call succeeds — error must reset to null at the start.
		vi.mocked(invoke).mockResolvedValueOnce(undefined);
		await folderStore.reorderFolders([]);
		expect(get(folderStore).error).toBeNull();
	});
});
