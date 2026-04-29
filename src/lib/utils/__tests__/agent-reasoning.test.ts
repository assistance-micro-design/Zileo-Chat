/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Tests for provider-aware reasoning-effort UI helpers.
 */

import { describe, it, expect } from 'vitest';
import {
	getReasoningHelp,
	getReasoningOptions,
	isMistralProvider,
	normalizeReasoningEffortForProvider,
	type Translator
} from '../agent-reasoning';

const identityTranslator: Translator = (key) => key;

describe('isMistralProvider', () => {
	it('matches the lowercase mistral id', () => {
		expect(isMistralProvider('mistral')).toBe(true);
	});

	it('matches when the provider name is capitalized', () => {
		expect(isMistralProvider('Mistral')).toBe(true);
	});

	it('returns false for other providers', () => {
		expect(isMistralProvider('ollama')).toBe(false);
		expect(isMistralProvider('custom')).toBe(false);
		expect(isMistralProvider('openrouter')).toBe(false);
	});

	it('returns false for empty input', () => {
		expect(isMistralProvider('')).toBe(false);
	});
});

describe('getReasoningOptions', () => {
	it('exposes only Off and High for Mistral', () => {
		const options = getReasoningOptions('mistral', identityTranslator);
		expect(options.map((o) => o.value)).toEqual(['', 'high']);
	});

	it('exposes Off / Low / Medium / High for non-Mistral providers', () => {
		const options = getReasoningOptions('custom', identityTranslator);
		expect(options.map((o) => o.value)).toEqual(['', 'low', 'medium', 'high']);
	});

	it('exposes Off / Low / Medium / High for Ollama', () => {
		const options = getReasoningOptions('ollama', identityTranslator);
		expect(options.map((o) => o.value)).toEqual(['', 'low', 'medium', 'high']);
	});

	it('uses the provided translator for labels', () => {
		const labels = getReasoningOptions('mistral', identityTranslator).map((o) => o.label);
		expect(labels).toEqual(['agents_reasoning_off', 'agents_reasoning_high']);
	});

	it('treats capitalized "Mistral" the same as lowercase', () => {
		const lower = getReasoningOptions('mistral', identityTranslator);
		const upper = getReasoningOptions('Mistral', identityTranslator);
		expect(upper).toEqual(lower);
	});
});

describe('getReasoningHelp', () => {
	it('returns the Mistral-specific help for Mistral', () => {
		expect(getReasoningHelp('mistral', identityTranslator)).toBe('agents_reasoning_mistral_help');
	});

	it('returns the generic tooltip for other providers', () => {
		expect(getReasoningHelp('custom', identityTranslator)).toBe('agents_reasoning_tooltip');
		expect(getReasoningHelp('ollama', identityTranslator)).toBe('agents_reasoning_tooltip');
	});
});

describe('normalizeReasoningEffortForProvider', () => {
	it('keeps undefined unchanged', () => {
		expect(normalizeReasoningEffortForProvider('mistral', undefined)).toBeUndefined();
		expect(normalizeReasoningEffortForProvider('custom', undefined)).toBeUndefined();
	});

	it('keeps high unchanged for any provider', () => {
		expect(normalizeReasoningEffortForProvider('mistral', 'high')).toBe('high');
		expect(normalizeReasoningEffortForProvider('custom', 'high')).toBe('high');
	});

	it('promotes low/medium to high on Mistral to match the backend mapping', () => {
		expect(normalizeReasoningEffortForProvider('mistral', 'low')).toBe('high');
		expect(normalizeReasoningEffortForProvider('mistral', 'medium')).toBe('high');
	});

	it('keeps low/medium unchanged for non-Mistral providers', () => {
		expect(normalizeReasoningEffortForProvider('custom', 'low')).toBe('low');
		expect(normalizeReasoningEffortForProvider('custom', 'medium')).toBe('medium');
		expect(normalizeReasoningEffortForProvider('ollama', 'low')).toBe('low');
	});
});
