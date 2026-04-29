/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Provider-aware helpers for the Agent reasoning_effort selector.
 *
 * The Mistral API only accepts "high" or "none" for reasoning_effort, while
 * OpenAI-compatible providers (OpenRouter, vLLM, Custom, ...) accept the full
 * low/medium/high range. The Settings -> Agent form must therefore expose
 * different option sets and help text depending on the selected provider.
 */

import type { ReasoningEffort } from '$types/agent';

/** Translation function shape compatible with `$lib/i18n`'s `$i18n` store. */
export type Translator = (key: string, params?: Record<string, string | number>) => string;

/** A single reasoning-effort option rendered by the Select component. */
export interface ReasoningOption {
	/** Empty string means "Off" (no reasoning_effort sent to the backend). */
	value: '' | ReasoningEffort;
	/** Translated label shown in the select. */
	label: string;
}

/**
 * Returns true when the given provider id matches Mistral.
 *
 * Provider ids are normalized to lowercase upstream (see AgentForm.svelte),
 * but this helper applies the same normalization to be safe against future
 * callers that pass the display name directly.
 */
export function isMistralProvider(provider: string): boolean {
	return provider.toLowerCase() === 'mistral';
}

/**
 * Returns the reasoning-effort options to expose for the given provider.
 *
 * Mistral models do not expose intensity levels: only "Off" and "High" are
 * valid. All other providers keep the full range.
 */
export function getReasoningOptions(provider: string, t: Translator): ReasoningOption[] {
	const off: ReasoningOption = { value: '', label: t('agents_reasoning_off') };
	if (isMistralProvider(provider)) {
		return [off, { value: 'high', label: t('agents_reasoning_high') }];
	}
	return [
		off,
		{ value: 'low', label: t('agents_reasoning_low') },
		{ value: 'medium', label: t('agents_reasoning_medium') },
		{ value: 'high', label: t('agents_reasoning_high') }
	];
}

/**
 * Returns the help text for the reasoning-effort selector for the given
 * provider.
 *
 * Mistral gets a dedicated explanation about the lack of intensity levels;
 * other providers keep the generic tooltip.
 */
export function getReasoningHelp(provider: string, t: Translator): string {
	return isMistralProvider(provider)
		? t('agents_reasoning_mistral_help')
		: t('agents_reasoning_tooltip');
}

/**
 * Normalizes a stored reasoning_effort value to one that is selectable in
 * the UI for the given provider.
 *
 * For Mistral, low/medium are not exposed in the selector. They are mapped
 * server-side to "high" anyway (see ReasoningEffort::to_mistral_str), so the
 * UI returns "high" so the Select can display the user's intent without
 * silently dropping it. For other providers the value is returned unchanged.
 *
 * Returns the original value when no normalization is needed.
 */
export function normalizeReasoningEffortForProvider(
	provider: string,
	effort: ReasoningEffort | undefined
): ReasoningEffort | undefined {
	if (!effort) return effort;
	if (isMistralProvider(provider) && (effort === 'low' || effort === 'medium')) {
		return 'high';
	}
	return effort;
}
