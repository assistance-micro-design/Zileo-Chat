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
 * Token store for managing token usage and cost tracking.
 * Provides reactive state management for streaming and cumulative token metrics.
 * @module stores/tokens
 */

import { writable, derived } from 'svelte/store';
import type { TokenDisplayData, Workflow } from '$types/workflow';
import type { LLMModel } from '$types/llm';

/**
 * State interface for the token store
 */
interface TokenState {
	/** Streaming token metrics (current session) */
	streaming: {
		input: number;
		output: number;
		cached: number | null;
		cacheWrite: number | null;
		speed: number | null;
	};
	/** Cumulative token metrics (entire workflow - main agent only) */
	cumulative: {
		input: number;
		output: number;
		cost: number;
		cached: number | null;
		cacheWrite: number | null;
	};
	/** Sub-agent cumulative token metrics */
	subAgent: {
		input: number;
		output: number;
	};
	/** Current context window usage (last API call input tokens) */
	contextUsed: number;
	/** Model context window size */
	contextMax: number;
	/** Input token price (per million tokens) */
	inputPrice: number;
	/** Output token price (per million tokens) */
	outputPrice: number;
	/** Cache-read token price (per million tokens) */
	cacheReadPrice: number;
	/** Cache-write token price (per million tokens) */
	cacheWritePrice: number;
	/** Whether streaming is currently active */
	isStreaming: boolean;
	/** Timestamp when streaming started */
	streamStartTime: number | null;
	/** Session cost from backend (more accurate than frontend calculation) */
	sessionCost: number | null;
}

/**
 * Initial state for the token store
 */
const initialState: TokenState = {
	streaming: { input: 0, output: 0, cached: null, cacheWrite: null, speed: null },
	cumulative: { input: 0, output: 0, cost: 0, cached: null, cacheWrite: null },
	subAgent: { input: 0, output: 0 },
	contextUsed: 0,
	contextMax: 128000,
	inputPrice: 0,
	outputPrice: 0,
	cacheReadPrice: 0,
	cacheWritePrice: 0,
	isStreaming: false,
	streamStartTime: null,
	sessionCost: null
};

/**
 * Internal writable store
 */
const store = writable<TokenState>(initialState);

/**
 * Token store with actions for managing token usage and cost tracking.
 * Tracks both streaming (current session) and cumulative (workflow lifetime) metrics.
 */
export const tokenStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Update token data from a workflow (cumulative values).
	 * Used when loading a workflow to restore cumulative metrics.
	 *
	 * @param workflow - The workflow containing cumulative token data
	 */
	updateFromWorkflow(workflow: Workflow): void {
		store.update((s) => ({
			...s,
			cumulative: {
				input: workflow.total_tokens_input ?? 0,
				output: workflow.total_tokens_output ?? 0,
				cost: workflow.total_cost_usd ?? 0,
				cached: workflow.total_cached_tokens ?? null,
				cacheWrite: workflow.total_cache_write_tokens ?? null
			},
			subAgent: {
				input: workflow.sub_agent_tokens_input ?? 0,
				output: workflow.sub_agent_tokens_output ?? 0
			},
			contextUsed: workflow.current_context_tokens ?? 0
		}));
	},

	/**
	 * Update pricing and context info from model configuration.
	 * Used when selecting a model to update cost calculations and context limits.
	 *
	 * @param model - The LLM model configuration
	 */
	updateFromModel(model: LLMModel): void {
		store.update((s) => ({
			...s,
			contextMax: model.context_window ?? 128000,
			inputPrice: model.input_price_per_mtok ?? 0,
			outputPrice: model.output_price_per_mtok ?? 0,
			cacheReadPrice: model.cache_read_price_per_mtok ?? 0,
			cacheWritePrice: model.cache_write_price_per_mtok ?? 0
		}));
	},

	/**
	 * Start streaming mode.
	 * Resets streaming tokens and records start time for speed calculation.
	 */
	startStreaming(): void {
		store.update((s) => ({
			...s,
			streaming: { input: 0, output: 0, cached: null, cacheWrite: null, speed: null },
			isStreaming: true,
			streamStartTime: Date.now(),
			sessionCost: null
		}));
	},

	/**
	 * Set session tokens from a response_block event.
	 * Sets both input and output tokens at once without speed calculation.
	 * Used with the block-by-block execution model.
	 *
	 * @param tokensIn - Input tokens consumed
	 * @param tokensOut - Output tokens generated
	 * @param cachedTokens - Cached input tokens (if reported by provider)
	 * @param cacheWriteTokens - Cache-write tokens (if reported by provider)
	 */
	setSessionTokens(tokensIn: number, tokensOut: number, cachedTokens?: number, cacheWriteTokens?: number): void {
		store.update((s) => ({
			...s,
			streaming: {
				input: tokensIn,
				output: tokensOut,
				cached: cachedTokens ?? null,
				cacheWrite: cacheWriteTokens ?? null,
				speed: null
			}
		}));
	},

	/**
	 * Stop streaming mode.
	 * Clears streaming state but preserves metrics.
	 */
	stopStreaming(): void {
		store.update((s) => ({
			...s,
			isStreaming: false,
			streamStartTime: null
		}));
	},

	/**
	 * Set session cost directly from backend result.
	 * Used when the backend has already calculated the cost.
	 *
	 * @param costUsd - Cost in USD from WorkflowMetrics
	 */
	setSessionCost(costUsd: number): void {
		store.update((s) => ({
			...s,
			sessionCost: costUsd
		}));
	},

	/**
	 * Reset to initial state.
	 * Clears all token metrics and streaming state.
	 */
	reset(): void {
		store.set(initialState);
	}
};

