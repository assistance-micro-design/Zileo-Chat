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
 * @fileoverview Runtime helpers for frontend Tauri adapters.
 *
 * These helpers keep browser/Tauri checks centralized so Svelte modules,
 * stores, services, and tests do not need to reach into runtime globals
 * directly before using Tauri APIs.
 *
 * @module lib/tauri/environment
 */

interface TauriWindow extends Window {
	__TAURI_INTERNALS__?: unknown;
}

/**
 * Returns true when the current module is running in a browser-like runtime.
 */
export function isBrowserRuntime(): boolean {
	return typeof window !== 'undefined';
}

/**
 * Returns true when the current browser runtime exposes Tauri internals.
 */
export function isTauriRuntime(): boolean {
	return isBrowserRuntime() && '__TAURI_INTERNALS__' in (window as TauriWindow);
}

/**
 * Error used when a required Tauri API is called outside the Tauri runtime.
 */
export function createTauriUnavailableError(apiName: string): Error {
	return new Error(`${apiName} is only available in the Tauri runtime`);
}
