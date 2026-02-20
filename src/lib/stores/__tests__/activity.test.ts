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
 * Unit tests for the activity store.
 * Tests activity capture guard (SA-011 H-001 race condition fix).
 */

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';

// Mock Tauri's event API (required by streaming store dependency chain)
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockResolvedValue(() => {})
}));

// Mock Tauri's core API (required by activity service)
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn().mockResolvedValue([])
}));

import { activityStore, historicalActivities } from '../activity';
import { streamingStore } from '../streaming';

describe('activityStore', () => {
	beforeEach(() => {
		activityStore.reset();
		streamingStore.reset();
	});

	describe('captureStreamingActivities', () => {
		it('should capture streaming activities into historical', () => {
			// Setup: streaming store has tools and reasoning
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 150);
			streamingStore.addReasoning('Step 1: analyzing');

			// Act: capture
			activityStore.captureStreamingActivities('wf-1');

			// Assert: historical should have the captured activities
			const historical = get(historicalActivities);
			expect(historical.length).toBe(2); // 1 tool + 1 reasoning
		});

		it('should not duplicate capture for same workflow', () => {
			// Setup: streaming store has activities
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 150);

			// Act: capture twice for the same workflow
			activityStore.captureStreamingActivities('wf-1');
			activityStore.captureStreamingActivities('wf-1');

			// Assert: should only have 1 tool activity (not 2)
			const historical = get(historicalActivities);
			const toolActivities = historical.filter((a) => a.title === 'MemoryTool');
			expect(toolActivities.length).toBe(1);
		});

		it('should allow capture for a different workflow', () => {
			// Setup: first workflow's activities
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 100);

			// Capture for wf-1
			activityStore.captureStreamingActivities('wf-1');

			// Reset streaming, start new workflow
			streamingStore.reset();
			streamingStore.addToolStart('TodoTool');
			streamingStore.completeToolEnd('TodoTool', 200);

			// Reset activity store for new workflow context
			activityStore.reset();

			// Capture for wf-2 should work
			activityStore.captureStreamingActivities('wf-2');

			const historical = get(historicalActivities);
			const todoActivities = historical.filter((a) => a.title === 'TodoTool');
			expect(todoActivities.length).toBe(1);
		});

		it('should capture activities even when streaming has errors', () => {
			// Setup: streaming had partial activity before error
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 100);
			streamingStore.addReasoning('Partial reasoning');
			streamingStore.setError('Network timeout');

			// Act: capture (should still capture the partial activities)
			activityStore.captureStreamingActivities('wf-error');

			// Assert: partial activities preserved
			const historical = get(historicalActivities);
			expect(historical.length).toBe(2); // 1 tool + 1 reasoning
		});

		it('should capture all activity types (tools, reasoning, sub-agents, tasks)', () => {
			// Setup: populate all activity types
			streamingStore.addToolStart('MemoryTool');
			streamingStore.completeToolEnd('MemoryTool', 100);
			streamingStore.addReasoning('Reasoning step');
			streamingStore.processChunkDirect({
				workflow_id: 'wf-1',
				chunk_type: 'sub_agent_start',
				sub_agent_id: 'sub-1',
				sub_agent_name: 'SubAgent1',
				parent_agent_id: 'parent-1',
				content: 'Sub task'
			});
			streamingStore.processChunkDirect({
				workflow_id: 'wf-1',
				chunk_type: 'task_create',
				task_id: 'task-1',
				task_name: 'Test Task',
				task_status: 'pending',
				task_priority: 3
			});

			// Act
			activityStore.captureStreamingActivities('wf-1');

			// Assert: all types captured
			const historical = get(historicalActivities);
			expect(historical.length).toBe(4); // 1 tool + 1 reasoning + 1 sub-agent + 1 task
		});

		it('should return false when capture is skipped (duplicate)', () => {
			streamingStore.addToolStart('MemoryTool');

			const first = activityStore.captureStreamingActivities('wf-1');
			const second = activityStore.captureStreamingActivities('wf-1');

			expect(first).toBe(true);
			expect(second).toBe(false);
		});

		it('should return false when streaming state is empty', () => {
			// No streaming activities to capture
			const result = activityStore.captureStreamingActivities('wf-empty');
			expect(result).toBe(false);
		});
	});

	describe('reset', () => {
		it('should clear the capture guard on reset', () => {
			streamingStore.addToolStart('MemoryTool');

			// Capture for wf-1
			activityStore.captureStreamingActivities('wf-1');

			// Reset (clears the guard)
			activityStore.reset();

			// Setup new activities
			streamingStore.reset();
			streamingStore.addToolStart('NewTool');

			// Capture again for wf-1 should work after reset
			const result = activityStore.captureStreamingActivities('wf-1');
			expect(result).toBe(true);
		});
	});
});
