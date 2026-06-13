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
 * UI Zoom Store
 *
 * Drives the native Tauri webview zoom (Ctrl + / Ctrl - / Ctrl 0) so the whole
 * interface — text, layout and images — scales up or down. The factor is
 * persisted to localStorage and re-applied on startup, because the native zoom
 * itself does not survive an app restart.
 */
import { writable, get } from 'svelte/store';
import { setTauriWebviewZoom } from '$lib/tauri';

/** Smallest allowed zoom factor (50%). */
export const MIN_ZOOM = 0.5;
/** Largest allowed zoom factor (200%). */
export const MAX_ZOOM = 2;
/** Increment applied by a single zoom-in / zoom-out step. */
export const ZOOM_STEP = 0.1;
/** Default zoom factor (100%). */
export const DEFAULT_ZOOM = 1;

const STORAGE_KEY = 'ui-zoom';

/** Discrete zoom intent derived from a keyboard shortcut. */
export type ZoomAction = 'in' | 'out' | 'reset';

/** Minimal shape of a keyboard event needed to resolve a zoom action. */
export type ZoomKeyEvent = Pick<KeyboardEvent, 'ctrlKey' | 'metaKey' | 'key' | 'code'>;

/**
 * Clamps a zoom factor into the allowed range and rounds away floating-point
 * drift. Non-finite input falls back to {@link DEFAULT_ZOOM}.
 *
 * @param factor - Raw zoom factor
 * @returns A safe factor within [MIN_ZOOM, MAX_ZOOM], rounded to two decimals
 */
export function clampZoom(factor: number): number {
	if (!Number.isFinite(factor)) {
		return DEFAULT_ZOOM;
	}
	const bounded = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, factor));
	return Math.round(bounded * 100) / 100;
}

/**
 * Formats a zoom factor as a whole-percentage label for display (e.g. 1.3 →
 * "130 %"), with a space before the percent sign per French typography.
 *
 * @param factor - Zoom factor
 * @returns The percentage label
 */
export function formatZoomPercent(factor: number): string {
	return `${Math.round(factor * 100)} %`;
}

/**
 * Computes the next zoom factor for a discrete action, keeping the result
 * within bounds. `reset` always returns {@link DEFAULT_ZOOM}.
 *
 * @param current - Current zoom factor
 * @param action - Zoom intent
 * @returns The clamped next zoom factor
 */
export function nextZoom(current: number, action: ZoomAction): number {
	switch (action) {
		case 'in':
			return clampZoom(current + ZOOM_STEP);
		case 'out':
			return clampZoom(current - ZOOM_STEP);
		case 'reset':
			return DEFAULT_ZOOM;
	}
}

/**
 * Resolves a keyboard event to a zoom action, or null when it is not a zoom
 * shortcut. Requires Ctrl (or Cmd on macOS). Matches both the produced
 * character (`key`) and the physical position (`code`) so the shortcuts work
 * across keyboard layouts (AZERTY, numpad, …).
 *
 * @param event - Keyboard event (or a compatible subset)
 * @returns The zoom action, or null
 */
export function zoomActionForKey(event: ZoomKeyEvent): ZoomAction | null {
	if (!event.ctrlKey && !event.metaKey) {
		return null;
	}
	const { key, code } = event;
	if (key === '+' || key === '=' || code === 'Equal' || code === 'NumpadAdd') {
		return 'in';
	}
	if (key === '-' || key === '_' || code === 'Minus' || code === 'NumpadSubtract') {
		return 'out';
	}
	if (key === '0' || code === 'Digit0' || code === 'Numpad0') {
		return 'reset';
	}
	return null;
}

const store = writable<number>(DEFAULT_ZOOM);

function getSavedZoom(): number | null {
	if (typeof window === 'undefined' || !('localStorage' in window)) return null;

	try {
		const saved = window.localStorage.getItem(STORAGE_KEY);
		if (saved === null) return null;
		const parsed = Number.parseFloat(saved);
		return Number.isFinite(parsed) ? parsed : null;
	} catch {
		return null;
	}
}

function persistZoom(factor: number): void {
	if (typeof window === 'undefined' || !('localStorage' in window)) return;

	try {
		window.localStorage.setItem(STORAGE_KEY, String(factor));
	} catch {
		// localStorage may fail (quota exceeded, private browsing)
	}
}

function syncWebviewZoom(factor: number): void {
	void setTauriWebviewZoom(factor).catch(() => {
		// Ignore: no-op outside Tauri, or the native call failed. The DOM never
		// depends on this, so a zoom request must never break the calling flow.
	});
}

/**
 * UI zoom store with persistence and native webview synchronization.
 */
export const uiZoom = {
	/** Subscribe to zoom factor changes. */
	subscribe: store.subscribe,

	/**
	 * Set the zoom factor explicitly, clamping, persisting and applying it to
	 * the native webview.
	 *
	 * @param factor - Desired zoom factor (clamped to the allowed range)
	 */
	setZoom: (factor: number): void => {
		const value = clampZoom(factor);
		store.set(value);
		persistZoom(value);
		syncWebviewZoom(value);
	},

	/**
	 * Apply a discrete zoom action relative to the current factor.
	 *
	 * @param action - Zoom intent (in / out / reset)
	 */
	step: (action: ZoomAction): void => {
		uiZoom.setZoom(nextZoom(get(store), action));
	},

	/** Increase the zoom by one step. */
	increase: (): void => uiZoom.step('in'),

	/** Decrease the zoom by one step. */
	decrease: (): void => uiZoom.step('out'),

	/** Reset the zoom to the default factor. */
	reset: (): void => uiZoom.step('reset'),

	/**
	 * Restore the persisted zoom factor and re-apply it to the native webview.
	 * Safe to call once at app startup; the native zoom does not survive a
	 * restart, so re-applying is what makes the preference stick.
	 */
	init: (): void => {
		if (typeof window === 'undefined') return;
		const saved = getSavedZoom();
		const value = clampZoom(saved ?? DEFAULT_ZOOM);
		store.set(value);
		syncWebviewZoom(value);
	}
};
