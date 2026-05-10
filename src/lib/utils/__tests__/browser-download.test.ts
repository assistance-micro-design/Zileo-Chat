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
import { downloadBrowserFile } from '../browser-download';

describe('downloadBrowserFile', () => {
	afterEach(() => {
		document.body.innerHTML = '';
		vi.restoreAllMocks();
	});

	it('creates a temporary anchor, triggers download, and revokes the object URL', () => {
		let capturedBlob: Blob | null = null;
		const createObjectURL = vi.fn((blob: Blob) => {
			capturedBlob = blob;
			return 'blob:test-download';
		});
		const revokeObjectURL = vi.fn();
		const click = vi.fn();
		const originalCreateElement = document.createElement.bind(document);

		Object.defineProperty(URL, 'createObjectURL', { configurable: true, value: createObjectURL });
		Object.defineProperty(URL, 'revokeObjectURL', { configurable: true, value: revokeObjectURL });
		vi.spyOn(document, 'createElement').mockImplementation((tagName: string) => {
			const element = originalCreateElement(tagName);
			if (tagName.toLowerCase() === 'a') {
				element.click = click;
			}
			return element;
		});

		downloadBrowserFile('export.json', '{"ok":true}', 'application/json');

		expect(createObjectURL).toHaveBeenCalledOnce();
		expect(capturedBlob).toBeInstanceOf(Blob);
		expect(click).toHaveBeenCalledOnce();
		expect(revokeObjectURL).toHaveBeenCalledWith('blob:test-download');
		expect(document.body.querySelector('a')).toBeNull();
	});
});
