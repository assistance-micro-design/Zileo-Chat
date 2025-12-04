/**
 * Locale Store
 * Manages application language with persistence.
 * Pattern: copied from theme.ts
 */
import { writable, derived, get } from 'svelte/store';
import type { Locale } from '$types/i18n';
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, LOCALES } from '$types/i18n';
import { setLanguageTag, isAvailableLanguageTag } from '$lib/i18n/generated/runtime';

const STORAGE_KEY = 'locale';

/**
 * Creates a locale store with persistence and system preference detection
 * @returns Locale store with methods for locale management
 */
function createLocaleStore() {
	const { subscribe, set } = writable<Locale>(DEFAULT_LOCALE);

	return {
		subscribe,

		/**
		 * Set the locale and persist to localStorage
		 * @param locale - The locale to apply
		 */
		setLocale: (locale: Locale): void => {
			if (!SUPPORTED_LOCALES.includes(locale)) {
				console.warn(`Unsupported locale: ${locale}, falling back to ${DEFAULT_LOCALE}`);
				locale = DEFAULT_LOCALE;
			}

			if (typeof document !== 'undefined') {
				document.documentElement.setAttribute('lang', locale);
				localStorage.setItem(STORAGE_KEY, locale);
			}

			// Update Paraglide runtime
			setLanguageTag(locale);
			set(locale);
		},

		/**
		 * Initialize locale from localStorage or system preference
		 */
		init: (): void => {
			if (typeof window === 'undefined') return;

			// Priority: localStorage > navigator.language > default
			const saved = localStorage.getItem(STORAGE_KEY);
			let locale: Locale = DEFAULT_LOCALE;

			if (saved && isAvailableLanguageTag(saved)) {
				locale = saved as Locale;
			} else {
				// Detect system language
				const browserLang = navigator.language.split('-')[0];
				if (isAvailableLanguageTag(browserLang)) {
					locale = browserLang as Locale;
				}
			}

			document.documentElement.setAttribute('lang', locale);
			setLanguageTag(locale);
			set(locale);
		},

		/**
		 * Get current locale info
		 * @returns LocaleInfo object for current locale
		 */
		getInfo: (): (typeof LOCALES)[Locale] => {
			const current = get({ subscribe });
			return LOCALES[current];
		}
	};
}

/**
 * Locale store instance
 */
export const localeStore = createLocaleStore();

/**
 * Derived store for current locale value (reactive)
 */
export const locale = derived(localeStore, ($locale) => $locale);

/**
 * Derived store for current locale info (reactive)
 */
export const localeInfo = derived(localeStore, ($locale) => LOCALES[$locale]);
