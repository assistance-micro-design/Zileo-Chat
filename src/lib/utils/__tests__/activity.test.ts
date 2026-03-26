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
import { formatTokenCount } from '../activity';

describe('formatTokenCount', () => {
	it('returns raw number for values below 1000', () => {
		expect(formatTokenCount(0)).toBe('0');
		expect(formatTokenCount(1)).toBe('1');
		expect(formatTokenCount(999)).toBe('999');
	});

	it('formats values at 1000 boundary with k suffix', () => {
		expect(formatTokenCount(1000)).toBe('1.0k');
	});

	it('formats values above 1000 with one decimal', () => {
		expect(formatTokenCount(1500)).toBe('1.5k');
		expect(formatTokenCount(2750)).toBe('2.8k');
		expect(formatTokenCount(10000)).toBe('10.0k');
		expect(formatTokenCount(123456)).toBe('123.5k');
	});
});
