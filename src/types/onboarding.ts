// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Onboarding type definitions for first-launch wizard.
 * @module types/onboarding
 */

/**
 * Onboarding step identifier
 */
export type OnboardingStep =
	| 'language'
	| 'theme'
	| 'welcome'
	| 'values'
	| 'api_key'
	| 'import'
	| 'complete';

/**
 * All onboarding steps in order
 */
export const ONBOARDING_STEPS: OnboardingStep[] = [
	'language',
	'theme',
	'welcome',
	'values',
	'api_key',
	'import',
	'complete'
];

/**
 * Total number of onboarding steps
 */
export const TOTAL_STEPS = ONBOARDING_STEPS.length;

/**
 * Onboarding state managed by the store
 */
export interface OnboardingState {
	/** Current step index (0-6) */
	currentStep: number;
	/** Whether onboarding has been completed */
	completed: boolean;
	/** Whether user skipped to the end */
	skipped: boolean;
	/** API key validation result (null = not tested) */
	apiKeyValid: boolean | null;
	/** Loading state for async operations */
	loading: boolean;
	/** Error message if any operation failed */
	error: string | null;
}

/**
 * Initial onboarding state
 */
export const INITIAL_ONBOARDING_STATE: OnboardingState = {
	currentStep: 0,
	completed: false,
	skipped: false,
	apiKeyValid: null,
	loading: false,
	error: null
};

/**
 * LocalStorage key for onboarding completion
 */
export const ONBOARDING_STORAGE_KEY = 'zileo_onboarding_completed';
