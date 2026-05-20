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

import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { captureActiveField, insertTextIntoField } from '../dom-insert';

function mountInput(type: string, value = ''): HTMLInputElement {
	const el = document.createElement('input');
	if (type) el.type = type;
	el.value = value;
	document.body.appendChild(el);
	return el;
}

function mountTextarea(value = ''): HTMLTextAreaElement {
	const el = document.createElement('textarea');
	el.value = value;
	document.body.appendChild(el);
	return el;
}

describe('captureActiveField', () => {
	afterEach(() => {
		document.body.innerHTML = '';
	});

	it('returns null when nothing is focused', () => {
		expect(captureActiveField()).toBeNull();
	});

	it('captures a focused textarea with its selection', () => {
		const el = mountTextarea('hello world');
		el.focus();
		el.setSelectionRange(2, 5);

		const captured = captureActiveField();
		expect(captured).not.toBeNull();
		expect(captured!.el).toBe(el);
		expect(captured!.selectionStart).toBe(2);
		expect(captured!.selectionEnd).toBe(5);
	});

	it('captures focused text-like inputs', () => {
		for (const type of ['text', 'search', 'url', 'tel', 'email']) {
			document.body.innerHTML = '';
			const el = mountInput(type, 'abc');
			el.focus();
			const captured = captureActiveField();
			expect(captured, `type=${type}`).not.toBeNull();
			expect(captured!.el).toBe(el);
		}
	});

	it('rejects focused password inputs (dictation should never land in a secret)', () => {
		const el = mountInput('password', 'hunter2');
		el.focus();
		expect(captureActiveField()).toBeNull();
	});

	it('captures an input with no explicit type (defaults to text)', () => {
		const el = mountInput('', 'value');
		el.focus();
		expect(captureActiveField()).not.toBeNull();
	});

	it('rejects non-text inputs (checkbox, radio, file, ...)', () => {
		for (const type of ['checkbox', 'radio', 'file', 'submit', 'button', 'range']) {
			document.body.innerHTML = '';
			const el = mountInput(type);
			el.focus();
			expect(captureActiveField(), `type=${type}`).toBeNull();
		}
	});

	it('rejects contenteditable divs (the FAB does not support them yet)', () => {
		const div = document.createElement('div');
		div.contentEditable = 'true';
		div.tabIndex = 0;
		document.body.appendChild(div);
		div.focus();
		expect(captureActiveField()).toBeNull();
	});

	it('falls back to value.length when selection accessors return null', () => {
		const el = mountTextarea('hello');
		el.focus();
		// Some browsers return null for non-text inputs; emulate by stubbing.
		Object.defineProperty(el, 'selectionStart', { value: null, configurable: true });
		Object.defineProperty(el, 'selectionEnd', { value: null, configurable: true });
		const captured = captureActiveField();
		expect(captured!.selectionStart).toBe(5);
		expect(captured!.selectionEnd).toBe(5);
	});
});

describe('insertTextIntoField', () => {
	let el: HTMLTextAreaElement;
	let inputEvents: InputEvent[];

	beforeEach(() => {
		el = mountTextarea('hello world');
		inputEvents = [];
		el.addEventListener('input', (ev) => inputEvents.push(ev as InputEvent));
	});

	afterEach(() => {
		document.body.innerHTML = '';
	});

	it('inserts text at the captured cursor when the range is collapsed', () => {
		insertTextIntoField(el, 5, 5, ' there');
		expect(el.value).toBe('hello there world');
	});

	it('replaces the selected range when start != end', () => {
		insertTextIntoField(el, 6, 11, 'vitest');
		expect(el.value).toBe('hello vitest');
	});

	it('leaves the caret at the end of the inserted text', () => {
		insertTextIntoField(el, 5, 5, ' there');
		expect(el.selectionStart).toBe(11);
		expect(el.selectionEnd).toBe(11);
	});

	it('dispatches a bubbling InputEvent so Svelte bind:value picks it up', () => {
		insertTextIntoField(el, 0, 0, 'X');
		expect(inputEvents).toHaveLength(1);
		const ev = inputEvents[0]!;
		expect(ev.bubbles).toBe(true);
		expect(ev.inputType).toBe('insertText');
		expect(ev.data).toBe('X');
	});

	it('is a no-op when text is empty', () => {
		insertTextIntoField(el, 0, 11, '');
		expect(el.value).toBe('hello world');
		expect(inputEvents).toHaveLength(0);
	});

	it('clamps negative offsets to 0', () => {
		insertTextIntoField(el, -10, -5, 'X');
		expect(el.value.startsWith('X')).toBe(true);
	});

	it('clamps offsets past value.length to the end', () => {
		insertTextIntoField(el, 999, 1000, '!');
		expect(el.value).toBe('hello world!');
	});

	it('clamps end below start to start (selection inverted post-truncate)', () => {
		// Simulate the user trimming the field while transcription was in flight:
		// savedEnd is now beyond the current length, but savedStart is still valid.
		el.value = 'hi';
		insertTextIntoField(el, 2, 1, ' there');
		expect(el.value).toBe('hi there');
	});

	it('focuses the field as a side-effect (so the caret is visible)', () => {
		(document.activeElement as HTMLElement | null)?.blur?.();
		expect(document.activeElement).not.toBe(el);
		insertTextIntoField(el, 0, 0, 'x');
		expect(document.activeElement).toBe(el);
	});
});
