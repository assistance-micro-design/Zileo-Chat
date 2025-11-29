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
import type { ActiveTool, ActiveSubAgent, ActiveReasoningStep } from '$lib/stores/streaming';
import type { ToolExecution } from '$types/tool';

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
	index: number
): WorkflowActivityEvent {
	return {
		id: `reasoning-${step.timestamp}-${index}`,
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
 */
export function formatActivityDuration(ms: number | undefined): string {
	if (ms === undefined) return '-';
	if (ms < 1000) return `${ms}ms`;
	if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
	return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
}

/**
 * Combine all activity sources into a sorted timeline.
 */
export function combineActivities(
	tools: ActiveTool[],
	subAgents: ActiveSubAgent[],
	reasoning: ActiveReasoningStep[]
): WorkflowActivityEvent[] {
	const toolActivities = tools.map((t, i) => activeToolToActivity(t, i));
	const agentActivities = subAgents.map((a, i) => activeSubAgentToActivity(a, i));
	const reasoningActivities = reasoning.map((r, i) => activeReasoningToActivity(r, i));

	return [...toolActivities, ...agentActivities, ...reasoningActivities].sort(
		(a, b) => a.timestamp - b.timestamp
	);
}

/**
 * Filter activities by type.
 */
export function filterActivities(
	activities: WorkflowActivityEvent[],
	filter: 'all' | 'tools' | 'agents' | 'reasoning'
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
		reasoning: activities.filter((a) => a.type === 'reasoning').length
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
