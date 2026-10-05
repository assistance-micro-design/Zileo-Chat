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
 * @fileoverview Typed Tauri API client for the workflows domain.
 *
 * Covers workflows, workflow folders, tasks, messages, tool executions,
 * thinking steps, sub-agent executions, streaming execution, validation
 * (human-in-the-loop), validation audit and user questions.
 * The `Window` parameter of streaming/user-question commands is injected
 * by Tauri and therefore omitted from these signatures.
 *
 * @module lib/api/workflows
 */

import { invokeCommand } from './internal';
import type {
	BatchDeleteResult,
	PersistedTask,
	TaskUpdate,
	Workflow,
	WorkflowFolder,
	WorkflowFullState,
	WorkflowResult
} from '$types/workflow';
import type {
	ImageReadResult,
	Message,
	MessageAttachment,
	MessageMetrics,
	PaginatedMessages
} from '$types/message';
import type { ChatBlock } from '$types/chat-block';
import type { ToolExecution } from '$types/tool';
import type { ThinkingStep } from '$types/thinking';
import type { SubAgentExecution } from '$types/sub-agent';
import type { UserQuestion } from '$types/user-question';
import type {
	AuditFilter,
	AuditStats,
	ListAuditParams,
	RiskLevel,
	UpdateValidationSettingsRequest,
	ValidationAuditEntry,
	ValidationRequest,
	ValidationSettings,
	ValidationType
} from '$types/validation';

// ============================================================================
// Workflows
// ============================================================================

/** List all workflows. */
export async function loadWorkflows(): Promise<Workflow[]> {
	return invokeCommand<Workflow[]>('load_workflows');
}

/** Create a workflow, returning its ID. */
export async function createWorkflow(name: string, agentId: string): Promise<string> {
	return invokeCommand<string>('create_workflow', { name, agentId });
}

/** Rename a workflow. */
export async function renameWorkflow(workflowId: string, name: string): Promise<Workflow> {
	return invokeCommand<Workflow>('rename_workflow', { workflowId, name });
}

/** Delete a workflow. */
export async function deleteWorkflow(workflowId: string): Promise<void> {
	return invokeCommand<void>('delete_workflow', { workflowId });
}

/** Delete several workflows at once (running ones are skipped). */
export async function deleteWorkflowsBatch(workflowIds: string[]): Promise<BatchDeleteResult> {
	return invokeCommand<BatchDeleteResult>('delete_workflows_batch', { workflowIds });
}

/** Load the complete state of a workflow (recovery). */
export async function loadWorkflowFullState(workflowId: string): Promise<WorkflowFullState> {
	return invokeCommand<WorkflowFullState>('load_workflow_full_state', { workflowId });
}

/** Move a workflow to a folder (`null` folder unfiles it). */
export async function moveWorkflowToFolder(
	workflowId: string,
	folderId?: string | null
): Promise<Workflow> {
	return invokeCommand<Workflow>('move_workflow_to_folder', {
		workflowId,
		folderId: folderId
	});
}

/** Move several workflows to a folder. Returns the number moved. */
export async function moveWorkflowsToFolder(
	workflowIds: string[],
	folderId?: string | null
): Promise<number> {
	return invokeCommand<number>('move_workflows_to_folder', {
		workflowIds,
		folderId: folderId
	});
}

/** Toggle the pinned state of a workflow. */
export async function toggleWorkflowPinned(workflowId: string): Promise<Workflow> {
	return invokeCommand<Workflow>('toggle_workflow_pinned', { workflowId });
}

// ============================================================================
// Workflow folders
// ============================================================================

/** List all workflow folders. */
export async function listWorkflowFolders(): Promise<WorkflowFolder[]> {
	return invokeCommand<WorkflowFolder[]>('list_workflow_folders');
}

/** Create a workflow folder. */
export async function createWorkflowFolder(name: string, color: string): Promise<WorkflowFolder> {
	return invokeCommand<WorkflowFolder>('create_workflow_folder', { name, color });
}

/** Rename a workflow folder. */
export async function renameWorkflowFolder(
	folderId: string,
	name: string
): Promise<WorkflowFolder> {
	return invokeCommand<WorkflowFolder>('rename_workflow_folder', { folderId, name });
}

/** Update a folder color. */
export async function updateFolderColor(
	folderId: string,
	color: string
): Promise<WorkflowFolder> {
	return invokeCommand<WorkflowFolder>('update_folder_color', { folderId, color });
}

/** Delete a workflow folder. */
export async function deleteWorkflowFolder(folderId: string): Promise<void> {
	return invokeCommand<void>('delete_workflow_folder', { folderId });
}

/** Reorder workflow folders. */
export async function reorderWorkflowFolders(folderIds: string[]): Promise<void> {
	return invokeCommand<void>('reorder_workflow_folders', { folderIds });
}

