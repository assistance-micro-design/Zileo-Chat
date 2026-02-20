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
import { isAllowedScheme } from '../url';

describe('isAllowedScheme', () => {
	it('allows https URLs', () => {
		expect(isAllowedScheme('https://example.com')).toBe(true);
	});

	it('allows http URLs', () => {
		expect(isAllowedScheme('http://example.com')).toBe(true);
	});

	it('allows mailto URLs', () => {
		expect(isAllowedScheme('mailto:user@example.com')).toBe(true);
	});

	it('blocks javascript: URLs', () => {
		expect(isAllowedScheme('javascript:alert(1)')).toBe(false);
	});

	it('blocks javascript: with mixed case', () => {
		expect(isAllowedScheme('JavaScript:alert(1)')).toBe(false);
	});

	it('blocks data: URLs', () => {
		expect(isAllowedScheme('data:text/html,<script>alert(1)</script>')).toBe(false);
	});

	it('blocks vbscript: URLs', () => {
		expect(isAllowedScheme('vbscript:MsgBox("xss")')).toBe(false);
	});

	it('blocks empty string', () => {
		expect(isAllowedScheme('')).toBe(false);
	});

	it('allows relative URLs', () => {
		expect(isAllowedScheme('/path/to/page')).toBe(true);
	});

	it('allows fragment-only URLs', () => {
		expect(isAllowedScheme('#section')).toBe(true);
	});
});
