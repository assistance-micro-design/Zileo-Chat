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
 * @fileoverview Utility functions for converting streaming data to activity events.
 * @module lib/utils/activity
 */

import type {
	WorkflowActivityEvent,
	ActivityType,
	ActivityStatus,
	ActivityMetadata
} from '$types/activity';
import type { ActiveTool, ActiveSubAgent, ActiveReasoningStep, ActiveTask } from '$lib/stores/streaming';
import type { ToolExecution } from '$types/tool';
import type { ThinkingStep } from '$types/thinking';
import type { SubAgentExecution } from '$types/sub-agent';
import type { Task } from '$types/task';
import { formatDuration as formatDurationUtil } from '$lib/utils/duration';

/**
 * Convert an ActiveTool from streaming store to WorkflowActivityEvent.
 */
export function activeToolToActivity(tool: ActiveTool, index: number): WorkflowActivityEvent {
	const status = toolStatusToActivityStatus(tool.status);
	const type = getToolActivityType(tool.status);

	return {
		id: `tool-${tool.name}-${tool.startedAt}-${index}`,
		timestamp: tool.startedAt,
		type,
		title: tool.name,
		status,
		duration: tool.duration,
		metadata: {
			toolName: tool.name,
			error: tool.error
		}
	};
}

/**
 * Convert an ActiveSubAgent from streaming store to WorkflowActivityEvent.
 */
export function activeSubAgentToActivity(
	agent: ActiveSubAgent,
	index: number
): WorkflowActivityEvent {
	const status = subAgentStatusToActivityStatus(agent.status);
	const type = getSubAgentActivityType(agent.status);

	const metadata: ActivityMetadata = {
		agentName: agent.name,
		agentId: agent.id,
		progress: agent.progress,
		error: agent.error
	};

	if (agent.metrics) {
		metadata.tokens = {
			input: agent.metrics.tokens_input,
			output: agent.metrics.tokens_output
		};
	}

	return {
		id: `agent-${agent.id}-${index}`,
		timestamp: agent.startedAt,
		type,
		title: agent.name,
		description: agent.taskDescription,
		status,
		duration: agent.duration,
		metadata
	};
}

/**
 * Convert an ActiveReasoningStep from streaming store to WorkflowActivityEvent.
 */
export function activeReasoningToActivity(
	step: ActiveReasoningStep,
	_index: number
): WorkflowActivityEvent {
	// Use stepNumber for unique ID (more reliable than timestamp+index which can collide)
	return {
		id: `reasoning-stream-${step.stepNumber}-${step.timestamp}`,
		timestamp: step.timestamp,
		type: 'reasoning',
		title: `Reasoning Step ${step.stepNumber}`,
		description: step.content.slice(0, 200) + (step.content.length > 200 ? '...' : ''),
		status: 'completed',
		metadata: {
			stepNumber: step.stepNumber
		}
	};
}

/**
 * Convert an ActiveTask from streaming store to WorkflowActivityEvent.
 */
export function activeTaskToActivity(
	task: ActiveTask,
	index: number
): WorkflowActivityEvent {
	const type = getTaskActivityType(task.status);
	const status = mapTaskStatusToActivityStatus(task.status);

	return {
		id: `task-${task.id}-${index}`,
		timestamp: task.createdAt,
		type,
		title: task.name,
		description: `Priority: ${task.priority}`,
		status,
		metadata: {
			taskId: task.id,
			priority: task.priority
		}
	};
}

/**
 * Get activity type based on task status.
 */
function getTaskActivityType(status: ActiveTask['status']): ActivityType {
	switch (status) {
		case 'completed':
			return 'task_complete';
		case 'in_progress':
			return 'task_update';
		default:
			return 'task_create';
	}
}

/**
 * Map task status to activity status.
 */
function mapTaskStatusToActivityStatus(status: ActiveTask['status']): ActivityStatus {
	switch (status) {
		case 'completed':
			return 'completed';
		case 'blocked':
			return 'error';
		case 'in_progress':
			return 'running';
		default:
			return 'pending';
	}
}

/**
 * Convert tool status to activity status.
 */
function toolStatusToActivityStatus(
	status: 'pending' | 'running' | 'completed' | 'error'
): ActivityStatus {
	return status;
}

/**
 * Get activity type based on tool status.
 */
function getToolActivityType(
	status: 'pending' | 'running' | 'completed' | 'error'
): ActivityType {
	switch (status) {
		case 'completed':
			return 'tool_complete';
		case 'error':
			return 'tool_error';
		default:
			return 'tool_start';
	}
}

/**
 * Convert sub-agent status to activity status.
 */
function subAgentStatusToActivityStatus(
	status: 'starting' | 'running' | 'completed' | 'error'
): ActivityStatus {
	switch (status) {
		case 'starting':
			return 'pending';
		case 'running':
			return 'running';
		case 'completed':
			return 'completed';
		case 'error':
			return 'error';
		default:
			return 'pending';
	}
}

