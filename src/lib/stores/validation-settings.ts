// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Store for managing global validation settings.
 * Provides state management for validation configuration.
 */

import { writable, derived, get } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type {
  ValidationSettings,
  UpdateValidationSettingsRequest
} from '$types/validation';

/**
 * State for the validation settings store
 */
interface ValidationSettingsState {
  /** Current settings (null if not loaded) */
  settings: ValidationSettings | null;
  /** Whether settings are being loaded */
  loading: boolean;
  /** Whether settings are being saved */
  saving: boolean;
  /** Error message if any operation failed */
  error: string | null;
}

const initialState: ValidationSettingsState = {
  settings: null,
  loading: false,
  saving: false,
  error: null
};

/**
 * Creates the validation settings store with actions
 */
function createValidationSettingsStore() {
  const store = writable<ValidationSettingsState>(initialState);

  return {
    subscribe: store.subscribe,

    /**
     * Load validation settings from backend
     */
    async loadSettings(): Promise<void> {
      store.update((s) => ({ ...s, loading: true, error: null }));
      try {
        const settings = await invoke<ValidationSettings>('get_validation_settings');
        store.update((s) => ({ ...s, settings, loading: false }));
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        store.update((s) => ({ ...s, error: errorMsg, loading: false }));
        throw err;
      }
    },

    /**
     * Update validation settings (partial update)
     * @param config - Partial configuration to update
     */
    async updateSettings(config: UpdateValidationSettingsRequest): Promise<void> {
      store.update((s) => ({ ...s, saving: true, error: null }));
      try {
        const settings = await invoke<ValidationSettings>('update_validation_settings', {
          config
        });
        store.update((s) => ({ ...s, settings, saving: false }));
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        store.update((s) => ({ ...s, error: errorMsg, saving: false }));
        throw err;
      }
    },

    /**
     * Reset settings to defaults
     */
    async resetToDefaults(): Promise<void> {
      store.update((s) => ({ ...s, saving: true, error: null }));
      try {
        const settings = await invoke<ValidationSettings>('reset_validation_settings');
        store.update((s) => ({ ...s, settings, saving: false }));
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        store.update((s) => ({ ...s, error: errorMsg, saving: false }));
        throw err;
      }
    },

    /**
     * Clear any error state
     */
    clearError(): void {
      store.update((s) => ({ ...s, error: null }));
    },

    /**
     * Get current store state synchronously
     */
    getState(): ValidationSettingsState {
      return get(store);
    }
  };
}

/** Main validation settings store instance */
export const validationSettingsStore = createValidationSettingsStore();

/** Derived store for settings only */
export const settings = derived(
  validationSettingsStore,
  ($store) => $store.settings
);

/** Derived store for loading state */
export const isLoading = derived(
  validationSettingsStore,
  ($store) => $store.loading
);

/** Derived store for saving state */
export const isSaving = derived(
  validationSettingsStore,
  ($store) => $store.saving
);

/** Derived store for error state */
export const settingsError = derived(
  validationSettingsStore,
  ($store) => $store.error
);

/** Derived store to check if settings are loaded */
export const hasSettings = derived(
  validationSettingsStore,
  ($store) => $store.settings !== null
);
