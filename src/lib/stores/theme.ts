/**
 * Theme Store
 * Manages application theme (light/dark mode) with persistence
 */
import { writable } from 'svelte/store';

/**
 * Theme type definition
 */
export type Theme = 'light' | 'dark';

/**
 * Creates a theme store with persistence and system preference detection
 * @returns Theme store with methods for theme management
 */
function createThemeStore() {
	const { subscribe, set } = writable<Theme>('light');

	return {
		subscribe,

		/**
		 * Set the theme and persist to localStorage
		 * @param theme - The theme to apply
		 */
		setTheme: (theme: Theme): void => {
			if (typeof document !== 'undefined') {
				document.documentElement.setAttribute('data-theme', theme);
				localStorage.setItem('theme', theme);
			}
			set(theme);
		},

		/**
		 * Toggle between light and dark themes
		 */
		toggle: (): void => {
			let currentTheme: Theme = 'light';

			const unsubscribe = subscribe((value) => {
				currentTheme = value;
			});
			unsubscribe();

			const nextTheme: Theme = currentTheme === 'light' ? 'dark' : 'light';

			if (typeof document !== 'undefined') {
				document.documentElement.setAttribute('data-theme', nextTheme);
				localStorage.setItem('theme', nextTheme);
			}
			set(nextTheme);
		},

		/**
		 * Initialize theme from localStorage or system preference
		 */
		init: (): void => {
			if (typeof window === 'undefined') return;

			const saved = localStorage.getItem('theme') as Theme | null;
			const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
			const theme: Theme = saved || (prefersDark ? 'dark' : 'light');

			document.documentElement.setAttribute('data-theme', theme);
			set(theme);
		}
	};
}

/**
 * Theme store instance
 */
export const theme = createThemeStore();
