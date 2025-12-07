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
 * Error handling utilities
 * @module utils/error
 */

/**
 * Extracts error message from unknown error type.
 * Standardizes error handling across all stores and components.
 *
 * @param error - Unknown error value (Error, string, or other)
 * @returns Human-readable error message string
 *
 * @example
 * ```typescript
 * try {
 *   await someAsyncOperation();
 * } catch (e) {
 *   const message = getErrorMessage(e);
 *   store.update(s => ({ ...s, error: message }));
 * }
 * ```
 */
export function getErrorMessage(error: unknown): string {
	if (error instanceof Error) {
		return error.message;
	}
	if (typeof error === 'string') {
		return error;
	}
	if (error !== null && typeof error === 'object' && 'message' in error) {
		return String((error as { message: unknown }).message);
	}
	return String(error);
}

/**
 * Type guard to check if an error is a Tauri invoke error.
 * Tauri errors typically have a specific structure with error codes.
 *
 * @param error - Unknown error value
 * @returns True if error appears to be a Tauri invoke error
 */
export function isTauriError(
	error: unknown
): error is { message: string; code?: string; data?: unknown } {
	return (
		error !== null &&
		typeof error === 'object' &&
		'message' in error &&
		typeof (error as { message: unknown }).message === 'string'
	);
}

/**
 * Formats an error for display in the UI.
 * Strips common Tauri error prefixes and normalizes the message.
 *
 * @param error - Unknown error value
 * @returns Cleaned error message suitable for UI display
 */
export function formatErrorForDisplay(error: unknown): string {
	const message = getErrorMessage(error);

	// Remove common Tauri error prefixes
	const prefixes = ['Error: ', 'invoke error: ', 'Tauri error: '];
	let cleaned = message;
	for (const prefix of prefixes) {
		if (cleaned.startsWith(prefix)) {
			cleaned = cleaned.slice(prefix.length);
		}
	}

	return cleaned;
}
