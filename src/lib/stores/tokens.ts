// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
		speed: number | null;
	};
	/** Cumulative token metrics (entire workflow) */
	cumulative: {
		input: number;
		output: number;
		cost: number;
	};
	/** Model context window size */
	contextMax: number;
	/** Input token price (per million tokens) */
	inputPrice: number;
	/** Output token price (per million tokens) */
	outputPrice: number;
	/** Whether streaming is currently active */
	isStreaming: boolean;
	/** Timestamp when streaming started */
	streamStartTime: number | null;
}

/**
 * Initial state for the token store
 */
const initialState: TokenState = {
	streaming: { input: 0, output: 0, speed: null },
	cumulative: { input: 0, output: 0, cost: 0 },
	contextMax: 128000,
	inputPrice: 0,
	outputPrice: 0,
	isStreaming: false,
	streamStartTime: null
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
				cost: workflow.total_cost_usd ?? 0
			}
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
			outputPrice: model.output_price_per_mtok ?? 0
		}));
	},

	/**
	 * Start streaming mode.
	 * Resets streaming tokens and records start time for speed calculation.
	 */
	startStreaming(): void {
		store.update((s) => ({
			...s,
			streaming: { input: 0, output: 0, speed: null },
			isStreaming: true,
			streamStartTime: Date.now()
		}));
	},

	/**
	 * Update streaming output tokens and calculate speed.
	 * Should be called each time new tokens are received during streaming.
	 *
	 * @param tokensOut - Total output tokens received so far
	 */
	updateStreamingTokens(tokensOut: number): void {
		store.update((s) => {
			const elapsed = s.streamStartTime ? (Date.now() - s.streamStartTime) / 1000 : 1;
			const speed = elapsed > 0 ? tokensOut / elapsed : null;

			return {
				...s,
				streaming: {
					...s.streaming,
					output: tokensOut,
					speed
				}
			};
		});
	},

	/**
	 * Set input tokens (from prompt).
	 * Should be called when the prompt is sent.
	 *
	 * @param tokensIn - Number of input tokens in the prompt
	 */
	setInputTokens(tokensIn: number): void {
		store.update((s) => ({
			...s,
			streaming: { ...s.streaming, input: tokensIn }
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
	 * Reset to initial state.
	 * Clears all token metrics and streaming state.
	 */
	reset(): void {
		store.set(initialState);
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: complete token display data for TokenDisplay component.
 *
 * Combines streaming and cumulative metrics with cost calculations.
 */
export const tokenDisplayData = derived(store, ($s): TokenDisplayData => {
	// Calculate current session cost
	const sessionCost =
		($s.streaming.input * $s.inputPrice) / 1_000_000 +
		($s.streaming.output * $s.outputPrice) / 1_000_000;

	return {
		tokens_input: $s.streaming.input,
		tokens_output: $s.streaming.output,
		cumulative_input: $s.cumulative.input,
		cumulative_output: $s.cumulative.output,
		context_max: $s.contextMax,
		cost_usd: sessionCost,
		cumulative_cost_usd: $s.cumulative.cost,
		speed_tks: $s.streaming.speed ?? undefined,
		is_streaming: $s.isStreaming
	};
});

/**
 * Derived store: whether streaming is active (token tracking perspective)
 * NOTE: Use streamingStore.isStreaming for general streaming state
 */
export const isTokenStreaming = derived(store, ($s) => $s.isStreaming);

/**
 * Derived store: streaming token metrics
 */
export const streamingTokens = derived(store, ($s) => $s.streaming);

/**
 * Derived store: cumulative token metrics
 */
export const cumulativeTokens = derived(store, ($s) => $s.cumulative);
