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

import { get } from 'svelte/store';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { backgroundWorkflowsStore, runningCount, canStartNew } from '../background-workflows';
import { tauriListen } from '$lib/tauri';
import type { StreamChunk } from '$types/streaming';

vi.mock('$lib/tauri');

describe('backgroundWorkflowsStore lifecycle cleanup', () => {
	beforeEach(() => {
		vi.useFakeTimers();
		vi.mocked(tauriListen).mockResolvedValue(vi.fn());
		backgroundWorkflowsStore.destroy();
	});

	afterEach(() => {
		backgroundWorkflowsStore.destroy();
		vi.useRealTimers();
		vi.clearAllMocks();
	});

	it('destroys synchronously and resets tracked executions', () => {
		backgroundWorkflowsStore.register('workflow-1', 'agent-1', 'Workflow 1');
		backgroundWorkflowsStore.setViewed('workflow-1');

		expect(get(runningCount)).toBe(1);
		expect(backgroundWorkflowsStore.getViewedWorkflowId()).toBe('workflow-1');

		const result = backgroundWorkflowsStore.destroy();

		expect(result).toBeUndefined();
		expect(get(runningCount)).toBe(0);
		expect(backgroundWorkflowsStore.getViewedWorkflowId()).toBeNull();
		expect(backgroundWorkflowsStore.getExecution('workflow-1')).toBeUndefined();
	});
});

describe('backgroundWorkflowsStore auto-register on unknown workflow', () => {
	let chunkHandler: ((event: { payload: StreamChunk }) => void) | null = null;

	beforeEach(() => {
		vi.useFakeTimers();
		chunkHandler = null;
		vi.mocked(tauriListen).mockImplementation(async (eventName, handler) => {
			if (eventName === 'workflow_stream') {
				chunkHandler = handler as (event: { payload: StreamChunk }) => void;
			}
			return () => {};
		});
		backgroundWorkflowsStore.destroy();
	});

	afterEach(() => {
		backgroundWorkflowsStore.destroy();
		vi.useRealTimers();
		vi.clearAllMocks();
	});

	/**
	 * Regression: a chunk for a workflow that was NOT registered (e.g. a
	 * backend-initiated re-run via RerunWorkerTool) was silently dropped at
	 * `if (!exec) return`. The page agent viewing that workflow would never
	 * see the live blocks. Auto-registering on first chunk fixes that: the
	 * execution becomes tracked and `onChunkForViewed` fires normally.
	 */
	it('auto-registers an unknown workflow on first chunk and applies it', async () => {
		await backgroundWorkflowsStore.init();
		expect(chunkHandler).not.toBeNull();

		const chunk: StreamChunk = {
			workflow_id: 'unknown-wf',
			chunk_type: 'reasoning',
			content: 'thinking...',
			agent_id: 'agent-xyz'
		};
		chunkHandler!({ payload: chunk });

		const exec = backgroundWorkflowsStore.getExecution('unknown-wf');
		expect(exec).toBeDefined();
		expect(exec?.agentId).toBe('agent-xyz');
		expect(exec?.status).toBe('running');
		// The chunk itself must also be applied (not just lost on register).
		expect(exec?.chunkHistory.length).toBe(1);
	});

	/**
	 * Rel-I1: a backend-initiated run (auto-registered on first chunk, e.g.
	 * `RerunWorkerTool` re-running a worker detached from the frontend executor)
	 * must NOT consume a concurrency slot. Otherwise, in non-auto validation
	 * mode (MAX_CONCURRENT_OTHER = 1), an in-flight detached re-run would make
	 * the next chat/agent turn fail the `canStart()` gate.
	 */
	it('backend-initiated runs do not consume a concurrency slot but frontend runs do', async () => {
		await backgroundWorkflowsStore.init();

		// A frontend-registered run consumes the single slot.
		backgroundWorkflowsStore.register('fe-wf', 'agent', 'Frontend WF');
		expect(backgroundWorkflowsStore.canStart()).toBe(false);
		expect(get(canStartNew)).toBe(false);
		backgroundWorkflowsStore.remove('fe-wf');

		// A backend-initiated (auto-registered) run is still tracked + running…
		const chunk: StreamChunk = {
			workflow_id: 'be-wf',
			chunk_type: 'reasoning',
			content: 'detached rerun',
			agent_id: 'agent-xyz'
		};
		chunkHandler!({ payload: chunk });
		expect(backgroundWorkflowsStore.getExecution('be-wf')?.status).toBe('running');
		// …but it must NOT block a fresh frontend turn.
		expect(backgroundWorkflowsStore.canStart()).toBe(true);
		expect(get(canStartNew)).toBe(true);
	});

	it('forwards chunks for auto-registered workflows when viewed', async () => {
		await backgroundWorkflowsStore.init();
		const forwarded: StreamChunk[] = [];
		backgroundWorkflowsStore.setForwardCallbacks(
			(c) => forwarded.push(c),
			() => {},
			() => {}
		);
		backgroundWorkflowsStore.setViewed('unknown-wf');

		const chunk: StreamChunk = {
			workflow_id: 'unknown-wf',
			chunk_type: 'reasoning',
			content: 'live block',
			agent_id: 'agent-xyz'
		};
		chunkHandler!({ payload: chunk });

		expect(forwarded.length).toBe(1);
		expect(forwarded[0]?.content).toBe('live block');
	});
});
