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
 * @fileoverview Safe frontend wrapper around Tauri event listeners.
 *
 * @module lib/tauri/events
 */

import { isTauriRuntime } from './environment';

export type TauriUnlistenFn = () => void;
export type TauriEvent<T> = {
	event: string;
	id: number;
	payload: T;
};

/**
 * Registers a Tauri event listener when Tauri is available.
 *
 * Outside Tauri, this returns a no-op unlistener so callers can keep teardown
 * logic simple in tests, preview, and browser-only environments.
 *
 * @param event - Tauri event name
 * @param handler - Event payload handler
 * @returns Unlisten function
 */
export async function tauriListen<T>(
	event: string,
	handler: (event: TauriEvent<T>) => void
): Promise<TauriUnlistenFn> {
	if (!isTauriRuntime()) {
		return () => {};
	}

	const { listen } = await import('@tauri-apps/api/event');
	return listen<T>(event, handler);
}
