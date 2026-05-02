/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 */

import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { tokenStore, tokenDisplayData } from '../tokens';

// Mock Tauri event API (transitively imported by tokens stack).
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {})
}));

describe('tokenStore.setSessionTokens', () => {
	beforeEach(() => {
		tokenStore.reset();
		vi.useFakeTimers();
	});

	afterEach(() => {
		vi.useRealTimers();
	});

	it('updates streaming token counts during a live session', () => {
		// Reproduces the bug visible in the metrics bar: until the chunk
		// callback wires setSessionTokens during streaming, the ENTREE/SORTIE
		// gauge stays at 0/0 even while the model is generating.
		tokenStore.startStreaming();
		tokenStore.setSessionTokens(1234, 567, 100, 50);

		const data = get(tokenDisplayData);
		expect(data.tokens_input).toBe(1234);
		expect(data.tokens_output).toBe(567);
		expect(data.cached_tokens).toBe(100);
		expect(data.cache_write_tokens).toBe(50);
	});

	it('computes speed (t/s) when streaming is active and time elapsed', () => {
		// Precondition: speed regressed to permanent null after a refactor —
		// nothing was computing it. With streaming active and a known start
		// time, setSessionTokens must derive tokens_output / elapsed_seconds.
		const start = Date.now();
		vi.setSystemTime(start);
		tokenStore.startStreaming();

		// Advance 2 seconds and report 200 output tokens => 100 t/s.
		vi.setSystemTime(start + 2000);
		tokenStore.setSessionTokens(500, 200);

		const data = get(tokenDisplayData);
		expect(data.speed_tks).toBeCloseTo(100, 1);
	});

	it('leaves speed null when streaming is not active', () => {
		// Restoring a past session via setSessionTokens must NOT invent a
		// speed value (would be misleading — there's no live generation).
		// Default state has isStreaming=false / streamStartTime=null.
		tokenStore.setSessionTokens(500, 200);
		expect(get(tokenDisplayData).speed_tks).toBeUndefined();
	});

	it('leaves speed null when output tokens are zero', () => {
		// Zero output during streaming means the model hasn't produced
		// anything yet; reporting 0 t/s would be technically accurate but
		// noisy — null keeps the display clean until real data lands.
		tokenStore.startStreaming();
		tokenStore.setSessionTokens(100, 0);
		expect(get(tokenDisplayData).speed_tks).toBeUndefined();
	});

	it('handles cached/cacheWrite as null when omitted by provider', () => {
		// Mistral/Ollama don't expose cache fields; the store must accept
		// undefined args without crashing or showing 0 (which would imply
		// "definitely no cache hits", different from "unknown").
		tokenStore.startStreaming();
		tokenStore.setSessionTokens(100, 50);
		const data = get(tokenDisplayData);
		expect(data.cached_tokens).toBeUndefined();
		expect(data.cache_write_tokens).toBeUndefined();
	});
});

describe('tokenStore.setContextUsed', () => {
	beforeEach(() => tokenStore.reset());

	it('updates context_used live so the contexte gauge tracks streaming', () => {
		// Before this method existed, context_used only updated on the
		// post-completion workflow row reload — the gauge stayed at 0%
		// during the entire stream.
		tokenStore.setContextUsed(42_000);
		expect(get(tokenDisplayData).context_used).toBe(42_000);
	});
});

describe('tokenStore.setPartialSessionCost', () => {
	beforeEach(() => tokenStore.reset());

	it('marks the cost as partial during streaming', () => {
		// Option A: while iterations stream in, the displayed cost is the
		// running sum and may grow — components key off cost_is_partial to
		// render a `~` prefix.
		tokenStore.startStreaming();
		tokenStore.setPartialSessionCost(0.0234);

		const data = get(tokenDisplayData);
		expect(data.cost_usd).toBe(0.0234);
		expect(data.cost_is_partial).toBe(true);
	});

	it('clears the partial flag when the final cost arrives', () => {
		tokenStore.startStreaming();
		tokenStore.setPartialSessionCost(0.0234);
		tokenStore.setSessionCost(0.05);

		const data = get(tokenDisplayData);
		expect(data.cost_usd).toBe(0.05);
		expect(data.cost_is_partial).toBe(false);
	});

	it('clears the partial flag when set to null', () => {
		tokenStore.startStreaming();
		tokenStore.setPartialSessionCost(0.0123);
		tokenStore.setPartialSessionCost(null);

		// hasActiveSession depends on streaming.input/output OR sessionCost,
		// so once both are zeroed the cost falls back to cumulative.cost.
		const data = get(tokenDisplayData);
		expect(data.cost_is_partial).toBe(false);
	});
});
