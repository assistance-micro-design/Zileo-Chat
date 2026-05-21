/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

/**
 * Read-only store for skill version history (list / get / restore).
 *
 * @module stores/skill-version
 */

import { writable, derived } from 'svelte/store';
import { tauriInvoke as invoke } from '$lib/tauri';
import { getErrorMessage } from '$lib/utils/error';
import type { SkillVersion, SkillVersionSummary } from '$types/skill_version';

interface State {
	versionsBySkill: Record<string, SkillVersionSummary[]>;
	loading: boolean;
	error: string | null;
}

const initial: State = {
	versionsBySkill: {},
	loading: false,
	error: null
};

const store = writable<State>(initial);

export const skillVersionStore = {
	subscribe: store.subscribe,

	async loadVersions(skillId: string): Promise<SkillVersionSummary[]> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const versions = await invoke<SkillVersionSummary[]>('list_skill_versions', { skillId });
			store.update((s) => ({
				...s,
				versionsBySkill: { ...s.versionsBySkill, [skillId]: versions },
				loading: false
			}));
			return versions;
		} catch (e) {
			store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	async getVersion(versionId: string): Promise<SkillVersion> {
		return invoke<SkillVersion>('get_skill_version', { versionId });
	},

	async restoreVersion(skillId: string, versionId: string, editedBy = 'user'): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			await invoke('restore_skill_version', { skillId, versionId, editedBy });
			await this.loadVersions(skillId);
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

export const skillVersionLoading = derived(store, (s) => s.loading);
export const skillVersionError = derived(store, (s) => s.error);
