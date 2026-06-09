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
 * @fileoverview Attachment that flags a scrollable container with the
 * `is-scrolling` class while the user scrolls. The global stylesheet pauses
 * decorative animations (spinners, running-status pulses) under that class,
 * and per-view styles can disable pointer events to avoid expensive hover
 * recalculations in WebKitGTK during momentum scrolling.
 *
 * @module lib/actions/pauseOnScroll
 */

import type { Attachment } from 'svelte/attachments';

/**
 * Default idle delay before the `is-scrolling` flag is cleared. Extended
 * beyond a single frame to cover momentum scrolling in WebKitGTK, which keeps
 * emitting scroll events after the user releases the gesture.
 */
export const SCROLL_IDLE_DELAY_MS = 250;

/**
 * Builds an attachment that toggles the `is-scrolling` class on its element
 * during scroll activity.
 *
 * @param idleDelayMs - Quiet period (ms) after the last scroll event before
 * the class is removed. Defaults to {@link SCROLL_IDLE_DELAY_MS}.
 * @returns Attachment managing the scroll listener; on cleanup it removes the
 * listener, cancels the pending timer and clears the class.
 *
 * @example
 * ```svelte
 * <main {@attach pauseOnScroll()}>...</main>
 * ```
 */
export function pauseOnScroll(idleDelayMs: number = SCROLL_IDLE_DELAY_MS): Attachment<HTMLElement> {
	return (element) => {
		let idleTimer: ReturnType<typeof setTimeout> | undefined;

		function handleScroll(): void {
			element.classList.add('is-scrolling');
			clearTimeout(idleTimer);
			idleTimer = setTimeout(() => {
				element.classList.remove('is-scrolling');
			}, idleDelayMs);
		}

		element.addEventListener('scroll', handleScroll, { passive: true });

		return () => {
			element.removeEventListener('scroll', handleScroll);
			clearTimeout(idleTimer);
			element.classList.remove('is-scrolling');
		};
	};
}
