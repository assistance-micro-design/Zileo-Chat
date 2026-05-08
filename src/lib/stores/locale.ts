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
 * Locale Store
 * Manages application language with persistence.
 * Pattern: copied from theme.ts
 */
import { writable, derived, get } from "svelte/store";
import type { Locale } from "$types/i18n";
import { DEFAULT_LOCALE, SUPPORTED_LOCALES, LOCALES } from "$types/i18n";
import { setLanguageTag, isAvailableLanguageTag } from "$lib/i18n";

const STORAGE_KEY = "locale";

function getSavedLocale(): string | null {
  if (typeof window === "undefined" || !("localStorage" in window)) return null;

  try {
    return window.localStorage.getItem(STORAGE_KEY);
  } catch {
    return null;
  }
}

function persistLocale(locale: Locale): void {
  if (typeof window === "undefined" || !("localStorage" in window)) return;

  try {
    window.localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    // localStorage may fail (quota exceeded, private browsing)
  }
}

function applyDocumentLocale(locale: Locale): void {
  if (typeof document !== "undefined") {
    document.documentElement.setAttribute("lang", locale);
  }
}

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
        locale = DEFAULT_LOCALE;
      }

      applyDocumentLocale(locale);
      persistLocale(locale);

      // Update i18n runtime
      setLanguageTag(locale);
      set(locale);
    },

    /**
     * Initialize locale from localStorage or system preference
     */
    init: (): void => {
      if (typeof window === "undefined") return;

      // Priority: localStorage > navigator.language > default
      const saved = getSavedLocale();
      let locale: Locale = DEFAULT_LOCALE;

      if (saved && isAvailableLanguageTag(saved)) {
        locale = saved as Locale;
      } else {
        // Detect system language
        const browserLang =
          typeof navigator !== "undefined"
            ? navigator.language.split("-")[0] ?? DEFAULT_LOCALE
            : DEFAULT_LOCALE;
        if (isAvailableLanguageTag(browserLang)) {
          locale = browserLang as Locale;
        }
      }

      applyDocumentLocale(locale);
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
    },
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
