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
import { adjustMenuPosition, getNextFocusableIndex } from '../contextMenu';
import type { ContextMenuItem } from '$types/sidebar';

describe('adjustMenuPosition', () => {
	const viewport = { width: 1024, height: 768 };

	it('keeps position when menu fits within viewport', () => {
		const result = adjustMenuPosition(100, 200, 180, 250, viewport.width, viewport.height);
		expect(result).toEqual({ x: 100, y: 200 });
	});

	it('flips X when menu overflows right edge', () => {
		const result = adjustMenuPosition(900, 200, 180, 250, viewport.width, viewport.height);
		expect(result.x).toBe(720); // 900 - 180
		expect(result.y).toBe(200);
	});

	it('flips Y when menu overflows bottom edge', () => {
		const result = adjustMenuPosition(100, 600, 180, 250, viewport.width, viewport.height);
		expect(result.x).toBe(100);
		expect(result.y).toBe(350); // 600 - 250
	});

	it('flips both axes when overflowing both edges', () => {
		const result = adjustMenuPosition(900, 600, 180, 250, viewport.width, viewport.height);
		expect(result.x).toBe(720);
		expect(result.y).toBe(350);
	});

	it('clamps to 0 when flipped position would be negative', () => {
		const result = adjustMenuPosition(50, 50, 200, 200, viewport.width, viewport.height);
		// 50 + 200 = 250 < 1024, so no flip on X
		// 50 + 200 = 250 < 768, so no flip on Y
		expect(result).toEqual({ x: 50, y: 50 });

		// Force overflow with tiny viewport
		const narrow = adjustMenuPosition(100, 100, 200, 200, 150, 150);
		// 100 + 200 > 150, flipped: max(0, 100 - 200) = 0
		expect(narrow.x).toBe(0);
		expect(narrow.y).toBe(0);
	});

	it('handles exact edge placement', () => {
		// Menu exactly fits
		const result = adjustMenuPosition(844, 518, 180, 250, viewport.width, viewport.height);
		// 844 + 180 = 1024 = viewport.width, so no flip
		expect(result.x).toBe(844);
		expect(result.y).toBe(518);
	});
});

describe('getNextFocusableIndex', () => {
	const items: ContextMenuItem[] = [
		{ id: 'a', labelKey: 'a' },
		{ id: 'b', labelKey: 'b', disabled: true },
		{ id: 'c', labelKey: 'c' },
		{ id: 'd', labelKey: 'd' }
	];

	it('moves down to next enabled item', () => {
		expect(getNextFocusableIndex(items, 0, 1)).toBe(2); // skips disabled b
	});

	it('moves up to previous enabled item', () => {
		expect(getNextFocusableIndex(items, 2, -1)).toBe(0); // skips disabled b
	});

	it('wraps around when moving down past end', () => {
		expect(getNextFocusableIndex(items, 3, 1)).toBe(0);
	});

	it('wraps around when moving up past start', () => {
		expect(getNextFocusableIndex(items, 0, -1)).toBe(3);
	});

	it('returns -1 for empty items', () => {
		expect(getNextFocusableIndex([], 0, 1)).toBe(-1);
	});

	it('handles all items disabled', () => {
		const allDisabled: ContextMenuItem[] = [
			{ id: 'a', labelKey: 'a', disabled: true },
			{ id: 'b', labelKey: 'b', disabled: true }
		];
		// Should not infinite loop, returns some index
		const result = getNextFocusableIndex(allDisabled, 0, 1);
		expect(result).toBeGreaterThanOrEqual(0);
		expect(result).toBeLessThan(allDisabled.length);
	});

	it('moves from -1 (no selection) to first enabled', () => {
		// -1 + 1 = 0, which is enabled
		expect(getNextFocusableIndex(items, -1, 1)).toBe(0);
	});

	it('skips multiple consecutive disabled items', () => {
		const manyDisabled: ContextMenuItem[] = [
			{ id: 'a', labelKey: 'a' },
			{ id: 'b', labelKey: 'b', disabled: true },
			{ id: 'c', labelKey: 'c', disabled: true },
			{ id: 'd', labelKey: 'd', disabled: true },
			{ id: 'e', labelKey: 'e' }
		];
		expect(getNextFocusableIndex(manyDisabled, 0, 1)).toBe(4);
	});
});
