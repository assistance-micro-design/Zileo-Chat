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
	supportsXhighReasoning,
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

	it('keeps xhigh when the model is DeepSeek', () => {
		expect(normalizeReasoningEffortForProvider('custom', 'xhigh', 'deepseek-v4')).toBe('xhigh');
		expect(normalizeReasoningEffortForProvider('custom', 'xhigh', 'pro/deepseek-r1')).toBe('xhigh');
	});

	it('downgrades xhigh to high when the model is known and unsupported', () => {
		expect(normalizeReasoningEffortForProvider('custom', 'xhigh', 'mistral-large')).toBe('high');
		expect(normalizeReasoningEffortForProvider('ollama', 'xhigh', 'qwen3')).toBe('high');
	});

	it('preserves a stored xhigh while the model is still unknown (list loading)', () => {
		// On form reopen the LLM list loads asynchronously, so `selectedModel`
		// (and thus its api_name) is undefined for the first render. An unknown
		// model must NOT be treated as "does not support xhigh", otherwise a
		// persisted xhigh is clobbered to high before the model resolves.
		expect(normalizeReasoningEffortForProvider('custom', 'xhigh', undefined)).toBe('xhigh');
		expect(normalizeReasoningEffortForProvider('custom', 'xhigh', null)).toBe('xhigh');
		expect(normalizeReasoningEffortForProvider('custom', 'xhigh', '')).toBe('xhigh');
	});

	it('collapses xhigh to high on Mistral regardless of the model', () => {
		// Mistral only accepts high/none, so the provider gate takes precedence
		// over (and does not depend on) the model api_name.
		expect(normalizeReasoningEffortForProvider('mistral', 'xhigh', undefined)).toBe('high');
		expect(normalizeReasoningEffortForProvider('mistral', 'xhigh', 'deepseek-v4')).toBe('high');
	});
});

describe('supportsXhighReasoning', () => {
	it('matches every supported family (case-insensitive)', () => {
		expect(supportsXhighReasoning('deepseek-v4')).toBe(true);
		expect(supportsXhighReasoning('DeepSeek-R1')).toBe(true);
		expect(supportsXhighReasoning('pro/deepseek-chat-v4')).toBe(true);
		expect(supportsXhighReasoning('gpt-5.1-codex-max')).toBe(true);
		expect(supportsXhighReasoning('openai/gpt-5.2-pro')).toBe(true);
		expect(supportsXhighReasoning('gpt-5.5')).toBe(true);
		expect(supportsXhighReasoning('x-ai/grok-4')).toBe(true);
		expect(supportsXhighReasoning('anthropic/claude-opus-4-7')).toBe(true);
		// Future-proof: any claude-opus / gpt-5.x / grok / deepseek variant.
		expect(supportsXhighReasoning('claude-opus-5')).toBe(true);
		expect(supportsXhighReasoning('gpt-5.9-turbo')).toBe(true);
	});

	it('returns false for unrelated names or empty input', () => {
		expect(supportsXhighReasoning('mistral-large')).toBe(false);
		expect(supportsXhighReasoning('qwen3')).toBe(false);
		expect(supportsXhighReasoning('gpt-4o')).toBe(false);
		// Base "gpt-5" without point release only exposes low/medium/high; the
		// trailing dot in the `gpt-5.` pattern excludes it.
		expect(supportsXhighReasoning('gpt-5')).toBe(false);
		expect(supportsXhighReasoning('claude-sonnet-4-7')).toBe(false);
		expect(supportsXhighReasoning('glm-4.6')).toBe(false);
		expect(supportsXhighReasoning('')).toBe(false);
		expect(supportsXhighReasoning(undefined)).toBe(false);
		expect(supportsXhighReasoning(null)).toBe(false);
	});
});

describe('getReasoningOptions xhigh gating', () => {
	it('exposes xhigh when the model is in a supported family', () => {
		const deepseek = getReasoningOptions('custom', identityTranslator, 'deepseek-v4');
		expect(deepseek.map((o) => o.value)).toEqual(['', 'low', 'medium', 'high', 'xhigh']);
		const grok = getReasoningOptions('custom', identityTranslator, 'x-ai/grok-4');
		expect(grok.map((o) => o.value)).toEqual(['', 'low', 'medium', 'high', 'xhigh']);
	});

	it('hides xhigh for unsupported models', () => {
		const opts = getReasoningOptions('custom', identityTranslator, 'mistral-large');
		expect(opts.map((o) => o.value)).toEqual(['', 'low', 'medium', 'high']);
	});

	it('hides xhigh when no model is provided', () => {
		const opts = getReasoningOptions('custom', identityTranslator);
		expect(opts.map((o) => o.value)).toEqual(['', 'low', 'medium', 'high']);
	});

	it('never exposes xhigh on Mistral regardless of the model name', () => {
		// Hypothetical edge case: Mistral provider with a model api_name containing
		// "deepseek". The provider gate takes precedence over the model gate.
		const opts = getReasoningOptions('mistral', identityTranslator, 'deepseek-via-mistral');
		expect(opts.map((o) => o.value)).toEqual(['', 'high']);
	});
});
