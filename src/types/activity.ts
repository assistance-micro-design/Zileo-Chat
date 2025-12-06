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
 * @fileoverview Activity types for workflow sidebar display.
 *
 * These types represent unified activity events displayed in the right sidebar
 * during workflow execution. They aggregate tool executions, sub-agent activity,
 * and reasoning steps into a single timeline.
 *
 * @module types/activity
 */

/**
 * Type of workflow activity event.
 * Maps to streaming chunk types but provides unified categorization.
 */
export type ActivityType =
	| 'tool_start'
	| 'tool_complete'
	| 'tool_error'
	| 'reasoning'
	| 'sub_agent_start'
	| 'sub_agent_progress'
	| 'sub_agent_complete'
	| 'sub_agent_error'
	| 'validation'
	| 'message'
	| 'task_create'
	| 'task_update'
	| 'task_complete';

/**
 * Status of an activity event.
 */
export type ActivityStatus = 'pending' | 'running' | 'completed' | 'error';

/**
 * Filter options for activity feed.
 */
export type ActivityFilter = 'all' | 'tools' | 'agents' | 'reasoning' | 'todos';

/**
 * Metadata specific to activity types.
 */
export interface ActivityMetadata {
	/** Tool name (for tool_* types) */
	toolName?: string;
	/** MCP server name (for MCP tools) */
	serverName?: string;
	/** Sub-agent name (for sub_agent_* types) */
	agentName?: string;
	/** Sub-agent ID */
	agentId?: string;
	/** Progress percentage 0-100 */
	progress?: number;
	/** Token counts */
	tokens?: {
		input: number;
		output: number;
	};
	/** Error message */
	error?: string;
	/** Iteration number (for tools) */
	iteration?: number;
	/** Step number (for reasoning) */
	stepNumber?: number;
	/** Task ID (for task_* types) */
	taskId?: string;
	/** Task priority (for task_* types) */
	priority?: number;
	/** Agent assigned to task (for task_* types) */
	agentAssigned?: string;
	/** Task completion timestamp (for task_* types) */
	completedAt?: string;
}

/**
 * Unified workflow activity event for sidebar display.
 */
export interface WorkflowActivityEvent {
	/** Unique identifier */
	id: string;
	/** Unix timestamp in milliseconds */
	timestamp: number;
	/** Activity type discriminator */
	type: ActivityType;
	/** Display title */
	title: string;
	/** Optional detailed description */
	description?: string;
	/** Current status */
	status: ActivityStatus;
	/** Duration in milliseconds (for completed activities) */
	duration?: number;
	/** Type-specific metadata */
	metadata?: ActivityMetadata;
}

/**
 * State for right sidebar in agent page.
 */
export interface RightSidebarState {
	/** Whether sidebar is collapsed */
	collapsed: boolean;
	/** Current activity filter */
	filter: ActivityFilter;
	/** All activity events for current workflow */
	activities: WorkflowActivityEvent[];
}

/**
 * Filter configuration for activity tabs.
 */
export interface ActivityFilterConfig {
	/** Filter ID */
	id: ActivityFilter;
	/** Display label */
	label: string;
	/** Icon name from lucide-svelte */
	icon: string;
}

/**
 * Default filter configurations.
 */
export const ACTIVITY_FILTERS: ActivityFilterConfig[] = [
	{ id: 'all', label: 'All', icon: 'Activity' },
	{ id: 'tools', label: 'Tools', icon: 'Wrench' },
	{ id: 'agents', label: 'Agents', icon: 'Bot' },
	{ id: 'reasoning', label: 'Reasoning', icon: 'Brain' },
	{ id: 'todos', label: 'Todos', icon: 'ListTodo' }
];
