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
 * @fileoverview Workflow service for encapsulating workflow-related Tauri IPC calls.
 *
 * @module lib/services/workflow
 */

import { tauriInvoke as invoke } from '$lib/tauri';
import type { Workflow, WorkflowResult, WorkflowFullState } from '$types/workflow';

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
	 * @param locale - User's selected language (e.g., "en", "fr")
	 * @returns Workflow result with metrics and report
	 */
	async executeStreaming(
		workflowId: string,
		message: string,
		agentId: string,
		locale: string
	): Promise<WorkflowResult> {
		return invoke<WorkflowResult>('execute_workflow_streaming', {
			workflowId,
			message,
			agentId,
			locale
		});
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
	 * Batch delete multiple workflows.
	 *
	 * @param workflowIds - Array of workflow IDs to delete
	 * @returns Result with deleted count and skipped running IDs
	 */
	async deleteBatch(
		workflowIds: string[]
	): Promise<{ deleted: number; skipped_running: string[] }> {
		return invoke<{ deleted: number; skipped_running: string[] }>('delete_workflows_batch', {
			workflowIds
		});
	},

	/**
	 * Move a single workflow to a folder (or remove from folder).
	 *
	 * @param workflowId - The workflow ID to move
	 * @param folderId - Target folder ID, or null to remove from folder
	 * @returns Updated workflow entity
	 */
	async moveToFolder(workflowId: string, folderId: string | null): Promise<Workflow> {
		return invoke<Workflow>('move_workflow_to_folder', { workflowId, folderId });
	},

	/**
	 * Move multiple workflows to a folder (or remove from folder).
	 *
	 * @param workflowIds - Array of workflow IDs to move
	 * @param folderId - Target folder ID, or null to remove from folder
	 * @returns Number of workflows moved
	 */
	async moveBatchToFolder(workflowIds: string[], folderId: string | null): Promise<number> {
		return invoke<number>('move_workflows_to_folder', { workflowIds, folderId });
	},

	/**
	 * Toggle the pinned state of a workflow.
	 *
	 * @param workflowId - The workflow ID to toggle
	 * @returns Updated workflow entity
	 */
	async togglePinned(workflowId: string): Promise<Workflow> {
		return invoke<Workflow>('toggle_workflow_pinned', { workflowId });
	},

	/**
	 * Get full workflow state including messages and activities.
	 *
	 * @param workflowId - Workflow ID to retrieve
	 * @returns Complete workflow state
	 */
	async getFullState(workflowId: string): Promise<WorkflowFullState> {
		return invoke<WorkflowFullState>('load_workflow_full_state', { workflowId });
	}
};
