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

import { describe, it, expect, vi } from 'vitest';
import {
	attachSettingsRefreshListener,
	dispatchSettingsRefresh,
	SETTINGS_REFRESH_EVENT
} from '../settings-refresh';

describe('attachSettingsRefreshListener', () => {
	it('invokes the handler when the settings:refresh event fires', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler);

		window.dispatchEvent(new CustomEvent(SETTINGS_REFRESH_EVENT));

		expect(handler).toHaveBeenCalledTimes(1);
		teardown();
	});

	it('removes the listener via the teardown function', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler);

		teardown();
		window.dispatchEvent(new CustomEvent(SETTINGS_REFRESH_EVENT));

		expect(handler).not.toHaveBeenCalled();
	});

	it('supports async handlers without awaiting their completion', () => {
		const handler = vi.fn(async () => undefined);
		const teardown = attachSettingsRefreshListener(handler);

		window.dispatchEvent(new CustomEvent(SETTINGS_REFRESH_EVENT));
		window.dispatchEvent(new CustomEvent(SETTINGS_REFRESH_EVENT));

		expect(handler).toHaveBeenCalledTimes(2);
		teardown();
	});

	it('ignores events unrelated to settings refresh', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler);

		window.dispatchEvent(new CustomEvent('other:event'));

		expect(handler).not.toHaveBeenCalled();
		teardown();
	});

	it('returns a no-op teardown when window is unavailable', () => {
		const originalWindow = globalThis.window;
		Reflect.deleteProperty(globalThis, 'window');
		const handler = vi.fn();

		try {
			const teardown = attachSettingsRefreshListener(handler);
			expect(teardown).toEqual(expect.any(Function));
			expect(() => teardown()).not.toThrow();
			expect(handler).not.toHaveBeenCalled();
		} finally {
			Object.defineProperty(globalThis, 'window', {
				value: originalWindow,
				configurable: true,
				writable: true
			});
		}
	});
});

describe('dispatchSettingsRefresh', () => {
	it('dispatches a settings:refresh event observable by listeners', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler);

		dispatchSettingsRefresh();

		expect(handler).toHaveBeenCalledTimes(1);
		teardown();
	});

	it('is a no-op when window is unavailable', () => {
		const originalWindow = globalThis.window;
		Reflect.deleteProperty(globalThis, 'window');

		try {
			expect(() => dispatchSettingsRefresh()).not.toThrow();
		} finally {
			Object.defineProperty(globalThis, 'window', {
				value: originalWindow,
				configurable: true,
				writable: true
			});
		}
	});
});
