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
 * @fileoverview Focus trap attachment for modal dialogs.
 *
 * Implements WCAG 2.1 modal dialog focus requirements:
 *  - Remember the previously focused element on mount.
 *  - Move focus to the first focusable element inside the modal.
 *  - Keep Tab navigation inside the modal by wrapping around the first/last
 *    focusable elements.
 *  - Restore focus to the previously active element on unmount.
 *
 * @module lib/actions/focusTrap
 */

import type { Attachment } from 'svelte/attachments';

/**
 * CSS selector matching every natively focusable element that can receive
 * keyboard focus. Hidden elements and `tabindex="-1"` are excluded.
 */
const FOCUSABLE_SELECTOR = [
	'a[href]',
	'area[href]',
	'button:not([disabled])',
	'input:not([disabled])',
	'select:not([disabled])',
	'textarea:not([disabled])',
	'[tabindex]:not([tabindex="-1"])',
	'[contenteditable="true"]'
].join(',');

/**
 * Returns the ordered list of focusable descendants of the given container.
 */
function getFocusableElements(container: HTMLElement): HTMLElement[] {
	return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR)).filter(
		(el) => !el.hasAttribute('disabled') && el.offsetParent !== null
	);
}

/**
 * Svelte 5 attachment that traps keyboard focus inside the element and
 * restores the previously focused element on teardown.
 *
 * @example
 * ```svelte
 * <div {@attach focusTrap}>…</div>
 * ```
 */
export const focusTrap: Attachment<HTMLElement> = (element) => {
	const previouslyFocused =
		document.activeElement instanceof HTMLElement ? document.activeElement : null;

	// Defer initial focus to the next microtask so the modal contents have a
	// chance to mount before we query for focusable descendants.
	queueMicrotask(() => {
		const focusables = getFocusableElements(element);
		const autoFocus = element.querySelector<HTMLElement>('[autofocus]');
		const target = autoFocus ?? focusables.find((el) => el.tagName !== 'BUTTON') ?? focusables[0];
		target?.focus();
	});

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key !== 'Tab') return;

		const focusables = getFocusableElements(element);
		if (focusables.length === 0) {
			event.preventDefault();
			element.focus();
			return;
		}

		const first = focusables[0];
		const last = focusables[focusables.length - 1];
		const active = document.activeElement as HTMLElement | null;

		if (event.shiftKey) {
			if (active === first || !element.contains(active)) {
				event.preventDefault();
				last.focus();
			}
		} else {
			if (active === last || !element.contains(active)) {
				event.preventDefault();
				first.focus();
			}
		}
	}

	element.addEventListener('keydown', handleKeydown);

	return () => {
		element.removeEventListener('keydown', handleKeydown);
		previouslyFocused?.focus();
	};
};
