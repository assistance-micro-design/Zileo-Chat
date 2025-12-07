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
 * Validation store for managing human-in-the-loop validation requests.
 * Handles sub-agent operation validation (spawn, delegate, parallel).
 *
 * @module stores/validation
 */

import { writable, derived, get } from 'svelte/store';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { ValidationRequest, RiskLevel } from '$types/validation';
import type { ValidationRequiredEvent } from '$types/sub-agent';

/**
 * Validation event names (inlined to avoid runtime resolution issues)
 */
const EVENTS = {
	VALIDATION_REQUIRED: 'validation_required',
	VALIDATION_RESPONSE: 'validation_response'
} as const;

// ============================================================================
// Types
// ============================================================================

/**
 * Pending validation state
 */
export interface PendingValidation {
	/** Original event from backend */
	event: ValidationRequiredEvent;
	/** Converted validation request for the modal */
	request: ValidationRequest;
	/** Timestamp when received */
	receivedAt: number;
}

/**
 * Validation store state
 */
export interface ValidationState {
	/** Currently pending validation (null if none) */
	pending: PendingValidation | null;
	/** Whether validation is currently being processed */
	isProcessing: boolean;
	/** Last error message */
	lastError: string | null;
	/** Total validations processed this session */
	totalProcessed: number;
}

// ============================================================================
// Initial State
// ============================================================================

const initialState: ValidationState = {
	pending: null,
	isProcessing: false,
	lastError: null,
	totalProcessed: 0
};

// ============================================================================
// Store Implementation
// ============================================================================

/**
 * Internal writable store
 */
const store = writable<ValidationState>(initialState);

/**
 * Event listener cleanup function
 */
let unlistener: UnlistenFn | null = null;

/**
 * Tracks whether the store has been initialized with event listeners
 */
let isInitialized = false;

/**
 * Converts a ValidationRequiredEvent to a ValidationRequest for the modal.
 */
function convertToValidationRequest(event: ValidationRequiredEvent): ValidationRequest {
	// Map operation_type to ValidationType
	const typeMap: Record<string, 'sub_agent'> = {
		spawn: 'sub_agent',
		delegate: 'sub_agent',
		parallel_batch: 'sub_agent'
	};

	return {
		id: event.validation_id,
		workflow_id: event.workflow_id,
		type: typeMap[event.operation_type] ?? 'sub_agent',
		operation: event.operation,
		details: event.details,
		risk_level: event.risk_level as RiskLevel,
		status: 'pending',
		created_at: new Date()
	};
}

/**
 * Validation store with actions for managing validation requests.
 */
export const validationStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Initialize the store and start listening for validation events.
	 * Call this when the agent page mounts.
	 */
	async init(): Promise<void> {
		// Safety check: cleanup existing listener if already initialized
		if (isInitialized) {
			console.warn('[validation] Store already initialized, cleaning up first');
			await this.cleanup();
		}

		// Listen for validation_required events
		unlistener = await listen<ValidationRequiredEvent>(
			EVENTS.VALIDATION_REQUIRED,
			(event) => {
				const validationEvent = event.payload;
				const request = convertToValidationRequest(validationEvent);

				store.update((s) => ({
					...s,
					pending: {
						event: validationEvent,
						request,
						receivedAt: Date.now()
					},
					lastError: null
				}));
			}
		);

		isInitialized = true;
	},

	/**
	 * Approve the current pending validation.
	 */
	async approve(): Promise<void> {
		const state = get(store);
		if (!state.pending) {
			return;
		}

		store.update((s) => ({ ...s, isProcessing: true }));

		try {
			await invoke('approve_validation', {
				validationId: state.pending.event.validation_id
			});

			store.update((s) => ({
				...s,
				pending: null,
				isProcessing: false,
				totalProcessed: s.totalProcessed + 1
			}));
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : String(error);
			store.update((s) => ({
				...s,
				isProcessing: false,
				lastError: `Failed to approve validation: ${errorMessage}`
			}));
		}
	},

	/**
	 * Reject the current pending validation.
	 *
	 * @param reason - Optional rejection reason
	 */
	async reject(reason?: string): Promise<void> {
		const state = get(store);
		if (!state.pending) {
			return;
		}

		store.update((s) => ({ ...s, isProcessing: true }));

		try {
			await invoke('reject_validation', {
				validationId: state.pending.event.validation_id,
				reason: reason ?? 'Rejected by user'
			});

			store.update((s) => ({
				...s,
				pending: null,
				isProcessing: false,
				totalProcessed: s.totalProcessed + 1
			}));
		} catch (error) {
			const errorMessage = error instanceof Error ? error.message : String(error);
			store.update((s) => ({
				...s,
				isProcessing: false,
				lastError: `Failed to reject validation: ${errorMessage}`
			}));
		}
	},

	/**
	 * Dismiss the pending validation without action (treats as timeout).
	 */
	dismiss(): void {
		store.update((s) => ({
			...s,
			pending: null,
			isProcessing: false
		}));
	},

	/**
	 * Clear any error state.
	 */
	clearError(): void {
		store.update((s) => ({ ...s, lastError: null }));
	},

	/**
	 * Cleanup event listeners.
	 */
	async cleanup(): Promise<void> {
		if (unlistener) {
			unlistener();
			unlistener = null;
		}
		isInitialized = false;
	},

	/**
	 * Reset the store to initial state.
	 */
	async reset(): Promise<void> {
		await this.cleanup();
		store.set(initialState);
	},

	/**
	 * Get current state snapshot.
	 */
	getState(): ValidationState {
		return get(store);
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: whether there is a pending validation
 */
export const hasPendingValidation = derived(store, (s) => s.pending !== null);

/**
 * Derived store: the current pending validation request
 */
export const pendingValidation = derived(store, (s) => s.pending?.request ?? null);

/**
 * Derived store: whether validation is being processed
 */
export const isValidating = derived(store, (s) => s.isProcessing);

/**
 * Derived store: last validation error
 */
export const validationError = derived(store, (s) => s.lastError);

/**
 * Derived store: the pending validation event details
 */
export const pendingValidationDetails = derived(store, (s) => s.pending?.event ?? null);