// ============================================================================
// Tasks
// ============================================================================

/** Input for `createTask`. */
export interface CreateTaskInput {
	workflowId: string;
	name: string;
	description: string;
	priority?: number | null;
	agentAssigned?: string | null;
	dependencies?: string[] | null;
}

/** Create a task, returning its ID. */
export async function createTask(input: CreateTaskInput): Promise<string> {
	return invokeCommand<string>('create_task', {
		workflowId: input.workflowId,
		name: input.name,
		description: input.description,
		priority: input.priority,
		agentAssigned: input.agentAssigned,
		dependencies: input.dependencies
	});
}

/** Get a single task. */
export async function getTask(taskId: string): Promise<PersistedTask> {
	return invokeCommand<PersistedTask>('get_task', { taskId });
}

/** List all tasks of a workflow. */
export async function listWorkflowTasks(workflowId: string): Promise<PersistedTask[]> {
	return invokeCommand<PersistedTask[]>('list_workflow_tasks', { workflowId });
}

/** List tasks filtered by status. */
export async function listTasksByStatus(
	status: string,
	workflowId?: string | null
): Promise<PersistedTask[]> {
	return invokeCommand<PersistedTask[]>('list_tasks_by_status', {
		status,
		workflowId: workflowId
	});
}

/** Partially update a task. */
export async function updateTask(taskId: string, updates: TaskUpdate): Promise<PersistedTask> {
	return invokeCommand<PersistedTask>('update_task', { taskId, updates });
}

/** Update a task status specifically. */
export async function updateTaskStatus(
	taskId: string,
	status: string
): Promise<PersistedTask> {
	return invokeCommand<PersistedTask>('update_task_status', { taskId, status });
}

/** Mark a task as completed with a duration. */
export async function completeTask(
	taskId: string,
	durationMs?: number | null
): Promise<PersistedTask> {
	return invokeCommand<PersistedTask>('complete_task', {
		taskId,
		durationMs: durationMs
	});
}

/** Delete a task. */
export async function deleteTask(taskId: string): Promise<void> {
	return invokeCommand<void>('delete_task', { taskId });
}

// ============================================================================
// Messages
// ============================================================================

/** Input for `saveMessage`. */
export interface SaveMessageInput {
	workflowId: string;
	role: string;
	content: string;
	tokensInput?: number | null;
	tokensOutput?: number | null;
	model?: string | null;
	provider?: string | null;
	durationMs?: number | null;
	thinkingTokens?: number | null;
	costUsd?: number | null;
	cachedTokens?: number | null;
	cacheWriteTokens?: number | null;
	modelIdUsed?: string | null;
	messageId?: string | null;
	attachments?: MessageAttachment[] | null;
}

/** Persist a message, returning its ID. */
export async function saveMessage(input: SaveMessageInput): Promise<string> {
	return invokeCommand<string>('save_message', {
		workflowId: input.workflowId,
		role: input.role,
		content: input.content,
		tokensInput: input.tokensInput,
		tokensOutput: input.tokensOutput,
		model: input.model,
		provider: input.provider,
		durationMs: input.durationMs,
		thinkingTokens: input.thinkingTokens,
		costUsd: input.costUsd,
		cachedTokens: input.cachedTokens,
		cacheWriteTokens: input.cacheWriteTokens,
		modelIdUsed: input.modelIdUsed,
		messageId: input.messageId,
		attachments: input.attachments
	});
}

/** Load all messages of a workflow. */
export async function loadWorkflowMessages(workflowId: string): Promise<Message[]> {
	return invokeCommand<Message[]>('load_workflow_messages', { workflowId });
}

/** Load messages of a workflow with pagination. */
export async function loadWorkflowMessagesPaginated(
	workflowId: string,
	limit?: number | null,
	offset?: number | null
): Promise<PaginatedMessages> {
	return invokeCommand<PaginatedMessages>('load_workflow_messages_paginated', {
		workflowId,
		limit: limit,
		offset: offset
	});
}

/** Delete a single message. */
export async function deleteMessage(messageId: string): Promise<void> {
	return invokeCommand<void>('delete_message', { messageId });
}

/** Batch-load chat blocks for every message of a workflow. */
export async function loadWorkflowBlocks(
	workflowId: string
): Promise<Record<string, ChatBlock[]>> {
	return invokeCommand<Record<string, ChatBlock[]>>('load_workflow_blocks', { workflowId });
}

/** Last assistant-message metrics of a workflow (or `null`). */
export async function getWorkflowLastAssistantMessageMetrics(
	workflowId: string
): Promise<MessageMetrics | null> {
	return invokeCommand<MessageMetrics | null>('get_workflow_last_assistant_message_metrics', {
		workflowId
	});
}

