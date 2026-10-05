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
 * @fileoverview Internal transport for the typed API clients.
 *
 * `undefined` argument values are stripped so optional parameters behave
 * exactly like omitted keys on the wire (`None` in Rust), while explicit
 * `null` values are preserved. This keeps IPC payloads byte-identical to
 * the previous direct-`tauriInvoke` call sites.
 *
 * @module lib/api/internal
 */

import { tauriInvoke } from '$lib/tauri';

/** Invoke a Tauri command, dropping `undefined` argument values. */
export async function invokeCommand<T>(
	command: string,
	args?: Record<string, unknown>
): Promise<T> {
	if (args === undefined) {
		return tauriInvoke<T>(command);
	}
	const clean: Record<string, unknown> = {};
	for (const [key, value] of Object.entries(args)) {
		if (value !== undefined) {
			clean[key] = value;
		}
	}
	return tauriInvoke<T>(command, clean);
}
