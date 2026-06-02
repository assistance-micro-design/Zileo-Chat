/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriListen } from '$lib/tauri';
import { toastStore } from '../toast';

vi.mock('$lib/tauri');
vi.mock('$lib/i18n', () => ({
	t: (key: string) => key,
	i18n: { subscribe: () => () => {} }
}));

import {
	composingStore,
	composingCount,
	canStartCompose,
	proposedDirtySeq,
	MAX_CONCURRENT_COMPOSE
} from '../kanban-compose';

type Handler = (event: { payload: unknown }) => void;

describe('composingStore', () => {
	const handlers: Record<string, Handler> = {};
	let addSpy: ReturnType<typeof vi.spyOn>;

	function fire(event: string, payload: unknown): void {
		const handler = handlers[event];
		if (!handler) throw new Error(`no handler registered for ${event}`);
		handler({ payload });
	}

	beforeEach(async () => {
		for (const key of Object.keys(handlers)) delete handlers[key];
		vi.mocked(tauriListen).mockImplementation(async (name, handler) => {
			handlers[name] = handler as unknown as Handler;
			return () => {};
		});
		addSpy = vi.spyOn(toastStore, 'add').mockImplementation(() => 'toast-id');
		composingStore.destroy();
		await composingStore.init();
	});

	afterEach(() => {
		composingStore.destroy();
		addSpy.mockRestore();
		vi.clearAllMocks();
	});

	it('registers both compose listeners on init', () => {
		expect(Object.keys(handlers).sort()).toEqual(['kanban:compose_failed', 'kanban:compose_ready']);
	});

	it('register tracks an in-flight compose (composingCount)', () => {
		composingStore.register('c1', 'hint');
		expect(get(composingCount)).toBe(1);
	});

	it('compose_ready clears the entry, toasts success and bumps proposedDirtySeq', () => {
		const before = get(proposedDirtySeq);
		composingStore.register('c1', 'hint');
		fire('kanban:compose_ready', { card_id: 'c1', title: 'My task' });

		expect(get(composingCount)).toBe(0);
		expect(get(proposedDirtySeq)).toBe(before + 1);
		expect(addSpy).toHaveBeenCalledWith(
			expect.objectContaining({ type: 'success', message: 'My task' })
		);
	});

	it('compose_failed clears the entry and toasts the error', () => {
		composingStore.register('c1', 'hint');
		fire('kanban:compose_failed', { card_id: 'c1', error: 'boom' });

		expect(get(composingCount)).toBe(0);
		expect(addSpy).toHaveBeenCalledWith(
			expect.objectContaining({ type: 'error', message: 'boom' })
		);
	});

	// m4: a `compose_ready` that races AHEAD of `register` must NOT be dropped —
	// it still toasts + bumps the dirty counter (and leaves no phantom entry).
	it('handles compose_ready that arrives before register', () => {
		const before = get(proposedDirtySeq);
		fire('kanban:compose_ready', { card_id: 'unknown', title: 'T' });

		expect(get(proposedDirtySeq)).toBe(before + 1);
		expect(get(composingCount)).toBe(0);
		expect(addSpy).toHaveBeenCalledWith(expect.objectContaining({ type: 'success' }));
	});

	it('canStartCompose flips to false once the cap is reached', () => {
		expect(get(canStartCompose)).toBe(true);
		for (let i = 0; i < MAX_CONCURRENT_COMPOSE; i++) {
			composingStore.register(`c${i}`, '');
		}
		expect(get(canStartCompose)).toBe(false);
	});
});
