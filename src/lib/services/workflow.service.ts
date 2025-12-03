// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Workflow service for encapsulating workflow-related Tauri IPC calls.
 *
 * @module lib/services/workflow
 */

import { invoke } from '@tauri-apps/api/core';
import type { Workflow, WorkflowResult, WorkflowFullState } from '$types/workflow';
import type { RestorationResult } from '$types/services';

/**
 * Service for workflow operations.
 *
 * Encapsulates all workflow-related Tauri IPC commands.
 */
export const WorkflowService = {
	/**
	 * Load all workflows from database.
	 *
	 * @returns Array of all workflows
	 */
	async loadAll(): Promise<Workflow[]> {
		return invoke<Workflow[]>('load_workflows');
	},

	/**
	 * Create a new workflow.
	 *
	 * @param name - Workflow name
	 * @param agentId - Agent ID to associate with workflow
	 * @returns ID of the created workflow
	 */
	async create(name: string, agentId: string): Promise<string> {
		return invoke<string>('create_workflow', { name, agentId });
	},

	/**
	 * Rename an existing workflow.
	 *
	 * @param workflowId - Workflow ID to rename
	 * @param name - New workflow name
	 * @returns Updated workflow entity
	 */
	async rename(workflowId: string, name: string): Promise<Workflow> {
		return invoke<Workflow>('rename_workflow', { workflowId, name });
	},

	/**
	 * Delete a workflow.
	 *
	 * @param workflowId - Workflow ID to delete
	 */
	async delete(workflowId: string): Promise<void> {
		return invoke<void>('delete_workflow', { workflowId });
	},

	/**
	 * Execute a workflow with streaming.
	 *
	 * @param workflowId - Workflow ID to execute
	 * @param message - User message to process
	 * @param agentId - Agent ID to use for execution
	 * @returns Workflow result with metrics and report
	 */
	async executeStreaming(workflowId: string, message: string, agentId: string): Promise<WorkflowResult> {
		return invoke<WorkflowResult>('execute_workflow_streaming', { workflowId, message, agentId });
	},

	/**
	 * Cancel an ongoing workflow execution.
	 *
	 * @param workflowId - Workflow ID to cancel
	 */
	async cancel(workflowId: string): Promise<void> {
		return invoke<void>('cancel_workflow_streaming', { workflowId });
	},

	/**
	 * Get full workflow state including messages and activities.
	 *
	 * @param workflowId - Workflow ID to retrieve
	 * @returns Complete workflow state
	 */
	async getFullState(workflowId: string): Promise<WorkflowFullState> {
		return invoke<WorkflowFullState>('get_workflow_full_state', { workflowId });
	},

	/**
	 * Restore a workflow state from database.
	 *
	 * @param workflowId - Workflow ID to restore
	 * @returns Restoration result with counts and success status
	 */
	async restoreState(workflowId: string): Promise<RestorationResult> {
		try {
			const state = await this.getFullState(workflowId);
			return {
				success: true,
				workflowId,
				messagesCount: state.messages?.length ?? 0,
				activitiesCount:
					(state.tool_executions?.length ?? 0) +
					(state.thinking_steps?.length ?? 0) +
					0 // sub_agent_executions not in WorkflowFullState yet
			};
		} catch (e) {
			return {
				success: false,
				workflowId,
				messagesCount: 0,
				activitiesCount: 0,
				error: String(e)
			};
		}
	}
};
