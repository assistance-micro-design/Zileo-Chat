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

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

type EventHandler<T> = (event: { payload: T }) => void;
const handlers = new Map<string, EventHandler<unknown>>();

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(async (eventName: string, handler: EventHandler<unknown>) => {
		handlers.set(eventName, handler);
		return () => handlers.delete(eventName);
	})
}));

vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

import { validationStore, pendingValidation } from '../validation';
import type { ValidationRequiredEvent } from '$types/sub-agent';

function emitRequired(validationId: string): void {
	const handler = handlers.get('validation_required') as
		| EventHandler<ValidationRequiredEvent>
		| undefined;
	expect(handler).toBeDefined();
	handler!({
		payload: {
			validation_id: validationId,
			workflow_id: `wf-${validationId}`,
			validation_type: 'tool',
			operation: 'do something',
			risk_level: 'low',
			details: {}
		}
	});
}

function emitResolved(validationId: string, resolution: 'approved' | 'rejected' | 'skipped'): void {
	const handler = handlers.get('validation_resolved');
	expect(handler).toBeDefined();
	handler!({
		payload: {
			validation_id: validationId,
			resolution,
			source: 'timeout'
		}
	});
}

describe('validationStore', () => {
	beforeEach(async () => {
		handlers.clear();
		await validationStore.reset();
		await validationStore.init();
	});

	it('opens a pending validation when validation_required arrives', () => {
		emitRequired('val-1');

		const pending = get(pendingValidation);
		expect(pending).not.toBeNull();
		expect(pending?.id).toBe('val-1');
	});

	it('clears the pending validation when backend emits validation_resolved (timeout)', () => {
		emitRequired('val-2');
		expect(get(pendingValidation)).not.toBeNull();

		emitResolved('val-2', 'rejected');

		expect(get(pendingValidation)).toBeNull();
		expect(validationStore.getState().totalProcessed).toBe(1);
	});

	it('ignores validation_resolved events for unrelated validation IDs', () => {
		emitRequired('val-3');

		emitResolved('some-other-id', 'rejected');

		const pending = get(pendingValidation);
		expect(pending).not.toBeNull();
		expect(pending?.id).toBe('val-3');
	});
});
