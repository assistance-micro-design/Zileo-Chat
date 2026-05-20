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

/**
 * @fileoverview Helpers to insert dictated text back into the field the
 * user was focused on before triggering the FAB.
 *
 * Svelte 5's `bind:value` only updates when the DOM emits an `input` event,
 * so `setRangeText` alone is not enough — we must dispatch a synthetic
 * `InputEvent` afterwards.
 */

export type EditableField = HTMLTextAreaElement | HTMLInputElement;

/** Result of {@link captureActiveField} — `null` when no editable field is focused. */
export interface CapturedField {
	el: EditableField;
	selectionStart: number;
	selectionEnd: number;
}

/**
 * Returns true when the element is a text-bearing `<input>` or a `<textarea>`.
 * The `<input>` type whitelist matches the spec's "text-like" controls
 * (`text`, `search`, `url`, `tel`, `email`, `password`, plus default — when
 * no `type` attribute is present the element behaves as `text`).
 */
function isEditable(el: Element | null): el is EditableField {
	if (!el) return false;
	if (el instanceof HTMLTextAreaElement) return true;
	if (el instanceof HTMLInputElement) {
		const t = el.type.toLowerCase();
		return (
			t === 'text' || t === 'search' || t === 'url' || t === 'tel' || t === 'email' || t === ''
		);
	}
	return false;
}

/**
 * Captures the currently focused editable field plus its selection range.
 * Returns `null` when focus is not on a text-bearing field.
 */
export function captureActiveField(doc: Document = document): CapturedField | null {
	const active = doc.activeElement;
	if (!isEditable(active)) {
		return null;
	}
	const start = active.selectionStart ?? active.value.length;
	const end = active.selectionEnd ?? active.value.length;
	return { el: active, selectionStart: start, selectionEnd: end };
}

/**
 * Inserts `text` into `target` at the previously captured selection range
 * and dispatches an `InputEvent` so Svelte's `bind:value` picks the change up.
 *
 * The cursor lands at the end of the inserted text.
 */
export function insertTextIntoField(
	target: EditableField,
	savedStart: number,
	savedEnd: number,
	text: string
): void {
	if (!text) return;

	target.focus();
	// Clamp in case the user truncated the field while transcription was in flight.
	const len = target.value.length;
	const start = Math.min(Math.max(savedStart, 0), len);
	const end = Math.min(Math.max(savedEnd, start), len);

	if (typeof target.setRangeText === 'function') {
		target.setRangeText(text, start, end, 'end');
	} else {
		// Fallback for ancient runtimes — concatenation around the range.
		target.value = `${target.value.slice(0, start)}${text}${target.value.slice(end)}`;
		const caret = start + text.length;
		target.setSelectionRange(caret, caret);
	}

	target.dispatchEvent(
		new InputEvent('input', { bubbles: true, inputType: 'insertText', data: text })
	);
}
