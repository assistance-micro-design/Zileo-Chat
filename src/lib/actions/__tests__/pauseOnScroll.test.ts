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

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { pauseOnScroll, SCROLL_IDLE_DELAY_MS } from '../pauseOnScroll';

describe('pauseOnScroll', () => {
	let element: HTMLElement;
	let cleanup: void | (() => void);

	beforeEach(() => {
		vi.useFakeTimers();
		element = document.createElement('div');
		document.body.appendChild(element);
	});

	afterEach(() => {
		if (typeof cleanup === 'function') cleanup();
		element.remove();
		vi.useRealTimers();
	});

	it('adds the is-scrolling class on scroll', () => {
		cleanup = pauseOnScroll()(element);

		element.dispatchEvent(new Event('scroll'));

		expect(element.classList.contains('is-scrolling')).toBe(true);
	});

	it('removes the class after the idle delay', () => {
		cleanup = pauseOnScroll()(element);

		element.dispatchEvent(new Event('scroll'));
		vi.advanceTimersByTime(SCROLL_IDLE_DELAY_MS);

		expect(element.classList.contains('is-scrolling')).toBe(false);
	});

	it('keeps the class while scroll events keep arriving (debounce reset)', () => {
		cleanup = pauseOnScroll()(element);

		element.dispatchEvent(new Event('scroll'));
		vi.advanceTimersByTime(SCROLL_IDLE_DELAY_MS - 50);
		element.dispatchEvent(new Event('scroll'));
		vi.advanceTimersByTime(SCROLL_IDLE_DELAY_MS - 50);

		expect(element.classList.contains('is-scrolling')).toBe(true);

		vi.advanceTimersByTime(50);
		expect(element.classList.contains('is-scrolling')).toBe(false);
	});

	it('honors a custom idle delay', () => {
		cleanup = pauseOnScroll(500)(element);

		element.dispatchEvent(new Event('scroll'));
		vi.advanceTimersByTime(SCROLL_IDLE_DELAY_MS);
		expect(element.classList.contains('is-scrolling')).toBe(true);

		vi.advanceTimersByTime(500 - SCROLL_IDLE_DELAY_MS);
		expect(element.classList.contains('is-scrolling')).toBe(false);
	});

	it('removes the class, the listener and the pending timer on cleanup', () => {
		const detach = pauseOnScroll()(element);

		element.dispatchEvent(new Event('scroll'));
		expect(element.classList.contains('is-scrolling')).toBe(true);

		if (typeof detach === 'function') detach();
		cleanup = undefined;

		expect(element.classList.contains('is-scrolling')).toBe(false);

		element.dispatchEvent(new Event('scroll'));
		expect(element.classList.contains('is-scrolling')).toBe(false);

		expect(vi.getTimerCount()).toBe(0);
	});
});