/**
 * Get activity type based on sub-agent status.
 */
function getSubAgentActivityType(
	status: 'starting' | 'running' | 'completed' | 'error'
): ActivityType {
	switch (status) {
		case 'starting':
			return 'sub_agent_start';
		case 'running':
			return 'sub_agent_progress';
		case 'completed':
			return 'sub_agent_complete';
		case 'error':
			return 'sub_agent_error';
		default:
			return 'sub_agent_start';
	}
}

/**
 * Format timestamp for display (relative time).
 */
export function formatActivityTime(timestamp: number): string {
	const now = Date.now();
	const diff = now - timestamp;

	if (diff < 1000) return 'now';
	if (diff < 60000) return `${Math.floor(diff / 1000)}s ago`;
	if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
	return new Date(timestamp).toLocaleTimeString();
}

/**
 * Format duration in human-readable format.
 * @deprecated Use formatDuration from $lib/utils/duration instead
 */
export function formatActivityDuration(ms: number | undefined): string {
	return formatDurationUtil(ms);
}

/**
 * Combine all activity sources into a sorted timeline.
 */
export function combineActivities(
	tools: ActiveTool[],
	subAgents: ActiveSubAgent[],
	reasoning: ActiveReasoningStep[],
	tasks: ActiveTask[]
): WorkflowActivityEvent[] {
	const toolActivities = tools.map((t, i) => activeToolToActivity(t, i));
	const agentActivities = subAgents.map((a, i) => activeSubAgentToActivity(a, i));
	const reasoningActivities = reasoning.map((r, i) => activeReasoningToActivity(r, i));
	const taskActivities = tasks.map((t, i) => activeTaskToActivity(t, i));

	return [...toolActivities, ...agentActivities, ...reasoningActivities, ...taskActivities].sort(
		(a, b) => a.timestamp - b.timestamp
	);
}

/**
 * Filter activities by type.
 */
export function filterActivities(
	activities: WorkflowActivityEvent[],
	filter: 'all' | 'tools' | 'agents' | 'reasoning' | 'todos'
): WorkflowActivityEvent[] {
	if (filter === 'all') return activities;

	return activities.filter((a) => {
		switch (filter) {
			case 'tools':
				return a.type.startsWith('tool_');
			case 'agents':
				return a.type.startsWith('sub_agent_');
			case 'reasoning':
				return a.type === 'reasoning';
			case 'todos':
				return a.type.startsWith('task_');
			default:
				return true;
		}
	});
}

/**
 * Count activities by type.
 */
export function countActivitiesByType(activities: WorkflowActivityEvent[]): Record<string, number> {
	return {
		all: activities.length,
		tools: activities.filter((a) => a.type.startsWith('tool_')).length,
		agents: activities.filter((a) => a.type.startsWith('sub_agent_')).length,
		reasoning: activities.filter((a) => a.type === 'reasoning').length,
		todos: activities.filter((a) => a.type.startsWith('task_')).length
	};
}

/**
 * Convert a persisted ToolExecution to WorkflowActivityEvent.
 * Used for restoring historical activities from database.
 */
export function toolExecutionToActivity(exec: ToolExecution, index: number): WorkflowActivityEvent {
	const type: ActivityType = exec.success ? 'tool_complete' : 'tool_error';
	const status: ActivityStatus = exec.success ? 'completed' : 'error';

	const metadata: ActivityMetadata = {
		toolName: exec.tool_name,
		iteration: exec.iteration
	};

	if (exec.server_name) {
		metadata.serverName = exec.server_name;
	}

	if (exec.error_message) {
		metadata.error = exec.error_message;
	}

	return {
		id: `tool-hist-${exec.id}-${index}`,
		timestamp: new Date(exec.created_at).getTime(),
		type,
		title: exec.tool_name,
		status,
		duration: exec.duration_ms,
		metadata
	};
}

/**
 * Convert an array of persisted ToolExecutions to WorkflowActivityEvents.
 * Used for restoring historical activities when loading a workflow.
 */