/** Read an image file for a chat attachment. */
export async function readImageForAttachment(path: string): Promise<ImageReadResult> {
	return invokeCommand<ImageReadResult>('read_image_for_attachment', { path });
}

// ============================================================================
// Tool executions
// ============================================================================

/** Input for `saveToolExecution`. */
export interface SaveToolExecutionInput {
	workflowId: string;
	messageId: string;
	agentId: string;
	toolType: string;
	toolName: string;
	serverName?: string | null;
	inputParams: unknown;
	outputResult: unknown;
	success: boolean;
	errorMessage?: string | null;
	durationMs: number;
	iteration: number;
}

/** Persist a tool execution log, returning its ID. */
export async function saveToolExecution(input: SaveToolExecutionInput): Promise<string> {
	return invokeCommand<string>('save_tool_execution', {
		workflowId: input.workflowId,
		messageId: input.messageId,
		agentId: input.agentId,
		toolType: input.toolType,
		toolName: input.toolName,
		serverName: input.serverName,
		inputParams: input.inputParams,
		outputResult: input.outputResult,
		success: input.success,
		errorMessage: input.errorMessage,
		durationMs: input.durationMs,
		iteration: input.iteration
	});
}

/** Get a single tool execution. */
export async function getToolExecution(executionId: string): Promise<ToolExecution> {
	return invokeCommand<ToolExecution>('get_tool_execution', { executionId });
}

/** Load all tool executions of a workflow. */
export async function loadWorkflowToolExecutions(workflowId: string): Promise<ToolExecution[]> {
	return invokeCommand<ToolExecution[]>('load_workflow_tool_executions', { workflowId });
}

/** Load tool executions of a message. */
export async function loadMessageToolExecutions(messageId: string): Promise<ToolExecution[]> {
	return invokeCommand<ToolExecution[]>('load_message_tool_executions', { messageId });
}

/** Delete a single tool execution. */
export async function deleteToolExecution(executionId: string): Promise<void> {
	return invokeCommand<void>('delete_tool_execution', { executionId });
}

/** Delete all tool executions of a workflow. Returns the number deleted. */
export async function clearWorkflowToolExecutions(workflowId: string): Promise<number> {
	return invokeCommand<number>('clear_workflow_tool_executions', { workflowId });
}

// ============================================================================
// Thinking steps
// ============================================================================

/** Input for `saveThinkingStep`. */
export interface SaveThinkingStepInput {
	workflowId: string;
	messageId: string;
	agentId: string;
	stepNumber: number;
	content: string;
	durationMs?: number | null;
	tokens?: number | null;
}

/** Persist a thinking step, returning its ID. */
export async function saveThinkingStep(input: SaveThinkingStepInput): Promise<string> {
	return invokeCommand<string>('save_thinking_step', {
		workflowId: input.workflowId,
		messageId: input.messageId,
		agentId: input.agentId,
		stepNumber: input.stepNumber,
		content: input.content,
		durationMs: input.durationMs,
		tokens: input.tokens
	});
}

/** Load all thinking steps of a workflow. */
export async function loadWorkflowThinkingSteps(workflowId: string): Promise<ThinkingStep[]> {
	return invokeCommand<ThinkingStep[]>('load_workflow_thinking_steps', { workflowId });
}

/** Load thinking steps of a message. */
export async function loadMessageThinkingSteps(messageId: string): Promise<ThinkingStep[]> {
	return invokeCommand<ThinkingStep[]>('load_message_thinking_steps', { messageId });
}

/** Delete a single thinking step. */
export async function deleteThinkingStep(stepId: string): Promise<void> {
	return invokeCommand<void>('delete_thinking_step', { stepId });
}

/** Delete all thinking steps of a workflow. Returns the number deleted. */
export async function clearWorkflowThinkingSteps(workflowId: string): Promise<number> {
	return invokeCommand<number>('clear_workflow_thinking_steps', { workflowId });
}

// ============================================================================
// Sub-agent executions
// ============================================================================

/** Load all sub-agent executions of a workflow. */
export async function loadWorkflowSubAgentExecutions(
	workflowId: string
): Promise<SubAgentExecution[]> {
	return invokeCommand<SubAgentExecution[]>('load_workflow_sub_agent_executions', { workflowId });
}

/** Delete all sub-agent executions of a workflow. Returns the number deleted. */
export async function clearWorkflowSubAgentExecutions(workflowId: string): Promise<number> {
	return invokeCommand<number>('clear_workflow_sub_agent_executions', { workflowId });
}

// ============================================================================
// Streaming execution
// ============================================================================

/** Input for `executeWorkflowStreaming` (`window` is injected by Tauri). */
export interface ExecuteWorkflowStreamingInput {
	workflowId: string;
	message: string;
	agentId: string;
	locale: string;
	attachments?: MessageAttachment[] | null;
}

