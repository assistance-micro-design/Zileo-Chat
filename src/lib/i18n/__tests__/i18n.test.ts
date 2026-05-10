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

import { afterEach, describe, expect, it } from 'vitest';
import en from '$messages/en.json';
import fr from '$messages/fr.json';
import { isAvailableLanguageTag, languageTag, setLanguageTag, t, type Messages } from '../index';
import { DEFAULT_LOCALE } from '$types/i18n';

const SIMPLE_PLACEHOLDER_REGEX = /\{(\w+)\}/g;
const ICU_PLURAL_START_REGEX = /^\{(\w+),\s*plural,/;

function sortedKeys(messages: Messages): string[] {
	return Object.keys(messages).sort();
}

function findIcuPluralEnd(value: string, startIndex: number): number {
	let depth = 0;
	for (let index = startIndex; index < value.length; index += 1) {
		if (value[index] === '{') depth += 1;
		if (value[index] === '}') depth -= 1;
		if (depth === 0) return index + 1;
	}
	return value.length;
}

function placeholdersFor(value: string): string[] {
	const placeholders = new Set<string>();
	let simpleText = '';

	for (let index = 0; index < value.length; ) {
		const pluralMatch = value.slice(index).match(ICU_PLURAL_START_REGEX);
		if (pluralMatch) {
			placeholders.add(pluralMatch[1]!);
			index = findIcuPluralEnd(value, index);
			continue;
		}

		simpleText += value[index];
		index += 1;
	}

	for (const match of simpleText.matchAll(SIMPLE_PLACEHOLDER_REGEX)) {
		placeholders.add(match[1]!);
	}

	return Array.from(placeholders).sort();
}

describe('i18n translations', () => {
	afterEach(() => {
		setLanguageTag(DEFAULT_LOCALE);
	});

	it('keeps English and French translation keys in parity', () => {
		expect(sortedKeys(fr as Messages)).toEqual(sortedKeys(en as Messages));
	});

	it('keeps interpolation placeholders in parity for every translated key', () => {
		for (const key of sortedKeys(en as Messages)) {
			expect(placeholdersFor((fr as Messages)[key] ?? ''), key).toEqual(
				placeholdersFor((en as Messages)[key] ?? '')
			);
		}
	});

	it('interpolates translation parameters', () => {
		setLanguageTag('en');

		expect(t('settings_llm_load_failed', { error: 'Network error' })).toBe(
			'Failed to load LLM data: Network error'
		);
	});

	it('falls back to the key when a translation is missing', () => {
		expect(t('missing_translation_key')).toBe('missing_translation_key');
	});

	it('falls back to the default locale for unsupported language tags', () => {
		setLanguageTag('fr');
		expect(languageTag()).toBe('fr');

		setLanguageTag('de');

		expect(languageTag()).toBe(DEFAULT_LOCALE);
	});

	it('detects available language tags', () => {
		expect(isAvailableLanguageTag('en')).toBe(true);
		expect(isAvailableLanguageTag('fr')).toBe(true);
		expect(isAvailableLanguageTag('de')).toBe(false);
	});
});
