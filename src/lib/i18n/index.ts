/**
 * Simple i18n System
 * Loads translations from JSON files and provides reactive translation functions.
 */
import { derived, writable, get } from 'svelte/store';
import type { Locale } from '$types/i18n';
import { DEFAULT_LOCALE, SUPPORTED_LOCALES } from '$types/i18n';

// Import translations statically for bundling
import en from '$messages/en.json';
import fr from '$messages/fr.json';

/**
 * Type for translation messages
 */
export type Messages = Record<string, string>;

/**
 * All translations indexed by locale
 */
const translations: Record<Locale, Messages> = {
	en: en as Messages,
	fr: fr as Messages
};

/**
 * Current locale store (internal)
 */
const currentLocale = writable<Locale>(DEFAULT_LOCALE);

/**
 * Current messages store (derived from locale)
 */
const currentMessages = derived(currentLocale, ($locale) => translations[$locale] || translations[DEFAULT_LOCALE]);

/**
 * Set the current language
 * @param locale - The locale to switch to
 */
export function setLanguageTag(locale: Locale | string): void {
	if (SUPPORTED_LOCALES.includes(locale as Locale)) {
		currentLocale.set(locale as Locale);
	} else {
		console.warn(`Unsupported locale: ${locale}, using ${DEFAULT_LOCALE}`);
		currentLocale.set(DEFAULT_LOCALE);
	}
}

/**
 * Check if a language tag is available
 * @param tag - The tag to check
 * @returns True if the tag is a supported locale
 */
export function isAvailableLanguageTag(tag: string): tag is Locale {
	return SUPPORTED_LOCALES.includes(tag as Locale);
}

/**
 * Get current language tag
 * @returns The current locale
 */
export function languageTag(): Locale {
	return get(currentLocale);
}

/**
 * Get a translation by key
 * @param key - The translation key
 * @returns The translated string or the key if not found
 */
export function t(key: string): string {
	const messages = get(currentMessages);
	return messages[key] || key;
}

/**
 * Reactive translation store
 * Subscribe to get automatic updates when locale changes
 */
export const i18n = derived(currentMessages, ($messages) => {
	return (key: string): string => $messages[key] || key;
});

/**
 * Export the locale store for subscriptions
 */
export { currentLocale as localeStore };
