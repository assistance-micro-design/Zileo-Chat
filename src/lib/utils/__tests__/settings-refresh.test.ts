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

	it('carries the source tag in the event detail', () => {
		// Captures the raw event so we can assert the detail shape; the
		// helper API only exposes the unwrapped detail to handlers, which
		// would hide a missing/empty detail bug at the dispatch site.
		const captured: CustomEvent[] = [];
		const rawListener = (event: Event): void => {
			captured.push(event as CustomEvent);
		};
		window.addEventListener(SETTINGS_REFRESH_EVENT, rawListener);

		try {
			dispatchSettingsRefresh({ source: 'agents' });
			expect(captured).toHaveLength(1);
			expect(captured[0]?.detail).toEqual({ source: 'agents' });
		} finally {
			window.removeEventListener(SETTINGS_REFRESH_EVENT, rawListener);
		}
	});
});

describe('source-filtering', () => {
	it('skips the handler when ignoreSource matches the dispatch source', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler, { ignoreSource: 'agents' });

		dispatchSettingsRefresh({ source: 'agents' });

		expect(handler).not.toHaveBeenCalled();
		teardown();
	});

	it('invokes the handler when the dispatch source differs from ignoreSource', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler, { ignoreSource: 'agents' });

		dispatchSettingsRefresh({ source: 'providers' });

		expect(handler).toHaveBeenCalledTimes(1);
		// Detail is forwarded to the handler so a consumer can branch on
		// the source without re-reading the raw event.
		expect(handler.mock.calls[0]?.[0]).toEqual({ source: 'providers' });
		teardown();
	});

	it('invokes the handler when the dispatch carries no source', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler, { ignoreSource: 'agents' });

		dispatchSettingsRefresh();

		expect(handler).toHaveBeenCalledTimes(1);
		teardown();
	});

	it('forwards every event when no options are provided (legitimate cross-page listener)', () => {
		const handler = vi.fn();
		const teardown = attachSettingsRefreshListener(handler);

		dispatchSettingsRefresh({ source: 'agents' });
		dispatchSettingsRefresh({ source: 'providers' });
		dispatchSettingsRefresh();

		expect(handler).toHaveBeenCalledTimes(3);
		teardown();
	});
});
