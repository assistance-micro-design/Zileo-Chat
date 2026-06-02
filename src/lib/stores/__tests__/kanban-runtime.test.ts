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
import { kanbanRuntimeStore, maxConcurrentWorkflows } from '../kanban-runtime';
import { tauriInvoke } from '$lib/tauri';

vi.mock('$lib/tauri');

describe('kanbanRuntimeStore.load', () => {
	beforeEach(() => vi.clearAllMocks());
	afterEach(() => vi.clearAllMocks());

	it('fetches the cap from the backend and caches it (single IPC call)', async () => {
		vi.mocked(tauriInvoke).mockResolvedValue(3);

		await kanbanRuntimeStore.load();
		expect(get(maxConcurrentWorkflows)).toBe(3);
		expect(tauriInvoke).toHaveBeenCalledWith('get_max_concurrent_workflows');

		// A second load short-circuits — the value is already cached, no re-fetch.
		await kanbanRuntimeStore.load();
		expect(tauriInvoke).toHaveBeenCalledTimes(1);
		expect(kanbanRuntimeStore.get()).toBe(3);
	});
});
