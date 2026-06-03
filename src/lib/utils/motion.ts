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
 * @fileoverview Helpers for honoring the OS-level "reduce motion" preference
 * when applying JavaScript-driven Svelte transitions. CSS media queries alone
 * cannot gate Svelte transitions (they run in JS), so these helpers read the
 * `prefers-reduced-motion` media query at transition-creation time.
 * @module utils/motion
 */

/** Media query matching the OS-level "reduce motion" accessibility setting. */
const REDUCED_MOTION_QUERY = '(prefers-reduced-motion: reduce)';

/**
 * Whether the user has requested reduced motion via their operating system.
 *
 * @returns true when the OS reports `prefers-reduced-motion: reduce`. Falls
 * back to false in environments without `matchMedia` (SSR, restricted
 * webviews) so animations are merely disabled, never crash.
 */
export function prefersReducedMotion(): boolean {
	if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
		return false;
	}
	return window.matchMedia(REDUCED_MOTION_QUERY).matches;
}

/**
 * Resolves a transition duration that respects the reduced-motion preference.
 *
 * @param duration - Desired duration in milliseconds when motion is allowed.
 * @returns The requested duration, or 0 when the user prefers reduced motion.
 */
export function motionDuration(duration: number): number {
	return prefersReducedMotion() ? 0 : duration;
}