/**
 * Derived store: complete token display data for TokenDisplay component.
 *
 * Combines streaming and cumulative metrics with cost calculations.
 */
export const tokenDisplayData = derived(store, ($s): TokenDisplayData => {
	// Determine if there's an active session (has streaming tokens or explicit session cost)
	const hasActiveSession = $s.sessionCost !== null || $s.streaming.input > 0 || $s.streaming.output > 0;

	// Use backend-calculated session cost if available, otherwise calculate from prices
	const calculatedCost =
		($s.streaming.input * $s.inputPrice) / 1_000_000 +
		($s.streaming.output * $s.outputPrice) / 1_000_000;

	// If no active session, show cumulative cost as main cost (avoids misleading "Free")
	// If active session, show session cost (backend value preferred)
	const displayCost = hasActiveSession
		? ($s.sessionCost ?? calculatedCost)
		: $s.cumulative.cost;

	// Estimate sub-agent cost using main agent pricing (approximation)
	const subAgentCost =
		($s.subAgent.input * $s.inputPrice) / 1_000_000 +
		($s.subAgent.output * $s.outputPrice) / 1_000_000;

	return {
		tokens_input: $s.streaming.input,
		tokens_output: $s.streaming.output,
		cumulative_input: $s.cumulative.input,
		cumulative_output: $s.cumulative.output,
		context_max: $s.contextMax,
		cost_usd: displayCost,
		cumulative_cost_usd: $s.cumulative.cost,
		sub_agent_input: $s.subAgent.input,
		sub_agent_output: $s.subAgent.output,
		cached_tokens: $s.streaming.cached ?? undefined,
		cumulative_cached: $s.cumulative.cached ?? undefined,
		cache_write_tokens: $s.streaming.cacheWrite ?? undefined,
		cumulative_cache_write: $s.cumulative.cacheWrite ?? undefined,
		workflow_total_cost: $s.cumulative.cost + subAgentCost,
		speed_tks: $s.streaming.speed ?? undefined,
		is_streaming: $s.isStreaming,
		context_used: $s.contextUsed,
		cache_hit_rate:
			$s.streaming.cached && $s.streaming.input > 0
				? Math.round(($s.streaming.cached / $s.streaming.input) * 100)
				: null
	};
});

/**
 * Derived store: streaming token metrics
 */
export const streamingTokens = derived(store, ($s) => $s.streaming);

/**
 * Derived store: cumulative token metrics
 */
export const cumulativeTokens = derived(store, ($s) => $s.cumulative);
