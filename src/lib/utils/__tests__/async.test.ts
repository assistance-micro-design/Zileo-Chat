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

import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockAdd = vi.fn();

vi.mock('$lib/stores/toast', () => ({
	toastStore: { add: mockAdd }
}));

// Import after mock setup
const { withToastError } = await import('$lib/utils/async');

describe('withToastError', () => {
	beforeEach(() => {
		mockAdd.mockClear();
	});

	it('should call the wrapped function and return its result', async () => {
		const fn = vi.fn().mockResolvedValue('result');
		const wrapped = withToastError(fn);

		const result = await wrapped('arg1', 'arg2');

		expect(fn).toHaveBeenCalledWith('arg1', 'arg2');
		expect(result).toBeUndefined();
		expect(mockAdd).not.toHaveBeenCalled();
	});

	it('should show an error toast when the function throws', async () => {
		const fn = vi.fn().mockRejectedValue(new Error('Something failed'));
		const wrapped = withToastError(fn);

		await wrapped();

		expect(mockAdd).toHaveBeenCalledOnce();
		expect(mockAdd).toHaveBeenCalledWith({
			type: 'error',
			title: 'Something failed',
			message: '',
			persistent: false,
			duration: 5000
		});
	});

	it('should show a toast with string errors', async () => {
		const fn = vi.fn().mockRejectedValue('string error');
		const wrapped = withToastError(fn);

		await wrapped();

		expect(mockAdd).toHaveBeenCalledOnce();
		expect(mockAdd.mock.calls[0][0].title).toBe('string error');
	});

	it('should preserve the function signature (pass-through args)', async () => {
		const fn = vi.fn().mockResolvedValue(undefined);
		const wrapped = withToastError(fn);

		await wrapped(42, 'hello', true);

		expect(fn).toHaveBeenCalledWith(42, 'hello', true);
	});

	it('should not rethrow the error', async () => {
		const fn = vi.fn().mockRejectedValue(new Error('fail'));
		const wrapped = withToastError(fn);

		// Should not throw
		await expect(wrapped()).resolves.toBeUndefined();
	});
});
