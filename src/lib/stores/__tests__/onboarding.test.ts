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

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { get } from 'svelte/store';
import { computeNextStep, computePrevStep, onboardingStore } from '../onboarding';
import { ONBOARDING_STEPS, ONBOARDING_STORAGE_KEY, TOTAL_STEPS } from '$types/onboarding';

const IMPORT_INDEX = ONBOARDING_STEPS.indexOf('import');
const GETTING_STARTED_INDEX = ONBOARDING_STEPS.indexOf('getting_started');
const COMPLETE_INDEX = ONBOARDING_STEPS.indexOf('complete');

describe('onboarding steps definition', () => {
	it('defines the 8-step onboarding flow in the expected order', () => {
		expect(ONBOARDING_STEPS).toEqual([
			'language',
			'theme',
			'welcome',
			'features',
			'api_key',
			'import',
			'getting_started',
			'complete'
		]);
	});

	it('derives TOTAL_STEPS from the step list', () => {
		expect(TOTAL_STEPS).toBe(8);
		expect(TOTAL_STEPS).toBe(ONBOARDING_STEPS.length);
	});
});

describe('onboardingStore', () => {
	beforeEach(() => {
		vi.restoreAllMocks();
		localStorage.clear();
		onboardingStore.reset();
	});

	it('navigates forward through every step and stops at the last one', () => {
		for (let i = 0; i < TOTAL_STEPS - 1; i++) {
			onboardingStore.nextStep();
		}
		expect(get(onboardingStore).currentStep).toBe(TOTAL_STEPS - 1);

		onboardingStore.nextStep();
		expect(get(onboardingStore).currentStep).toBe(TOTAL_STEPS - 1);
	});

	it('reads completion state from localStorage', () => {
		expect(onboardingStore.shouldShow()).toBe(true);

		localStorage.setItem(ONBOARDING_STORAGE_KEY, 'true');

		expect(onboardingStore.shouldShow()).toBe(false);
	});

	it('updates UI state even when localStorage setItem fails', () => {
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new Error('storage blocked');
		});

		expect(() => onboardingStore.markComplete()).not.toThrow();
		expect(get(onboardingStore).completed).toBe(true);
	});

	it('falls back to showing onboarding when localStorage getItem fails', () => {
		vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
			throw new Error('storage blocked');
		});

		expect(onboardingStore.shouldShow()).toBe(true);
	});

	it('resets UI state even when localStorage removeItem fails', () => {
		onboardingStore.markComplete();
		vi.spyOn(Storage.prototype, 'removeItem').mockImplementation(() => {
			throw new Error('storage blocked');
		});

		expect(() => onboardingStore.reset()).not.toThrow();
		expect(get(onboardingStore).completed).toBe(false);
	});

	it('defaults the imported flag to false', () => {
		expect(get(onboardingStore).importedDuringOnboarding).toBe(false);
	});

	it('records the imported flag via setImported', () => {
		onboardingStore.setImported(true);
		expect(get(onboardingStore).importedDuringOnboarding).toBe(true);
	});

	it('skips getting_started after a successful import when advancing', () => {
		onboardingStore.goToStep(IMPORT_INDEX);
		onboardingStore.setImported(true);

		onboardingStore.nextStep();

		expect(get(onboardingStore).currentStep).toBe(COMPLETE_INDEX);
	});

	it('keeps getting_started when no import happened', () => {
		onboardingStore.goToStep(IMPORT_INDEX);

		onboardingStore.nextStep();

		expect(get(onboardingStore).currentStep).toBe(GETTING_STARTED_INDEX);
	});

	it('skips getting_started when navigating back from complete after import', () => {
		onboardingStore.setImported(true);
		onboardingStore.goToStep(COMPLETE_INDEX);

		onboardingStore.prevStep();

		expect(get(onboardingStore).currentStep).toBe(IMPORT_INDEX);
	});
});

describe('computeNextStep / computePrevStep', () => {
	it('advances normally without import', () => {
		expect(computeNextStep(IMPORT_INDEX, false)).toBe(GETTING_STARTED_INDEX);
	});

	it('jumps over getting_started to complete with import', () => {
		expect(computeNextStep(IMPORT_INDEX, true)).toBe(COMPLETE_INDEX);
	});

	it('clamps next at the last step', () => {
		expect(computeNextStep(TOTAL_STEPS - 1, false)).toBe(TOTAL_STEPS - 1);
		expect(computeNextStep(TOTAL_STEPS - 1, true)).toBe(TOTAL_STEPS - 1);
	});

	it('goes back over getting_started to import with import', () => {
		expect(computePrevStep(COMPLETE_INDEX, true)).toBe(IMPORT_INDEX);
	});

	it('goes back to getting_started without import', () => {
		expect(computePrevStep(COMPLETE_INDEX, false)).toBe(GETTING_STARTED_INDEX);
	});

	it('clamps prev at the first step', () => {
		expect(computePrevStep(0, false)).toBe(0);
		expect(computePrevStep(0, true)).toBe(0);
	});
});