export function convertToolExecutions(executions: ToolExecution[]): WorkflowActivityEvent[] {
	return executions
		.map((e, i) => toolExecutionToActivity(e, i))
		.sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * Convert a persisted ThinkingStep to WorkflowActivityEvent.
 * Used for restoring historical reasoning activities from database.
 */
export function thinkingStepToActivity(step: ThinkingStep, index: number): WorkflowActivityEvent {
	return {
		id: `reasoning-hist-${step.id}-${index}`,
		timestamp: new Date(step.created_at).getTime(),
		type: 'reasoning',
		title: `Reasoning Step ${step.step_number + 1}`,
		description: step.content.slice(0, 200) + (step.content.length > 200 ? '...' : ''),
		status: 'completed',
		duration: step.duration_ms,
		metadata: {
			stepNumber: step.step_number + 1
		}
	};
}

/**
 * Convert an array of persisted ThinkingSteps to WorkflowActivityEvents.
 * Used for restoring historical reasoning activities when loading a workflow.
 */
export function convertThinkingSteps(steps: ThinkingStep[]): WorkflowActivityEvent[] {
	return steps
		.map((s, i) => thinkingStepToActivity(s, i))
		.sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * Convert a persisted Task to WorkflowActivityEvent.
 * Used for restoring historical task activities from database.
 */
export function taskToActivity(task: Task, index: number): WorkflowActivityEvent {
	const activityType: ActivityType =
		task.status === 'completed'
			? 'task_complete'
			: task.status === 'in_progress'
				? 'task_update'
				: 'task_create';

	const status = mapTaskStatusToActivityStatus(task.status);

	const metadata: ActivityMetadata = {
		taskId: task.id,
		priority: task.priority
	};

	if (task.agent_assigned) {
		metadata.agentAssigned = task.agent_assigned;
	}

	if (task.completed_at) {
		metadata.completedAt = task.completed_at;
	}

	return {
		id: `task-hist-${task.id}-${index}`,
		timestamp: new Date(task.created_at).getTime(),
		type: activityType,
		title: task.name,
		description: task.description?.substring(0, 100),
		status,
		duration: task.duration_ms,
		metadata
	};
}

/**
 * Combine multiple activity arrays into a sorted timeline.
 * Used for merging different types of historical activities.
 */
export function mergeActivities(...activityArrays: WorkflowActivityEvent[][]): WorkflowActivityEvent[] {
	return activityArrays
		.flat()
		.sort((a, b) => a.timestamp - b.timestamp);
}

/**
 * Convert a persisted SubAgentExecution to WorkflowActivityEvent.
 * Used for restoring historical sub-agent activities from database.
 */
export function subAgentExecutionToActivity(exec: SubAgentExecution, index: number): WorkflowActivityEvent {
	// Map SubAgentExecution status to ActivityType
	let type: ActivityType;
	let status: ActivityStatus;

	switch (exec.status) {
		case 'pending':
			type = 'sub_agent_start';
			status = 'pending';
			break;
		case 'running':
			type = 'sub_agent_progress';
			status = 'running';
			break;
		case 'completed':
			type = 'sub_agent_complete';
			status = 'completed';
			break;
		case 'error':
		case 'cancelled':
			type = 'sub_agent_error';
			status = 'error';
			break;
		default:
			type = 'sub_agent_start';
			status = 'pending';
	}

	const metadata: ActivityMetadata = {
		agentName: exec.sub_agent_name,
		agentId: exec.sub_agent_id
	};

	if (exec.tokens_input !== undefined && exec.tokens_output !== undefined) {
		metadata.tokens = {
			input: exec.tokens_input,
			output: exec.tokens_output
		};
	}

	if (exec.error_message) {
		metadata.error = exec.error_message;
	}

	return {
		id: `agent-hist-${exec.id}-${index}`,
		timestamp: new Date(exec.created_at).getTime(),
		type,
		title: exec.sub_agent_name,
		description: exec.task_description?.slice(0, 200) + (exec.task_description?.length > 200 ? '...' : ''),
		status,
		duration: exec.duration_ms,
		metadata
	};
}

/**
 * Convert an array of persisted SubAgentExecutions to WorkflowActivityEvents.
 * Used for restoring historical sub-agent activities when loading a workflow.
 */
export function convertSubAgentExecutions(executions: SubAgentExecution[]): WorkflowActivityEvent[] {
	return executions
		.map((e, i) => subAgentExecutionToActivity(e, i))
		.sort((a, b) => a.timestamp - b.timestamp);
}

// ============================================================================
// Streaming-to-Activity Conversion Functions (Phase B)
// ============================================================================

/**
 * Convert an ActiveTool from streaming store to WorkflowActivityEvent.
 * Used by activityStore to merge streaming activities with historical.
 *
 * NOTE: This is the same as activeToolToActivity but exported with the expected name.
 * @see activeToolToActivity
 */
export const toolToStreamingActivity = activeToolToActivity;

/**
 * Convert an ActiveReasoningStep from streaming store to WorkflowActivityEvent.
 * Used by activityStore to merge streaming activities with historical.
 *
 * NOTE: This is the same as activeReasoningToActivity but exported with the expected name.
 * @see activeReasoningToActivity
 */
export const reasoningToStreamingActivity = activeReasoningToActivity;

/**
 * Convert an ActiveSubAgent from streaming store to WorkflowActivityEvent.
 * Used by activityStore to merge streaming activities with historical.
 *
 * NOTE: This is the same as activeSubAgentToActivity but exported with the expected name.
 * @see activeSubAgentToActivity
 */
export const subAgentToStreamingActivity = activeSubAgentToActivity;

/**
 * Convert an ActiveTask from streaming store to WorkflowActivityEvent.
 * Used by activityStore to merge streaming activities with historical.
 *
 * NOTE: This is the same as activeTaskToActivity but exported with the expected name.
 * @see activeTaskToActivity
 */
export const taskToStreamingActivity = activeTaskToActivity;
