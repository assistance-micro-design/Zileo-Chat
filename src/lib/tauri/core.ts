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
 * @fileoverview Safe frontend wrapper around Tauri IPC calls.
 *
 * @module lib/tauri/core
 */

import { createTauriUnavailableError, isTauriRuntime } from './environment';

export type TauriInvokeArgs = Record<string, unknown>;

/**
 * Invokes a Tauri command from frontend code.
 *
 * The Tauri module is imported dynamically so consumers do not import
 * `@tauri-apps/api/core` directly at module evaluation time.
 *
 * @param command - Tauri command name
 * @param args - Optional command payload
 * @returns Command result
 */
export async function tauriInvoke<T>(command: string, args?: TauriInvokeArgs): Promise<T> {
	if (!isTauriRuntime()) {
		throw createTauriUnavailableError(`Tauri command "${command}"`);
	}

	const { invoke } = await import('@tauri-apps/api/core');
	return invoke<T>(command, args);
}
