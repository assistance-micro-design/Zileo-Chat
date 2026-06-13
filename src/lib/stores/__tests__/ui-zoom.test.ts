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

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';

vi.mock('$lib/tauri', () => ({
	setTauriWebviewZoom: vi.fn().mockResolvedValue(undefined)
}));

import { setTauriWebviewZoom } from '$lib/tauri';
import {
	uiZoom,
	nextZoom,
	clampZoom,
	zoomActionForKey,
	formatZoomPercent,
	MIN_ZOOM,
	MAX_ZOOM,
	DEFAULT_ZOOM
} from '../ui-zoom';

describe('ui-zoom pure helpers', () => {
	describe('clampZoom', () => {
		it('clamps below the minimum', () => {
			expect(clampZoom(0.1)).toBe(MIN_ZOOM);
		});

		it('clamps above the maximum', () => {
			expect(clampZoom(5)).toBe(MAX_ZOOM);
		});

		it('falls back to the default for non-finite input', () => {
			expect(clampZoom(Number.NaN)).toBe(DEFAULT_ZOOM);
			expect(clampZoom(Number.POSITIVE_INFINITY)).toBe(DEFAULT_ZOOM);
		});

		it('rounds floating-point drift to two decimals', () => {
			expect(clampZoom(1.1 + 0.1 + 0.1)).toBe(1.3);
		});
	});

	describe('nextZoom', () => {
		it('increases by one step', () => {
			expect(nextZoom(1, 'in')).toBe(1.1);
		});

		it('decreases by one step', () => {
			expect(nextZoom(1, 'out')).toBe(0.9);
		});

		it('does not exceed the maximum', () => {
			expect(nextZoom(MAX_ZOOM, 'in')).toBe(MAX_ZOOM);
		});

		it('does not drop below the minimum', () => {
			expect(nextZoom(MIN_ZOOM, 'out')).toBe(MIN_ZOOM);
		});

		it('resets to the default', () => {
			expect(nextZoom(1.7, 'reset')).toBe(DEFAULT_ZOOM);
		});

		it('avoids floating-point accumulation across successive steps', () => {
			let z = DEFAULT_ZOOM;
			for (let i = 0; i < 3; i++) {
				z = nextZoom(z, 'in');
			}
			expect(z).toBe(1.3);
		});
	});

	describe('formatZoomPercent', () => {
		it('formats the default factor as 100 %', () => {
			expect(formatZoomPercent(DEFAULT_ZOOM)).toBe('100 %');
		});

		it('formats fractional factors as whole percentages', () => {
			expect(formatZoomPercent(1.3)).toBe('130 %');
			expect(formatZoomPercent(0.5)).toBe('50 %');
			expect(formatZoomPercent(MAX_ZOOM)).toBe('200 %');
		});

		it('rounds floating-point drift to a whole percentage', () => {
			expect(formatZoomPercent(1.15)).toBe('115 %');
		});
	});

	describe('zoomActionForKey', () => {
		it('maps Ctrl + "+" / "=" to zoom in', () => {
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: '+', code: 'Equal' })).toBe(
				'in'
			);
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: '=', code: 'Equal' })).toBe(
				'in'
			);
		});

		it('maps Ctrl + "-" / "_" to zoom out', () => {
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: '-', code: 'Minus' })).toBe(
				'out'
			);
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: '_', code: 'Minus' })).toBe(
				'out'
			);
		});

		it('maps Ctrl + "0" to reset', () => {
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: '0', code: 'Digit0' })).toBe(
				'reset'
			);
		});

		it('matches by physical key code regardless of keyboard layout', () => {
			// On an AZERTY layout the digit/symbol keys produce other characters,
			// but the physical position (code) is stable.
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: 'à', code: 'Digit0' })).toBe(
				'reset'
			);
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: '+', code: 'NumpadAdd' })).toBe(
				'in'
			);
			expect(
				zoomActionForKey({ ctrlKey: true, metaKey: false, key: '-', code: 'NumpadSubtract' })
			).toBe('out');
		});

		it('accepts the meta key (macOS)', () => {
			expect(zoomActionForKey({ ctrlKey: false, metaKey: true, key: '+', code: 'Equal' })).toBe(
				'in'
			);
		});

		it('ignores zoom keys pressed without a modifier', () => {
			expect(zoomActionForKey({ ctrlKey: false, metaKey: false, key: '+', code: 'Equal' })).toBe(
				null
			);
		});

		it('ignores unrelated keys', () => {
			expect(zoomActionForKey({ ctrlKey: true, metaKey: false, key: 'a', code: 'KeyA' })).toBe(
				null
			);
		});
	});
});

describe('uiZoom store', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		uiZoom.setZoom(DEFAULT_ZOOM);
		window.localStorage.clear();
		vi.clearAllMocks();
	});

	it('setZoom clamps, persists and syncs the native webview', () => {
		uiZoom.setZoom(1.4);

		expect(get(uiZoom)).toBe(1.4);
		expect(window.localStorage.getItem('ui-zoom')).toBe('1.4');
		expect(setTauriWebviewZoom).toHaveBeenCalledWith(1.4);
	});

	it('setZoom clamps out-of-range values', () => {
		uiZoom.setZoom(99);
		expect(get(uiZoom)).toBe(MAX_ZOOM);
	});

	it('step applies the action relative to the current zoom', () => {
		uiZoom.setZoom(1);
		uiZoom.step('in');
		expect(get(uiZoom)).toBe(1.1);

		uiZoom.step('reset');
		expect(get(uiZoom)).toBe(DEFAULT_ZOOM);
	});

	it('increase / decrease / reset are convenience wrappers', () => {
		uiZoom.setZoom(1);
		uiZoom.increase();
		expect(get(uiZoom)).toBe(1.1);

		uiZoom.decrease();
		expect(get(uiZoom)).toBe(1);

		uiZoom.setZoom(1.5);
		uiZoom.reset();
		expect(get(uiZoom)).toBe(DEFAULT_ZOOM);
	});

	it('init restores a saved zoom and re-applies it to the webview', () => {
		window.localStorage.setItem('ui-zoom', '1.3');

		uiZoom.init();

		expect(get(uiZoom)).toBe(1.3);
		expect(setTauriWebviewZoom).toHaveBeenCalledWith(1.3);
	});

	it('init falls back to the default for a missing or invalid saved value', () => {
		window.localStorage.setItem('ui-zoom', 'not-a-number');

		uiZoom.init();

		expect(get(uiZoom)).toBe(DEFAULT_ZOOM);
	});

	it('remains usable when localStorage throws', () => {
		vi.spyOn(window.localStorage.__proto__, 'setItem').mockImplementation(() => {
			throw new Error('storage unavailable');
		});

		expect(() => uiZoom.setZoom(1.2)).not.toThrow();
		expect(get(uiZoom)).toBe(1.2);
	});
});
