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
 * Theme Store
 * Manages application theme (light/dark mode) with persistence.
 * Syncs with OS theme and Tauri window decorations.
 */
import { writable, get } from 'svelte/store';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Theme type definition
 */
export type Theme = 'light' | 'dark';

/**
 * Internal writable store
 */
const store = writable<Theme>('light');

/**
 * Sync Tauri window theme.
 * Pass null to follow the OS theme natively.
 */
async function syncWindowTheme(value: Theme | null): Promise<void> {
	try {
		await getCurrentWindow().setTheme(value);
	} catch {
		// Ignore errors (e.g. during SSR or tests)
	}
}

/**
 * Theme store with persistence and system preference detection
 */
export const theme = {
	/**
	 * Subscribe to theme changes
	 */
	subscribe: store.subscribe,

	/**
	 * Set the theme explicitly and persist to localStorage
	 * @param value - The theme to apply
	 */
	setTheme: (value: Theme): void => {
		if (typeof document !== 'undefined') {
			document.documentElement.setAttribute('data-theme', value);
			localStorage.setItem('theme', value);
		}
		store.set(value);
		syncWindowTheme(value);
	},

	/**
	 * Toggle between light and dark themes
	 */
	toggle: (): void => {
		const currentTheme = get(store);
		theme.setTheme(currentTheme === 'light' ? 'dark' : 'light');
	},

	/**
	 * Initialize theme from localStorage or system preference.
	 * When no user preference is saved, delegates to OS via setTheme(null).
	 */
	init: (): void => {
		if (typeof window === 'undefined') return;

		const saved = localStorage.getItem('theme') as Theme | null;

		if (saved) {
			document.documentElement.setAttribute('data-theme', saved);
			store.set(saved);
			syncWindowTheme(saved);
		} else {
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			const value: Theme = prefersDark ? 'dark' : 'light';
			document.documentElement.setAttribute('data-theme', value);
			store.set(value);
			// Let Tauri follow OS theme natively
			syncWindowTheme(null);
		}

		window.matchMedia('(prefers-color-scheme: dark)').addEventListener(
			'change',
			(e: MediaQueryListEvent) => {
				if (!localStorage.getItem('theme')) {
					const value: Theme = e.matches ? 'dark' : 'light';
					document.documentElement.setAttribute('data-theme', value);
					store.set(value);
					// Keep Tauri following OS natively
					syncWindowTheme(null);
				}
			}
		);
	}
};
