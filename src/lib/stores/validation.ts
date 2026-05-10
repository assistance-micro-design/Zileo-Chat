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
 * Validation store for managing human-in-the-loop validation requests.
 * Handles sub-agent operation validation (spawn, delegate, parallel).
 *
 * @module stores/validation
 */

import { writable, derived, get } from 'svelte/store';
import {
	tauriInvoke as invoke,
	tauriListen as listen,
	type TauriUnlistenFn as UnlistenFn
} from '$lib/tauri';
import type { ValidationRequest, RiskLevel, ValidationType } from '$types/validation';
import { getErrorMessage } from '$lib/utils/error';
import type { ValidationRequiredEvent } from '$types/sub-agent';

/**
 * Validation event names (inlined to avoid runtime resolution issues).
 *
 * The backend is the single source of truth for the timeout: when the
 * configured `timeout_seconds` elapses, it applies the configured
 * `timeout_behavior` (reject/approve/skip) and emits VALIDATION_RESOLVED
 * so the frontend can close the modal.
 */
const EVENTS = {
	VALIDATION_REQUIRED: 'validation_required',
	VALIDATION_RESOLVED: 'validation_resolved'
} as const;

/**
 * Payload of the `validation_resolved` event emitted by the backend when it
 * resolves a validation request itself (timeout). User-driven approve/reject
 * does not emit this event because it is already handled via the IPC response.
 */
interface ValidationResolvedEvent {
	validation_id: string;
	resolution: 'approved' | 'rejected' | 'skipped';
	source: 'timeout';
}

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
 * Cleanup functions for the two Tauri event listeners (required + resolved).
 */
let requiredUnlistener: UnlistenFn | null = null;
let resolvedUnlistener: UnlistenFn | null = null;

/**
 * Tracks whether the store has been initialized with event listeners
 */
let isInitialized = false;

/**
 * Converts a ValidationRequiredEvent to a ValidationRequest for the modal.
 */
function convertToValidationRequest(event: ValidationRequiredEvent): ValidationRequest {
	return {
		id: event.validation_id,
		workflow_id: event.workflow_id,
		type: (event.validation_type as ValidationType) ?? 'sub_agent',
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
			await this.cleanup();
		}

		// Listen for validation_required events
		requiredUnlistener = await listen<ValidationRequiredEvent>(
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

		// Listen for validation_resolved events: backend resolved the request
		// itself (e.g. timeout). Drop the matching pending entry so the modal
		// closes; ignore stale events for other validations.
		resolvedUnlistener = await listen<ValidationResolvedEvent>(
			EVENTS.VALIDATION_RESOLVED,
			(event) => {
				const { validation_id } = event.payload;
				const state = get(store);
				if (state.pending?.event.validation_id !== validation_id) {
					return;
				}

				store.update((s) => ({
					...s,
					pending: null,
					isProcessing: false,
					totalProcessed: s.totalProcessed + 1
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
			const errorMessage = getErrorMessage(error);
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
			const errorMessage = getErrorMessage(error);
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
		if (requiredUnlistener) {
			requiredUnlistener();
			requiredUnlistener = null;
		}
		if (resolvedUnlistener) {
			resolvedUnlistener();
			resolvedUnlistener = null;
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
