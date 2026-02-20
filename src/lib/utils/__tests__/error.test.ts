// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { describe, it, expect } from 'vitest';
import { getErrorMessage, formatErrorForDisplay } from '../error';

describe('getErrorMessage', () => {
	it('handles Error instances', () => {
		expect(getErrorMessage(new Error('test'))).toBe('test');
	});

	it('handles string errors', () => {
		expect(getErrorMessage('raw string')).toBe('raw string');
	});

	it('handles unknown errors with message property', () => {
		expect(getErrorMessage({ message: 'from object' })).toBe('from object');
	});

	it('handles unknown errors without message property', () => {
		expect(getErrorMessage({ code: 42 })).toMatch(/42/);
	});

	it('handles null', () => {
		expect(getErrorMessage(null)).toBeTruthy();
	});

	it('handles undefined', () => {
		expect(getErrorMessage(undefined)).toBeTruthy();
	});

	it('handles number errors', () => {
		expect(getErrorMessage(404)).toBe('404');
	});
});

describe('formatErrorForDisplay', () => {
	it('strips Error: prefix', () => {
		expect(formatErrorForDisplay('Error: something went wrong')).toBe('something went wrong');
	});

	it('strips invoke error: prefix', () => {
		expect(formatErrorForDisplay('invoke error: bad request')).toBe('bad request');
	});

	it('passes through clean messages', () => {
		expect(formatErrorForDisplay('something went wrong')).toBe('something went wrong');
	});

	it('handles Error objects', () => {
		expect(formatErrorForDisplay(new Error('test error'))).toBe('test error');
	});
});
