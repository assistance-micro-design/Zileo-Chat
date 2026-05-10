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

import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriInvoke } from '../core';
import { openDialog, saveDialog } from '../dialog';
import { tauriListen } from '../events';
import { openExternalUrl } from '../opener';
import { setTauriWindowTheme } from '../window';

describe('Tauri adapter browser fallbacks', () => {
	afterEach(() => {
		delete (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
	});

	it('returns a no-op unlistener outside Tauri', async () => {
		const handler = vi.fn();
		const unlisten = await tauriListen('test-event', handler);

		expect(unlisten).toEqual(expect.any(Function));
		expect(() => unlisten()).not.toThrow();
		expect(handler).not.toHaveBeenCalled();
	});

	it('does not throw when applying the native window theme outside Tauri', async () => {
		await expect(setTauriWindowTheme('dark')).resolves.toBeUndefined();
	});

	it('throws a controlled error for invoke outside Tauri', async () => {
		await expect(tauriInvoke('test_command')).rejects.toThrow(
			'Tauri command "test_command" is only available in the Tauri runtime'
		);
	});

	it('throws a controlled error for dialogs outside Tauri', async () => {
		await expect(openDialog()).rejects.toThrow(
			'Tauri open dialog is only available in the Tauri runtime'
		);
		await expect(saveDialog()).rejects.toThrow(
			'Tauri save dialog is only available in the Tauri runtime'
		);
	});

	it('throws a controlled error for external URL opening outside Tauri', async () => {
		await expect(openExternalUrl('https://example.com')).rejects.toThrow(
			'Tauri external URL opener is only available in the Tauri runtime'
		);
	});
});
