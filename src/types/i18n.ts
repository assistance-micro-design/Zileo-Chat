/**
 * i18n Types and Configuration
 * Defines locale types and metadata for multi-language support.
 */

/**
 * Locale identifier (BCP 47 language tags)
 */
export type Locale = 'en' | 'fr';

/**
 * Available locales with metadata
 */
export interface LocaleInfo {
  /** Locale identifier */
  id: Locale;
  /** Display name in native language */
  nativeName: string;
  /** Flag text representation (country code) */
  flag: string;
  /** ISO 3166-1 country code for flag icon */
  countryCode: string;
}

/**
 * Locale configuration
 */
export const LOCALES: Record<Locale, LocaleInfo> = {
  en: {
    id: 'en',
    nativeName: 'English',
    flag: 'GB',
    countryCode: 'GB'
  },
  fr: {
    id: 'fr',
    nativeName: 'Francais',
    flag: 'FR',
    countryCode: 'FR'
  }
} as const;

/**
 * Supported locales array (for iteration)
 */
export const SUPPORTED_LOCALES: Locale[] = ['en', 'fr'];

/**
 * Default locale
 */
export const DEFAULT_LOCALE: Locale = 'en';
