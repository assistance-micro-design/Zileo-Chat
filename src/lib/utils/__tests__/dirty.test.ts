// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { stableSnapshot, isDirty } from '$lib/utils/dirty';

describe('stableSnapshot', () => {
	it('produces identical snapshots for objects with different key order', () => {
		const a = { name: 'Atlas', tools: ['MemoryTool'], max: 50 };
		const b = { max: 50, name: 'Atlas', tools: ['MemoryTool'] };
		expect(stableSnapshot(a)).toBe(stableSnapshot(b));
	});

	it('sorts keys recursively in nested objects', () => {
		const a = { outer: { b: 2, a: 1 }, list: [{ y: 2, x: 1 }] };
		const b = { list: [{ x: 1, y: 2 }], outer: { a: 1, b: 2 } };
		expect(stableSnapshot(a)).toBe(stableSnapshot(b));
	});

	it('keeps array order significant', () => {
		expect(stableSnapshot(['a', 'b'])).not.toBe(stableSnapshot(['b', 'a']));
	});

	it('drops undefined members like JSON does', () => {
		expect(stableSnapshot({ a: 1, b: undefined })).toBe(stableSnapshot({ a: 1 }));
	});

	it('distinguishes null from absent', () => {
		expect(stableSnapshot({ a: null })).not.toBe(stableSnapshot({}));
	});

	it('handles primitives and empty containers', () => {
		expect(stableSnapshot('x')).toBe(stableSnapshot('x'));
		expect(stableSnapshot(0)).not.toBe(stableSnapshot(false));
		expect(stableSnapshot([])).not.toBe(stableSnapshot({}));
	});
});

describe('isDirty', () => {
	it('is false when current matches initial regardless of key order', () => {
		expect(isDirty({ a: 1, b: 'x' }, { b: 'x', a: 1 })).toBe(false);
	});

	it('is true when a scalar field changed', () => {
		expect(isDirty({ name: 'Atlas' }, { name: 'Hermes' })).toBe(true);
	});

	it('is true when a list gained an element', () => {
		expect(isDirty({ tools: ['a'] }, { tools: ['a', 'b'] })).toBe(true);
	});

	it('is true when an optional field flips between undefined and a value', () => {
		expect(isDirty({ effort: undefined }, { effort: 'high' })).toBe(true);
	});
});
