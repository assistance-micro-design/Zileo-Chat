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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Debounce utility function
 * Delays function execution until after a specified wait time has elapsed
 * since the last invocation.
 *
 * @param fn - Function to debounce
 * @param delay - Delay in milliseconds
 * @returns Debounced function
 *
 * @example
 * const debouncedSearch = debounce((query: string) => search(query), 300);
 */
export function debounce<T extends (...args: Parameters<T>) => void>(
	fn: T,
	delay: number
): (...args: Parameters<T>) => void {
	let timeoutId: ReturnType<typeof setTimeout> | null = null;

	return function debounced(...args: Parameters<T>): void {
		if (timeoutId !== null) {
			clearTimeout(timeoutId);
		}

		timeoutId = setTimeout(() => {
			fn(...args);
			timeoutId = null;
		}, delay);
	};
}

/**
 * Throttle utility function
 * Ensures function is called at most once per specified interval.
 *
 * @param fn - Function to throttle
 * @param interval - Minimum time between calls in milliseconds
 * @returns Throttled function
 *
 * @example
 * const throttledScroll = throttle(() => handleScroll(), 100);
 */
export function throttle<T extends (...args: Parameters<T>) => void>(
	fn: T,
	interval: number
): (...args: Parameters<T>) => void {
	let lastTime = 0;
	let timeoutId: ReturnType<typeof setTimeout> | null = null;

	return function throttled(...args: Parameters<T>): void {
		const now = Date.now();
		const remaining = interval - (now - lastTime);

		if (remaining <= 0) {
			if (timeoutId !== null) {
				clearTimeout(timeoutId);
				timeoutId = null;
			}
			lastTime = now;
			fn(...args);
		} else if (timeoutId === null) {
			timeoutId = setTimeout(() => {
				lastTime = Date.now();
				timeoutId = null;
				fn(...args);
			}, remaining);
		}
	};
}
