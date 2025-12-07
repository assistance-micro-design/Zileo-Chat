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
 * Manages application theme (light/dark mode) with persistence
 */
import { writable, get } from 'svelte/store';

/**
 * Theme type definition
 */
export type Theme = 'light' | 'dark';

/**
 * Internal writable store
 */
const store = writable<Theme>('light');

/**
 * Theme store with persistence and system preference detection
 */
export const theme = {
	/**
	 * Subscribe to theme changes
	 */
	subscribe: store.subscribe,

	/**
	 * Set the theme and persist to localStorage
	 * @param value - The theme to apply
	 */
	setTheme: (value: Theme): void => {
		if (typeof document !== 'undefined') {
			document.documentElement.setAttribute('data-theme', value);
			localStorage.setItem('theme', value);
		}
		store.set(value);
	},

	/**
	 * Toggle between light and dark themes
	 */
	toggle: (): void => {
		const currentTheme = get(store);
		const nextTheme: Theme = currentTheme === 'light' ? 'dark' : 'light';

		if (typeof document !== 'undefined') {
			document.documentElement.setAttribute('data-theme', nextTheme);
			localStorage.setItem('theme', nextTheme);
		}
		store.set(nextTheme);
	},

	/**
	 * Initialize theme from localStorage or system preference
	 */
	init: (): void => {
		if (typeof window === 'undefined') return;

		const saved = localStorage.getItem('theme') as Theme | null;
		const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
		const value: Theme = saved || (prefersDark ? 'dark' : 'light');

		document.documentElement.setAttribute('data-theme', value);
		store.set(value);
	}
};
