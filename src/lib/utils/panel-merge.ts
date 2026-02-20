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
 * @fileoverview Utility functions for merging active (streaming) and
 * persisted (database) panel data into unified display lists.
 *
 * Extracted from ReasoningPanel and ToolExecutionPanel derivations
 * (SA-011 M-003, M-004) for maintainability and testability.
 *
 * @module utils/panel-merge
 */

import type { ThinkingStep, ActiveThinkingStep } from '$types/thinking';
import type { ToolExecution, WorkflowToolExecution, ToolExecutionStatus } from '$types/tool';
import { getToolTypeDisplay, getToolIdentifier } from '$types/tool';
import type { ActiveTool } from '$lib/stores/streaming';

// ============================================================================
// Reasoning Steps (M-003)
// ============================================================================

/**
 * Unified reasoning step for display (combines active + persisted)
 */
export interface DisplayReasoningStep {
	id: string;
	stepNumber: number;
	content: string;
	durationMs?: number;
	tokens?: number;
	isActive: boolean;
	timestamp?: number;
}

/**
 * Merges active (streaming) and persisted (database) reasoning steps
 * into a sorted display list.
 *
 * @param activeSteps - Real-time steps from streaming
 * @param persistedSteps - Steps loaded from database
 * @returns Sorted array of unified display steps
 */
export function mergeAndSortReasoningSteps(
	activeSteps: ActiveThinkingStep[],
	persistedSteps: ThinkingStep[]
): DisplayReasoningStep[] {
	const items: DisplayReasoningStep[] = [];

	for (const step of activeSteps) {
		items.push({
			id: `active-${step.stepNumber}-${step.timestamp}`,
			stepNumber: step.stepNumber,
			content: step.content,
			durationMs: step.durationMs,
			isActive: true,
			timestamp: step.timestamp
		});
	}

	for (const step of persistedSteps) {
		items.push({
			id: step.id,
			stepNumber: step.step_number + 1,
			content: step.content,
			durationMs: step.duration_ms,
			tokens: step.tokens,
			isActive: false
		});
	}

	items.sort((a, b) => a.stepNumber - b.stepNumber);

	return items;
}

// ============================================================================
// Tool Executions (M-004)
// ============================================================================

/**
 * Unified tool execution for display (combines active + workflow + persisted)
 */
export interface DisplayToolExecution {
	id: string;
	name: string;
	type: string;
	serverName?: string;
	status: ToolExecutionStatus;
	duration?: number;
	error?: string;
	iteration: number;
	isActive: boolean;
}

/**
 * Converts a boolean success flag to a ToolExecutionStatus.
 */
function getHistoricalStatus(success: boolean): ToolExecutionStatus {
	return success ? 'completed' : 'error';
}

/**
 * Merges active (streaming), workflow (current result), and persisted (database)
 * tool executions into a unified display list.
 *
 * @param activeTools - Real-time tools from streaming
 * @param workflowExecutions - Tools from current workflow result
 * @param persistedExecutions - Tools loaded from database
 * @returns Array of unified display executions
 */
export function mergeToolExecutions(
	activeTools: ActiveTool[],
	workflowExecutions: WorkflowToolExecution[],
	persistedExecutions: ToolExecution[]
): DisplayToolExecution[] {
	const items: DisplayToolExecution[] = [];

	for (const tool of activeTools) {
		items.push({
			id: `active-${tool.name}-${tool.startedAt}`,
			name: tool.name,
			type: 'unknown',
			status: tool.status,
			duration: tool.duration,
			error: tool.error,
			iteration: 0,
			isActive: true
		});
	}

	for (let i = 0; i < workflowExecutions.length; i++) {
		const exec = workflowExecutions[i];
		items.push({
			id: `workflow-${i}`,
			name: getToolIdentifier(exec),
			type: getToolTypeDisplay(exec.tool_type as 'local' | 'mcp'),
			serverName: exec.server_name,
			status: getHistoricalStatus(exec.success),
			duration: exec.duration_ms,
			error: exec.error_message,
			iteration: exec.iteration,
			isActive: false
		});
	}

	for (const exec of persistedExecutions) {
		items.push({
			id: exec.id,
			name: getToolIdentifier(exec),
			type: getToolTypeDisplay(exec.tool_type),
			serverName: exec.server_name,
			status: getHistoricalStatus(exec.success),
			duration: exec.duration_ms,
			error: exec.error_message,
			iteration: exec.iteration,
			isActive: false
		});
	}

	return items;
}
