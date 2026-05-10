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
import type { MessageMetrics } from '$types/message';

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
		/** Cumulative USD cost of sub-agents (computed with their OWN pricing). */
		costUsd: number;
	};
	/** Current context window usage (last API call input tokens) */
	contextUsed: number;
	/** Model context window size */
	contextMax: number;
	/** Whether streaming is currently active */
	isStreaming: boolean;
	/** Timestamp when streaming started */
	streamStartTime: number | null;
	/**
	 * Session cost from backend. `null` means "not yet provided" — the UI must
	 * render a neutral placeholder rather than invent a number.
	 */
	sessionCost: number | null;
	/**
	 * `true` when `sessionCost` is the running sum of per-iteration costs
	 * carried by `response_block` chunks (Option A): the workflow is still
	 * progressing and more iterations may bump the value. Components show a
	 * `~` prefix to make the partial nature visible. `false` when the cost
	 * is the final post-completion value.
	 */
	sessionCostInProgress: boolean;
}

/**
 * Initial state for the token store
 */
const initialState: TokenState = {
	streaming: { input: 0, output: 0, cached: null, cacheWrite: null, speed: null },
	cumulative: { input: 0, output: 0, cost: 0, cached: null, cacheWrite: null },
	subAgent: { input: 0, output: 0, costUsd: 0 },
	contextUsed: 0,
	contextMax: 0,
	isStreaming: false,
	streamStartTime: null,
	sessionCost: null,
	sessionCostInProgress: false
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
				output: workflow.sub_agent_tokens_output ?? 0,
				// Read sub-agent cost computed by backend with each sub-agent's
				// own pricing. Falls back to 0 on legacy rows that predate the column.
				costUsd: workflow.sub_agent_cost_usd ?? 0
			},
			contextUsed: workflow.current_context_tokens ?? 0
		}));
	},

	/**
	 * Restore the session display from the last assistant message of a workflow.
	 *
	 * Called by `selectWorkflow` when switching to a workflow that
	 * has no live execution running, so the UI shows "what the last run cost"
	 * rather than blank zeros (which would look like a free / fresh session).
	 *
	 * Passing `null` resets the session block but leaves cumulative untouched.
	 */
	restoreFromLastMessage(metrics: MessageMetrics | null): void {
		store.update((s) => ({
			...s,
			streaming: {
				input: metrics?.tokens_input ?? 0,
				output: metrics?.tokens_output ?? 0,
				cached: metrics?.cached_tokens ?? null,
				cacheWrite: metrics?.cache_write_tokens ?? null,
				speed: null
			},
			sessionCost: metrics?.cost_usd ?? null,
			// A persisted message is a finalised cost — never partial.
			sessionCostInProgress: false
		}));
	},

	/**
	 * Update context window size from model configuration.
	 * Used when selecting a model to update the context-usage gauge ceiling.
	 *
	 * Skips the update when `model.context_window` is falsy (0, null, or
	 * undefined). This guards against malformed DB rows (e.g. legacy custom
	 * models inserted without `context_window`, or schema drift after a
	 * migration) — without the guard, a single bad row would silently zero
	 * the gauge ceiling and the user would see "X / 0 contexte" with no
	 * way to recover until a healthy model is selected.
	 *
	 * @param model - The LLM model configuration
	 */
	updateFromModel(model: LLMModel): void {
		if (!model.context_window || model.context_window <= 0) {
			// Skip silently: an invalid context_window keeps the previous gauge
			// ceiling. The misconfigured row is fixed in Settings > LLM Models;
			// nothing actionable to do here.
			return;
		}
		store.update((s) => ({
			...s,
			contextMax: model.context_window
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
			sessionCost: null,
			sessionCostInProgress: false
		}));
	},

	/**
	 * Set session tokens from a response_block event.
	 *
	 * Computes `speed` (tokens/sec) when streaming is active and a start time
	 * was recorded by `startStreaming()`. Without this, the `t/s` indicator
	 * stays `null` forever — a regression from a prior refactor that moved
	 * the speed calculation out of an older streaming helper. Outside of
	 * streaming (e.g. restoring a previous session), speed is set to `null`
	 * since there's no in-progress generation to measure.
	 *
	 * @param tokensIn - Input tokens consumed
	 * @param tokensOut - Output tokens generated
	 * @param cachedTokens - Cached input tokens (if reported by provider)
	 * @param cacheWriteTokens - Cache-write tokens (if reported by provider)
	 */
	setSessionTokens(
		tokensIn: number,
		tokensOut: number,
		cachedTokens?: number,
		cacheWriteTokens?: number
	): void {
		store.update((s) => {
			let speed: number | null = null;
			if (s.isStreaming && s.streamStartTime !== null && tokensOut > 0) {
				const elapsedSec = (Date.now() - s.streamStartTime) / 1000;
				if (elapsedSec > 0) {
					speed = tokensOut / elapsedSec;
				}
			}
			return {
				...s,
				streaming: {
					input: tokensIn,
					output: tokensOut,
					cached: cachedTokens ?? null,
					cacheWrite: cacheWriteTokens ?? null,
					speed
				}
			};
		});
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
	 * Update the live context-window usage from a `response_block` chunk.
	 *
	 * `tokens_input` reported by the latest LLM iteration IS the context
	 * window size that call consumed, so mirroring it here makes the
	 * "contexte" gauge update during streaming instead of only after the
	 * workflow finishes (which is when `current_context_tokens` is persisted
	 * onto the workflow row).
	 */
	setContextUsed(tokensInput: number): void {
		store.update((s) => ({ ...s, contextUsed: tokensInput }));
	},

	/**
	 * Set the FINAL session cost from a completed workflow result.
	 * Always clears the in-progress flag since this value won't change again.
	 *
	 * @param costUsd - Cost in USD from WorkflowMetrics
	 */
	setSessionCost(costUsd: number): void {
		store.update((s) => ({
			...s,
			sessionCost: costUsd,
			sessionCostInProgress: false
		}));
	},

	/**
	 * Set a PARTIAL session cost from a `response_block` chunk during streaming.
	 *
	 * The backend pricing layer computes one `cost_usd` per LLM iteration; the
	 * chunkProcessor accumulates them into `partialCostUsd` on the bg execution.
	 * This method mirrors that running total to the visible `sessionCost` and
	 * marks it as still-progressing so the UI can render a `~` prefix.
	 *
	 * Backend-as-source-of-truth invariant preserved: the value comes 100%
	 * from the backend; the frontend just stores it.
	 */
	setPartialSessionCost(costUsd: number | null): void {
		store.update((s) => ({
			...s,
			sessionCost: costUsd,
			sessionCostInProgress: costUsd !== null
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
	// The frontend NEVER multiplies tokens × price. The backend is the single
	// source of truth for cost. When no session cost has been provided,
	// we fall back to the workflow's cumulative cost (so a freshly opened
	// workflow doesn't show a blank); during a live session we wait for the
	// backend value rather than inventing one.
	const hasActiveSession =
		$s.sessionCost !== null || $s.isStreaming || $s.streaming.input > 0 || $s.streaming.output > 0;

	const displayCost: number | null = hasActiveSession ? $s.sessionCost : $s.cumulative.cost;

	// Sub-agent cost comes from the workflow row, not a per-call
	// approximation with the parent's pricing.
	const subAgentCost = $s.subAgent.costUsd;

	return {
		tokens_input: $s.streaming.input,
		tokens_output: $s.streaming.output,
		cumulative_input: $s.cumulative.input,
		cumulative_output: $s.cumulative.output,
		context_max: $s.contextMax,
		cost_usd: displayCost,
		// Only mark as partial when we're actually showing the session value
		// (not when displaying the workflow's cumulative cost as fallback).
		cost_is_partial: hasActiveSession && $s.sessionCostInProgress,
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
