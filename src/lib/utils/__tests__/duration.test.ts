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
import { formatDuration } from '../duration';

describe('formatDuration', () => {
	it('returns dash for undefined', () => {
		expect(formatDuration(undefined)).toBe('-');
	});

	it('returns dash for null', () => {
		expect(formatDuration(null)).toBe('-');
	});

	it('formats sub-second values in milliseconds', () => {
		expect(formatDuration(0)).toBe('0ms');
		expect(formatDuration(500)).toBe('500ms');
		expect(formatDuration(999)).toBe('999ms');
	});

	it('formats values at 1000ms boundary in seconds', () => {
		expect(formatDuration(1000)).toBe('1.0s');
	});

	it('formats second-range values with one decimal', () => {
		expect(formatDuration(1500)).toBe('1.5s');
		expect(formatDuration(30000)).toBe('30.0s');
		expect(formatDuration(59999)).toBe('60.0s');
	});

	it('formats minute-range values as Xm Ys', () => {
		expect(formatDuration(60000)).toBe('1m 0s');
		expect(formatDuration(90000)).toBe('1m 30s');
		expect(formatDuration(125000)).toBe('2m 5s');
	});
});
