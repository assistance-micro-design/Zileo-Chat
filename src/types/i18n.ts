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
