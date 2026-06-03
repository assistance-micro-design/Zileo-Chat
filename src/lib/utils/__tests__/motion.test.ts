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

import { afterEach, describe, expect, it, vi } from 'vitest';
import { motionDuration, prefersReducedMotion } from '../motion';

describe('prefersReducedMotion', () => {
	const originalMatchMedia = window.matchMedia;

	afterEach(() => {
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: originalMatchMedia
		});
		vi.restoreAllMocks();
	});

	function mockMatchMedia(matches: boolean): void {
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: vi.fn().mockImplementation((query: string) => ({
				matches,
				media: query,
				addEventListener: vi.fn(),
				removeEventListener: vi.fn()
			}))
		});
	}

	it('returns true when the user requests reduced motion', () => {
		mockMatchMedia(true);
		expect(prefersReducedMotion()).toBe(true);
	});

	it('returns false when the user does not request reduced motion', () => {
		mockMatchMedia(false);
		expect(prefersReducedMotion()).toBe(false);
	});

	it('returns false when matchMedia is unavailable', () => {
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: undefined
		});
		expect(prefersReducedMotion()).toBe(false);
	});
});

describe('motionDuration', () => {
	const originalMatchMedia = window.matchMedia;

	afterEach(() => {
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: originalMatchMedia
		});
	});

	function mockMatchMedia(matches: boolean): void {
		Object.defineProperty(window, 'matchMedia', {
			configurable: true,
			value: vi.fn().mockImplementation((query: string) => ({
				matches,
				media: query,
				addEventListener: vi.fn(),
				removeEventListener: vi.fn()
			}))
		});
	}

	it('returns the requested duration when motion is allowed', () => {
		mockMatchMedia(false);
		expect(motionDuration(250)).toBe(250);
	});

	it('collapses the duration to zero when reduced motion is requested', () => {
		mockMatchMedia(true);
		expect(motionDuration(250)).toBe(0);
	});
});
