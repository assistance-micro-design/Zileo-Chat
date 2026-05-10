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
	if (error !== null && typeof error === 'object') {
		try {
			return JSON.stringify(error);
		} catch {
			return String(error);
		}
	}
	return String(error);
}
