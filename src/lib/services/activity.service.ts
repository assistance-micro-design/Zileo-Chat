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
 * @fileoverview Activity service for encapsulating activity-related Tauri IPC calls.
 *
 * @module lib/services/activity
 */

import { invoke } from '@tauri-apps/api/core';
import type { ToolExecution } from '$types/tool';
import type { ThinkingStep } from '$types/thinking';
import type { SubAgentExecution } from '$types/sub-agent';
import type { Task } from '$types/task';
import type { WorkflowActivityEvent } from '$types/activity';
import {
	toolExecutionToActivity,
	thinkingStepToActivity,
	subAgentExecutionToActivity,
	taskToActivity
} from '$lib/utils/activity';

/**
 * Result of loading all activity types for a workflow.
 */
interface LoadAllResult {
	/** Tool executions */
	tools: ToolExecution[];
	/** Thinking/reasoning steps */
	thinking: ThinkingStep[];
	/** Sub-agent executions */
	subAgents: SubAgentExecution[];
	/** Tasks */
	tasks: Task[];
}

/**
 * Service for activity operations.
 *
 * Encapsulates all activity-related Tauri IPC commands and provides
 * conversion to unified WorkflowActivityEvent format.
 */
export const ActivityService = {
	/**
	 * Load tool executions for a workflow.
	 *
	 * @param workflowId - Workflow ID
	 * @returns Array of tool executions
	 */
	async loadToolExecutions(workflowId: string): Promise<ToolExecution[]> {
		try {
			return await invoke<ToolExecution[]>('load_workflow_tool_executions', { workflowId });
		} catch (e) {
			console.error('Failed to load tool executions:', e);
			return [];
		}
	},

	/**
	 * Load thinking steps for a workflow.
	 *
	 * @param workflowId - Workflow ID
	 * @returns Array of thinking steps
	 */
	async loadThinkingSteps(workflowId: string): Promise<ThinkingStep[]> {
		try {
			return await invoke<ThinkingStep[]>('load_workflow_thinking_steps', { workflowId });
		} catch (e) {
			console.error('Failed to load thinking steps:', e);
			return [];
		}
	},

	/**
	 * Load sub-agent executions for a workflow.
	 *
	 * @param workflowId - Workflow ID
	 * @returns Array of sub-agent executions
	 */
	async loadSubAgentExecutions(workflowId: string): Promise<SubAgentExecution[]> {
		try {
			return await invoke<SubAgentExecution[]>('load_workflow_sub_agent_executions', { workflowId });
		} catch (e) {
			console.error('Failed to load sub-agent executions:', e);
			return [];
		}
	},

	/**
	 * Load tasks for a workflow.
	 *
	 * @param workflowId - Workflow ID
	 * @returns Array of tasks
	 */
	async loadTasks(workflowId: string): Promise<Task[]> {
		try {
			return await invoke<Task[]>('list_workflow_tasks', { workflowId });
		} catch (e) {
			console.error('Failed to load tasks:', e);
			return [];
		}
	},

	/**
	 * Load all activity types for a workflow in parallel.
	 *
	 * @param workflowId - Workflow ID
	 * @returns Object containing all activity types
	 */
	async loadAll(workflowId: string): Promise<LoadAllResult> {
		const [tools, thinking, subAgents, tasks] = await Promise.all([
			this.loadToolExecutions(workflowId),
			this.loadThinkingSteps(workflowId),
			this.loadSubAgentExecutions(workflowId),
			this.loadTasks(workflowId)
		]);
		return { tools, thinking, subAgents, tasks };
	},

	/**
	 * Convert loaded activity data to unified activity events.
	 *
	 * @param data - Result from loadAll()
	 * @returns Array of unified activity events sorted by timestamp (most recent first)
	 */
	convertToActivities(data: LoadAllResult): WorkflowActivityEvent[] {
		const activities: WorkflowActivityEvent[] = [
			...data.tools.map((t, i) => toolExecutionToActivity(t, i)),
			...data.thinking.map((t, i) => thinkingStepToActivity(t, i)),
			...data.subAgents.map((s, i) => subAgentExecutionToActivity(s, i)),
			...data.tasks.map((t, i) => taskToActivity(t, i))
		];

		// Sort by timestamp descending (most recent first)
		return activities.sort((a, b) => b.timestamp - a.timestamp);
	}
};