/** Execute a workflow with real-time streaming events. */
export async function executeWorkflowStreaming(
	input: ExecuteWorkflowStreamingInput
): Promise<WorkflowResult> {
	return invokeCommand<WorkflowResult>('execute_workflow_streaming', {
		workflowId: input.workflowId,
		message: input.message,
		agentId: input.agentId,
		locale: input.locale,
		attachments: input.attachments
	});
}

/** Cancel a streaming execution. */
export async function cancelWorkflowStreaming(workflowId: string): Promise<void> {
	return invokeCommand<void>('cancel_workflow_streaming', { workflowId });
}

/** Backend boot readiness flag. */
export async function bootReadyState(): Promise<boolean> {
	return invokeCommand<boolean>('boot_ready_state');
}

// ============================================================================
// Validation (human-in-the-loop)
// ============================================================================

/** Input for `createValidationRequest`. */
export interface CreateValidationRequestInput {
	workflowId: string;
	validationType: ValidationType;
	operation: string;
	details: unknown;
	riskLevel: RiskLevel;
}

/** Create a validation request. */
export async function createValidationRequest(
	input: CreateValidationRequestInput
): Promise<ValidationRequest> {
	return invokeCommand<ValidationRequest>('create_validation_request', {
		workflowId: input.workflowId,
		validationType: input.validationType,
		operation: input.operation,
		details: input.details,
		riskLevel: input.riskLevel
	});
}

/** List pending validations. */
export async function listPendingValidations(): Promise<ValidationRequest[]> {
	return invokeCommand<ValidationRequest[]>('list_pending_validations');
}

/** List validations of a workflow. */
export async function listWorkflowValidations(workflowId: string): Promise<ValidationRequest[]> {
	return invokeCommand<ValidationRequest[]>('list_workflow_validations', { workflowId });
}

/** Approve a validation request. */
export async function approveValidation(validationId: string): Promise<void> {
	return invokeCommand<void>('approve_validation', { validationId });
}

/** Reject a validation request with a reason. */
export async function rejectValidation(validationId: string, reason: string): Promise<void> {
	return invokeCommand<void>('reject_validation', { validationId, reason });
}

/** Delete a validation request. */
export async function deleteValidation(validationId: string): Promise<void> {
	return invokeCommand<void>('delete_validation', { validationId });
}

/** Get validation settings. */
export async function getValidationSettings(): Promise<ValidationSettings> {
	return invokeCommand<ValidationSettings>('get_validation_settings');
}

/** Update validation settings. */
export async function updateValidationSettings(
	config: UpdateValidationSettingsRequest
): Promise<ValidationSettings> {
	return invokeCommand<ValidationSettings>('update_validation_settings', { config });
}

/** Reset validation settings to defaults. */
export async function resetValidationSettings(): Promise<ValidationSettings> {
	return invokeCommand<ValidationSettings>('reset_validation_settings');
}

// ============================================================================
// Validation audit
// ============================================================================

/** List validation audit entries. */
export async function listValidationAudit(
	params: ListAuditParams
): Promise<ValidationAuditEntry[]> {
	return invokeCommand<ValidationAuditEntry[]>('list_validation_audit', { params });
}

/** Get validation audit statistics. */
export async function getValidationAuditStats(): Promise<AuditStats> {
	return invokeCommand<AuditStats>('get_validation_audit_stats');
}

/** Purge the validation audit log now. Returns the number purged. */
export async function purgeValidationAuditNow(): Promise<number> {
	return invokeCommand<number>('purge_validation_audit_now');
}

/** Export the validation audit log as CSV, honoring an optional filter. */
export async function exportValidationAuditCsv(filter?: AuditFilter | null): Promise<string> {
	return invokeCommand<string>('export_validation_audit_csv', { filter: filter });
}

// ============================================================================
// User questions
// ============================================================================

/** Input for `submitUserResponse` (`window` is injected by Tauri). */
export interface SubmitUserResponseInput {
	questionId: string;
	workflowId: string;
	selectedOptions: string[];
	textResponse?: string | null;
}

/** Submit the user's answer to a pending question. */
export async function submitUserResponse(input: SubmitUserResponseInput): Promise<void> {
	return invokeCommand<void>('submit_user_response', {
		questionId: input.questionId,
		workflowId: input.workflowId,
		selectedOptions: input.selectedOptions,
		textResponse: input.textResponse
	});
}

/** Get all pending questions of a workflow. */
export async function getPendingQuestions(workflowId: string): Promise<UserQuestion[]> {
	return invokeCommand<UserQuestion[]>('get_pending_questions', { workflowId });
}

/** Skip a pending question. */
export async function skipQuestion(questionId: string, workflowId: string): Promise<void> {
	return invokeCommand<void>('skip_question', { questionId, workflowId });
}
