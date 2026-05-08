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
 * @fileoverview Helper for subscribing to the `settings:refresh` custom event
 * dispatched after import/export operations. Handles attachment/teardown via
 * Svelte's onMount lifecycle so each settings page can share a one-liner wiring.
 *
 * @module lib/utils/settings-refresh
 */

import { onMount } from 'svelte';

/** Name of the custom event dispatched after import operations complete. */
export const SETTINGS_REFRESH_EVENT = 'settings:refresh';

/**
 * Broadcasts a `settings:refresh` event so sibling Settings surfaces
 * (Agents form, MCP, LLM, Validation, etc.) pick up CRUD changes without
 * waiting for a remount. No-op when `window` is unavailable (SSR).
 */
export function dispatchSettingsRefresh(): void {
	if (typeof window === 'undefined') {
		return;
	}
	window.dispatchEvent(new CustomEvent(SETTINGS_REFRESH_EVENT));
}

/**
 * Attaches a listener for `settings:refresh` events and returns the teardown.
 * Exported for unit testing; components should use {@link onSettingsRefresh}.
 *
 * @param handler - Callback invoked for each refresh event
 * @returns Teardown function that removes the listener
 */
export function attachSettingsRefreshListener(
	handler: () => void | Promise<void>
): () => void {
	if (typeof window === 'undefined') {
		return () => {};
	}

	const listener = (): void => {
		void handler();
	};
	window.addEventListener(SETTINGS_REFRESH_EVENT, listener);
	return () => {
		window.removeEventListener(SETTINGS_REFRESH_EVENT, listener);
	};
}

/**
 * Registers a handler for the `settings:refresh` custom event, wiring it to
 * the component lifecycle. Adds the listener on mount and removes it on
 * destroy automatically.
 *
 * Must be called during component initialization (not inside onMount itself).
 *
 * @param handler - Callback invoked each time a settings refresh is requested
 */
export function onSettingsRefresh(handler: () => void | Promise<void>): void {
	onMount(() => attachSettingsRefreshListener(handler));
}
