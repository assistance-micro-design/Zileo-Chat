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

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { debounce, throttle } from '../debounce';

describe('debounce', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('should delay function execution', () => {
		const fn = vi.fn();
		const debouncedFn = debounce(fn, 100);

		debouncedFn();
		expect(fn).not.toHaveBeenCalled();

		vi.advanceTimersByTime(50);
		expect(fn).not.toHaveBeenCalled();

		vi.advanceTimersByTime(50);
		expect(fn).toHaveBeenCalledTimes(1);
	});

	it('should reset delay on subsequent calls', () => {
		const fn = vi.fn();
		const debouncedFn = debounce(fn, 100);

		debouncedFn();
		vi.advanceTimersByTime(50);

		debouncedFn();
		vi.advanceTimersByTime(50);

		expect(fn).not.toHaveBeenCalled();

		vi.advanceTimersByTime(50);
		expect(fn).toHaveBeenCalledTimes(1);
	});

	it('should pass arguments to the debounced function', () => {
		const fn = vi.fn();
		const debouncedFn = debounce(fn, 100);

		debouncedFn('arg1', 'arg2');
		vi.advanceTimersByTime(100);

		expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
	});

	it('should use the latest arguments when called multiple times', () => {
		const fn = vi.fn();
		const debouncedFn = debounce(fn, 100);

		debouncedFn('first');
		debouncedFn('second');
		debouncedFn('third');
		vi.advanceTimersByTime(100);

		expect(fn).toHaveBeenCalledTimes(1);
		expect(fn).toHaveBeenCalledWith('third');
	});
});

describe('throttle', () => {
	beforeEach(() => {
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('should execute immediately on first call', () => {
		const fn = vi.fn();
		const throttledFn = throttle(fn, 100);

		throttledFn();
		expect(fn).toHaveBeenCalledTimes(1);
	});

	it('should not execute again within interval', () => {
		const fn = vi.fn();
		const throttledFn = throttle(fn, 100);

		throttledFn();
		throttledFn();
		throttledFn();

		expect(fn).toHaveBeenCalledTimes(1);
	});

	it('should execute after interval has passed', () => {
		const fn = vi.fn();
		const throttledFn = throttle(fn, 100);

		throttledFn();
		vi.advanceTimersByTime(100);
		throttledFn();

		expect(fn).toHaveBeenCalledTimes(2);
	});

	it('should pass arguments to the throttled function', () => {
		const fn = vi.fn();
		const throttledFn = throttle(fn, 100);

		throttledFn('arg1', 'arg2');

		expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
	});

	it('should schedule trailing call when called during interval', () => {
		const fn = vi.fn();
		const throttledFn = throttle(fn, 100);

		throttledFn('first');
		expect(fn).toHaveBeenCalledTimes(1);
		expect(fn).toHaveBeenCalledWith('first');

		// Call during interval - will schedule trailing
		throttledFn('second');

		// Advance past interval - trailing should execute
		vi.advanceTimersByTime(100);

		expect(fn).toHaveBeenCalledTimes(2);
		expect(fn).toHaveBeenLastCalledWith('second');
	});
});
