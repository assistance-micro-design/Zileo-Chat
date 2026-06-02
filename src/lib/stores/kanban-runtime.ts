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
 * @fileoverview Kanban runtime configuration loaded once from the backend.
 *
 * Exposes the worker-promotion concurrency budget
 * (`DEFAULT_MAX_CONCURRENT_WORKFLOWS`) so the board's slot accounting reflects
 * the SAME cap the scheduler uses to promote `todo→doing`, instead of
 * recopying the literal. The value is a backend compile-time constant, so it
 * is fetched once at app boot and never changes for the session.
 *
 * @module stores/kanban-runtime
 */

import { writable, derived, get } from 'svelte/store';
import { tauriInvoke as invoke } from '$lib/tauri';

/**
 * The backend's max-concurrent-workflows cap, or `null` until loaded (or if the
 * one-shot fetch failed). Consumers must tolerate `null` (render a placeholder
 * rather than a hardcoded fallback — the backend constant is the single source).
 */
const store = writable<number | null>(null);

/**
 * Memoized in-flight load promise. Guards against concurrent `load()` calls
 * (e.g. the root layout and the Kanban page both triggering it) issuing
 * duplicate IPC fetches.
 */
let loadPromise: Promise<void> | null = null;

/**
 * Runtime config store for the Kanban board.
 */
export const kanbanRuntimeStore = {
	subscribe: store.subscribe,

	/**
	 * Fetch the backend's concurrency cap once and cache it. Safe to call
	 * multiple times: a successful load short-circuits, and concurrent calls
	 * share the same in-flight promise. Resolves silently on failure (the value
	 * stays `null`); callers should not block on it.
	 */
	async load(): Promise<void> {
		if (get(store) !== null) {
			return;
		}
		if (loadPromise) {
			return loadPromise;
		}
		loadPromise = (async () => {
			const max = await invoke<number>('get_max_concurrent_workflows');
			store.set(max);
		})();
		try {
			await loadPromise;
		} catch (e) {
			// Reset so a later call can retry; leave the value null (no hardcoded
			// fallback — the badge degrades to a placeholder until a retry lands).
			loadPromise = null;
			throw e;
		}
	},

	/**
	 * Synchronously read the current cap (`null` if not yet loaded).
	 */
	get(): number | null {
		return get(store);
	}
};

/** Derived store for the cap value (`number | null`). */
export const maxConcurrentWorkflows = derived(store, ($v) => $v);
