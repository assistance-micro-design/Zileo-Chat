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

import { describe, it, expect } from 'vitest';
import {
	formatTokenCount,
	formatAbsoluteTimestamp,
	toolExecutionToActivity,
	thinkingStepToActivity,
	activeReasoningToActivity
} from '../activity';

describe('formatTokenCount', () => {
	it('formats small numbers as plain strings', () => {
		expect(formatTokenCount(0)).toBe('0');
		expect(formatTokenCount(42)).toBe('42');
		expect(formatTokenCount(999)).toBe('999');
	});

	it('formats thousands with k suffix', () => {
		expect(formatTokenCount(1000)).toBe('1.0k');
		expect(formatTokenCount(1500)).toBe('1.5k');
		expect(formatTokenCount(10000)).toBe('10.0k');
		expect(formatTokenCount(142000)).toBe('142.0k');
	});
});

describe('formatAbsoluteTimestamp', () => {
	it('returns a locale string for a timestamp', () => {
		const timestamp = new Date('2025-06-15T14:30:00Z').getTime();
		const result = formatAbsoluteTimestamp(timestamp);
		// Just verify it's a non-empty string (exact format is locale-dependent)
		expect(result).toBeTruthy();
		expect(typeof result).toBe('string');
	});
});

describe('toolExecutionToActivity', () => {
	const mockExec = {
		id: 'exec-123',
		workflow_id: 'wf-1',
		message_id: 'msg-1',
		agent_id: 'agent-1',
		tool_type: 'local' as const,
		tool_name: 'MemoryTool',
		input_params: { operation: 'search' },
		output_result: { success: true },
		success: true,
		duration_ms: 150,
		iteration: 0,
		created_at: '2025-06-15T14:30:00Z'
	};

	it('preserves executionId in metadata', () => {
		const activity = toolExecutionToActivity(mockExec, 0);
		expect(activity.metadata?.executionId).toBe('exec-123');
	});

	it('preserves messageId in metadata', () => {
		const activity = toolExecutionToActivity(mockExec, 0);
		expect(activity.metadata?.messageId).toBe('msg-1');
	});

	it('sets correct type for successful execution', () => {
		const activity = toolExecutionToActivity(mockExec, 0);
		expect(activity.type).toBe('tool_complete');
		expect(activity.status).toBe('completed');
	});

	it('sets correct type for failed execution', () => {
		const failed = { ...mockExec, success: false, error_message: 'timeout' };
		const activity = toolExecutionToActivity(failed, 0);
		expect(activity.type).toBe('tool_error');
		expect(activity.status).toBe('error');
		expect(activity.metadata?.error).toBe('timeout');
	});
});

describe('thinkingStepToActivity', () => {
	const mockStep = {
		id: 'step-123',
		workflow_id: 'wf-1',
		message_id: 'msg-1',
		agent_id: 'agent-1',
		step_number: 0,
		content: 'This is a reasoning step with detailed analysis.',
		duration_ms: 500,
		tokens: 142,
		created_at: '2025-06-15T14:30:00Z'
	};

	it('preserves full content in metadata', () => {
		const activity = thinkingStepToActivity(mockStep, 0);
		expect(activity.metadata?.content).toBe(mockStep.content);
	});

	it('preserves messageId in metadata', () => {
		const activity = thinkingStepToActivity(mockStep, 0);
		expect(activity.metadata?.messageId).toBe('msg-1');
	});

	it('maps tokens to metadata', () => {
		const activity = thinkingStepToActivity(mockStep, 0);
		expect(activity.metadata?.tokens).toEqual({ input: 0, output: 142 });
	});

	it('omits tokens when not available', () => {
		const noTokens = { ...mockStep, tokens: undefined };
		const activity = thinkingStepToActivity(noTokens, 0);
		expect(activity.metadata?.tokens).toBeUndefined();
	});

	it('truncates description to 200 chars', () => {
		const longContent = 'A'.repeat(300);
		const step = { ...mockStep, content: longContent };
		const activity = thinkingStepToActivity(step, 0);
		expect(activity.description?.length).toBeLessThanOrEqual(203); // 200 + '...'
		expect(activity.metadata?.content).toBe(longContent);
	});
});

describe('activeReasoningToActivity', () => {
	const mockStep = {
		content: 'Streaming reasoning content here.',
		timestamp: 1718457000000,
		stepNumber: 1
	};

	it('preserves full content in metadata', () => {
		const activity = activeReasoningToActivity(mockStep, 0);
		expect(activity.metadata?.content).toBe(mockStep.content);
	});

	it('sets correct step number in metadata', () => {
		const activity = activeReasoningToActivity(mockStep, 0);
		expect(activity.metadata?.stepNumber).toBe(1);
	});
});
