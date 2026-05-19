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
 * dispatched after CRUD operations on a Settings page. Carries a `source` tag
 * so a page can ignore the echo of its own dispatch (the surrounding CRUD
 * store has already refreshed) while still receiving events from sibling
 * Settings surfaces and from cross-page consumers (workflow sidebar).
 *
 * @module lib/utils/settings-refresh
 */

import { onMount } from 'svelte';

/** Name of the custom event dispatched after a settings CRUD operation. */
export const SETTINGS_REFRESH_EVENT = 'settings:refresh';

/**
 * Source identifier carried by a `settings:refresh` event so listeners can
 * filter out the echo of their own dispatch. The literal union is the
 * defensive surface: any unknown source value still propagates as a generic
 * refresh, but the matching `ignoreSource` check requires an exact string.
 */
export type SettingsRefreshSource = 'agents' | 'providers' | 'mcp' | 'validation' | 'import';

/** Detail payload attached to the CustomEvent. */
export interface SettingsRefreshDetail {
	/**
	 * Origin of the refresh request (the page or component that dispatched).
	 * Listeners with a matching `ignoreSource` skip the handler.
	 */
	source?: SettingsRefreshSource;
}

/** Options for listener registration. */
export interface OnSettingsRefreshOptions {
	/**
	 * Source value to ignore. When the dispatched event carries this exact
	 * source, the handler is not invoked. Useful for a page that dispatches
	 * its own refresh after a CRUD store update (the store already reloaded,
	 * a second reload from the listener would race with the in-flight render).
	 */
	ignoreSource?: SettingsRefreshSource;
}

/** Listener handler signature: receives the event detail (may be empty). */
export type SettingsRefreshHandler = (detail?: SettingsRefreshDetail) => void | Promise<void>;

/**
 * Broadcasts a `settings:refresh` event so sibling Settings surfaces
 * (workflow sidebar, sibling forms) pick up CRUD changes without waiting
 * for a remount. The optional `source` lets the destination page filter out
 * its own echo. No-op when `window` is unavailable (SSR).
 *
 * @param detail - Optional payload, typically the dispatcher's source tag
 */
export function dispatchSettingsRefresh(detail?: SettingsRefreshDetail): void {
	if (typeof window === 'undefined') {
		return;
	}
	window.dispatchEvent(
		new CustomEvent<SettingsRefreshDetail>(SETTINGS_REFRESH_EVENT, {
			detail: detail ?? {}
		})
	);
}

/**
 * Attaches a listener for `settings:refresh` events and returns the teardown.
 * Exported for unit testing; components should use {@link onSettingsRefresh}.
 *
 * When `opts.ignoreSource` is provided, events whose `detail.source` matches
 * that value are skipped (no handler invocation). All other events flow
 * through, including legacy dispatches with no detail.
 *
 * @param handler - Callback invoked for each refresh event (with event detail)
 * @param opts - Listener options (e.g. ignore the page's own source)
 * @returns Teardown function that removes the listener
 */
export function attachSettingsRefreshListener(
	handler: SettingsRefreshHandler,
	opts?: OnSettingsRefreshOptions
): () => void {
	if (typeof window === 'undefined') {
		return () => {};
	}

	const ignoreSource = opts?.ignoreSource;
	const listener = (event: Event): void => {
		const detail = (event as CustomEvent<SettingsRefreshDetail>).detail;
		if (ignoreSource !== undefined && detail?.source === ignoreSource) {
			return;
		}
		void handler(detail);
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
 * @param opts - Listener options (e.g. ignore the page's own source)
 */
export function onSettingsRefresh(
	handler: SettingsRefreshHandler,
	opts?: OnSettingsRefreshOptions
): void {
	onMount(() => attachSettingsRefreshListener(handler, opts));
}
