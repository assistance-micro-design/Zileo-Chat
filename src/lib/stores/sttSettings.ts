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
 * @fileoverview Store wrapping the `settings:stt` blob.
 * Mirrors `validation-settings.ts` — same load/update/reset shape.
 */

import { writable, derived, get } from 'svelte/store';
import { tauriInvoke as invoke } from '$lib/tauri';
import type { STTSettings, UpdateSTTSettingsRequest } from '$types/stt';
import { getErrorMessage } from '$lib/utils/error';

interface STTSettingsState {
	settings: STTSettings | null;
	loading: boolean;
	saving: boolean;
	error: string | null;
}

const initialState: STTSettingsState = {
	settings: null,
	loading: false,
	saving: false,
	error: null
};

function createSTTSettingsStore() {
	const store = writable<STTSettingsState>(initialState);

	return {
		subscribe: store.subscribe,

		async loadSettings(): Promise<void> {
			store.update((s) => ({ ...s, loading: true, error: null }));
			try {
				const settings = await invoke<STTSettings>('get_stt_settings');
				store.update((s) => ({ ...s, settings, loading: false }));
			} catch (err) {
				const errorMsg = getErrorMessage(err);
				store.update((s) => ({ ...s, error: errorMsg, loading: false }));
				throw err;
			}
		},

		async updateSettings(config: UpdateSTTSettingsRequest): Promise<void> {
			store.update((s) => ({ ...s, saving: true, error: null }));
			try {
				const settings = await invoke<STTSettings>('update_stt_settings', { config });
				store.update((s) => ({ ...s, settings, saving: false }));
			} catch (err) {
				const errorMsg = getErrorMessage(err);
				store.update((s) => ({ ...s, error: errorMsg, saving: false }));
				throw err;
			}
		},

		async resetToDefaults(): Promise<void> {
			store.update((s) => ({ ...s, saving: true, error: null }));
			try {
				const settings = await invoke<STTSettings>('reset_stt_settings');
				store.update((s) => ({ ...s, settings, saving: false }));
			} catch (err) {
				const errorMsg = getErrorMessage(err);
				store.update((s) => ({ ...s, error: errorMsg, saving: false }));
				throw err;
			}
		},

		clearError(): void {
			store.update((s) => ({ ...s, error: null }));
		},

		getState(): STTSettingsState {
			return get(store);
		}
	};
}

export const sttSettingsStore = createSTTSettingsStore();
export const sttSettings = derived(sttSettingsStore, ($s) => $s.settings);
export const sttSettingsLoading = derived(sttSettingsStore, ($s) => $s.loading);
export const sttSettingsSaving = derived(sttSettingsStore, ($s) => $s.saving);
export const sttSettingsError = derived(sttSettingsStore, ($s) => $s.error);
