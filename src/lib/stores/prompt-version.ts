/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * Read-only store for prompt version history (list / get / restore).
 *
 * Versions are snapshotted backend-side on every `update_prompt`, so the
 * authoritative list always lives in DB. This store is a thin caching shell
 * keyed by `promptId` to avoid re-fetching on rapid open/close cycles.
 *
 * @module stores/prompt-version
 */

import { writable, derived } from 'svelte/store';
import { tauriInvoke as invoke } from '$lib/tauri';
import { getErrorMessage } from '$lib/utils/error';
import type { PromptVersion, PromptVersionSummary } from '$types/prompt_version';

interface State {
	/** Versions cached per prompt id. */
	versionsByPrompt: Record<string, PromptVersionSummary[]>;
	loading: boolean;
	error: string | null;
}

const initial: State = {
	versionsByPrompt: {},
	loading: false,
	error: null
};

const store = writable<State>(initial);

export const promptVersionStore = {
	subscribe: store.subscribe,

	async loadVersions(promptId: string): Promise<PromptVersionSummary[]> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const versions = await invoke<PromptVersionSummary[]>('list_prompt_versions', {
				promptId
			});
			store.update((s) => ({
				...s,
				versionsByPrompt: { ...s.versionsByPrompt, [promptId]: versions },
				loading: false
			}));
			return versions;
		} catch (e) {
			store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	async getVersion(versionId: string): Promise<PromptVersion> {
		return invoke<PromptVersion>('get_prompt_version', { versionId });
	},

	async restoreVersion(promptId: string, versionId: string, editedBy = 'user'): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			await invoke('restore_prompt_version', { promptId, versionId, editedBy });
			// Refresh after restore so the new HEAD shows up.
			await this.loadVersions(promptId);
		} catch (e) {
			store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	clearError(): void {
		store.update((s) => ({ ...s, error: null }));
	},

	reset(): void {
		store.set(initial);
	}
};

export const promptVersionLoading = derived(store, (s) => s.loading);
export const promptVersionError = derived(store, (s) => s.error);
