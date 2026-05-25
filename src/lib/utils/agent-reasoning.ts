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
 * Substrings (lowercase) that flag a model as supporting the `xhigh`
 * reasoning tier on OpenAI-compatible gateways (OpenRouter primarily).
 *
 * Kept deliberately broad to absorb future versions without re-editing:
 * `gpt-5.` matches every 5.x point release (5.1/5.2/5.5/...) without
 * catching the base `gpt-5` that only exposes low/medium/high. `deepseek`,
 * `grok` and `claude-opus` catch every variant of those families. If a
 * gateway rejects `xhigh` on an older opus (4.5/4.6) the error surfaces
 * cleanly — false positives are benign, false negatives would silently
 * hide a tier from users.
 */
const XHIGH_MODEL_PATTERNS = ['deepseek', 'gpt-5.', 'grok', 'claude-opus'] as const;

/**
 * Returns true when the model's `api_name` matches a known family that
 * exposes the `xhigh` reasoning tier (see {@link XHIGH_MODEL_PATTERNS}).
 */
export function supportsXhighReasoning(apiName: string | undefined | null): boolean {
	if (!apiName) return false;
	const lower = apiName.toLowerCase();
	return XHIGH_MODEL_PATTERNS.some((pattern) => lower.includes(pattern));
}

/**
 * Returns the reasoning-effort options to expose for the given provider.
 *
 * Mistral models do not expose intensity levels: only "Off" and "High" are
 * valid. All other providers keep the full range. The `xhigh` tier ("Think
 * Max") is only added when `modelApiName` matches a family known to expose
 * it (see {@link supportsXhighReasoning}).
 */
export function getReasoningOptions(
	provider: string,
	t: Translator,
	modelApiName?: string | null
): ReasoningOption[] {
	const off: ReasoningOption = { value: '', label: t('agents_reasoning_off') };
	if (isMistralProvider(provider)) {
		return [off, { value: 'high', label: t('agents_reasoning_high') }];
	}
	const base: ReasoningOption[] = [
		off,
		{ value: 'low', label: t('agents_reasoning_low') },
		{ value: 'medium', label: t('agents_reasoning_medium') },
		{ value: 'high', label: t('agents_reasoning_high') }
	];
	if (supportsXhighReasoning(modelApiName)) {
		base.push({ value: 'xhigh', label: t('agents_reasoning_xhigh') });
	}
	return base;
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
 * For Mistral, low/medium/xhigh are not exposed in the selector. They are
 * mapped server-side to "high" anyway (see ReasoningEffort::to_mistral_str),
 * so the UI returns "high" so the Select can display the user's intent without
 * silently dropping it. The Mistral gate is provider-only and does not depend
 * on the model.
 *
 * For other providers, `xhigh` is valid only for the families listed in
 * {@link XHIGH_MODEL_PATTERNS}. It is downgraded to `high` ONLY when the model
 * is known (a non-empty `modelApiName`) and does not support it — e.g. when the
 * user switches to another model while xhigh is selected. A `null`/`undefined`/
 * empty `modelApiName` means the model is still unknown (the LLM list loads
 * asynchronously when the form opens); in that window the stored value is
 * preserved so a persisted `xhigh` is not clobbered before the model resolves.
 *
 * Returns the original value when no normalization is needed.
 */
export function normalizeReasoningEffortForProvider(
	provider: string,
	effort: ReasoningEffort | undefined,
	modelApiName?: string | null
): ReasoningEffort | undefined {
	if (!effort) return effort;
	if (isMistralProvider(provider)) {
		if (effort === 'low' || effort === 'medium' || effort === 'xhigh') {
			return 'high';
		}
		return effort;
	}
	// Non-Mistral: downgrade xhigh only when the model is KNOWN and unsupported.
	// An unknown model (list still loading) must not be treated as unsupported,
	// otherwise a persisted xhigh is wiped on form reopen before the model loads.
	if (effort === 'xhigh' && modelApiName != null && modelApiName !== '') {
		if (!supportsXhighReasoning(modelApiName)) {
			return 'high';
		}
	}
	return effort;
}
