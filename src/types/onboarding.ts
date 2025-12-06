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
