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

import { afterEach, describe, expect, it } from 'vitest';
import { createTauriUnavailableError, isBrowserRuntime, isTauriRuntime } from '../environment';

describe('Tauri environment helpers', () => {
	afterEach(() => {
		delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
	});

	it('detects the jsdom browser runtime', () => {
		expect(isBrowserRuntime()).toBe(true);
	});

	it('returns false when Tauri internals are absent', () => {
		expect(isTauriRuntime()).toBe(false);
	});

	it('returns true when Tauri internals are present', () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};

		expect(isTauriRuntime()).toBe(true);
	});

	it('creates a clear unavailable-runtime error', () => {
		expect(createTauriUnavailableError('Tauri command test').message).toBe(
			'Tauri command test is only available in the Tauri runtime'
		);
	});
});
