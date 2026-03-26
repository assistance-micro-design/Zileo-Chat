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
 * Async Handler Utilities
 *
 * @module utils/async
 *
 * @example
 * ```typescript
 * const handleRename = withToastError(async (id: string, name: string) => {
 *   await WorkflowService.rename(id, name);
 *   await workflowStore.loadWorkflows();
 * });
 * ```
 */

import { toastStore } from '$lib/stores/toast';
import { getErrorMessage } from '$lib/utils/error';

/**
 * Wraps an async function to catch errors and display them as toast notifications.
 *
 * Eliminates the repetitive try/catch + toastStore.add pattern. The wrapped function
 * swallows errors (they are shown to the user via toast instead of propagating).
 *
 * @param fn - The async function to wrap
 * @returns A function with the same parameters that catches errors and shows a toast
 *
 * @example
 * ```typescript
 * // Before: repeated in 7+ handlers
 * async function handleRename(id: string, name: string) {
 *   try {
 *     await WorkflowService.rename(id, name);
 *   } catch (err) {
 *     toastStore.add({ type: 'error', title: getErrorMessage(err), message: '', persistent: false, duration: 5000 });
 *   }
 * }
 *
 * // After: clean one-liner wrap
 * const handleRename = withToastError(async (id: string, name: string) => {
 *   await WorkflowService.rename(id, name);
 * });
 * ```
 */
export function withToastError<Args extends unknown[]>(
	fn: (...args: Args) => Promise<void>
): (...args: Args) => Promise<void> {
	return async (...args: Args): Promise<void> => {
		try {
			await fn(...args);
		} catch (err: unknown) {
			toastStore.add({
				type: 'error',
				title: getErrorMessage(err),
				message: '',
				persistent: false,
				duration: 5000
			});
		}
	};
}
