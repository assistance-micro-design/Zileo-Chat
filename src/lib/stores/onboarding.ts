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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Onboarding store for first-launch wizard state management.
 * Uses localStorage for persistence (no backend required).
 * @module stores/onboarding
 */

import { writable, derived } from 'svelte/store';
import type { OnboardingState } from '$types/onboarding';
import {
	TOTAL_STEPS,
	INITIAL_ONBOARDING_STATE,
	ONBOARDING_STORAGE_KEY
} from '$types/onboarding';

/**
 * Creates the onboarding store with localStorage persistence
 */
function createOnboardingStore() {
	const { subscribe, set, update } = writable<OnboardingState>(INITIAL_ONBOARDING_STATE);

	return {
		subscribe,

		/**
		 * Check if onboarding should be shown (first launch)
		 * @returns true if onboarding has not been completed
		 */
		shouldShow: (): boolean => {
			if (typeof localStorage === 'undefined') return false;
			return localStorage.getItem(ONBOARDING_STORAGE_KEY) !== 'true';
		},

		/**
		 * Go to next step
		 */
		nextStep: (): void => {
			update((s) => ({
				...s,
				currentStep: Math.min(s.currentStep + 1, TOTAL_STEPS - 1),
				error: null
			}));
		},

		/**
		 * Go to previous step
		 */
		prevStep: (): void => {
			update((s) => ({
				...s,
				currentStep: Math.max(s.currentStep - 1, 0),
				error: null
			}));
		},

		/**
		 * Skip to last step (completion)
		 */
		skipToEnd: (): void => {
			update((s) => ({
				...s,
				currentStep: TOTAL_STEPS - 1,
				skipped: true,
				error: null
			}));
		},

		/**
		 * Go to a specific step by index
		 * @param stepIndex - Step index (0 to TOTAL_STEPS - 1)
		 */
		goToStep: (stepIndex: number): void => {
			if (stepIndex < 0 || stepIndex >= TOTAL_STEPS) return;
			update((s) => ({
				...s,
				currentStep: stepIndex,
				error: null
			}));
		},

		/**
		 * Mark onboarding as complete and persist to localStorage
		 */
		markComplete: (): void => {
			if (typeof localStorage !== 'undefined') {
				localStorage.setItem(ONBOARDING_STORAGE_KEY, 'true');
			}
			update((s) => ({ ...s, completed: true }));
		},

		/**
		 * Set API key validation result
		 * @param valid - Whether the API key is valid
		 */
		setApiKeyValid: (valid: boolean): void => {
			update((s) => ({ ...s, apiKeyValid: valid }));
		},

		/**
		 * Set loading state
		 * @param loading - Whether an async operation is in progress
		 */
		setLoading: (loading: boolean): void => {
			update((s) => ({ ...s, loading }));
		},

		/**
		 * Set error message
		 * @param error - Error message or null to clear
		 */
		setError: (error: string | null): void => {
			update((s) => ({ ...s, error }));
		},

		/**
		 * Reset store to initial state (for testing)
		 */
		reset: (): void => {
			if (typeof localStorage !== 'undefined') {
				localStorage.removeItem(ONBOARDING_STORAGE_KEY);
			}
			set(INITIAL_ONBOARDING_STATE);
		}
	};
}

/**
 * Onboarding store instance
 */
export const onboardingStore = createOnboardingStore();

// Derived stores for reactive UI updates
// Prefixed with 'onboarding' to avoid conflicts with other stores
export const currentStep = derived(onboardingStore, ($s) => $s.currentStep);
export const onboardingCompleted = derived(onboardingStore, ($s) => $s.completed);
export const onboardingSkipped = derived(onboardingStore, ($s) => $s.skipped);
export const onboardingLoading = derived(onboardingStore, ($s) => $s.loading);
export const onboardingError = derived(onboardingStore, ($s) => $s.error);
export const onboardingApiKeyValid = derived(onboardingStore, ($s) => $s.apiKeyValid);

/**
 * Progress percentage (0-100)
 */
export const progressPercent = derived(
	onboardingStore,
	($s) => (($s.currentStep + 1) / TOTAL_STEPS) * 100
);

/**
 * Whether user can go back
 */
export const canGoBack = derived(onboardingStore, ($s) => $s.currentStep > 0);

/**
 * Whether user is on last step
 */
export const isLastStep = derived(onboardingStore, ($s) => $s.currentStep === TOTAL_STEPS - 1);
