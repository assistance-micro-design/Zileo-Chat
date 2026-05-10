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
 * @fileoverview Workflow executor service for orchestrating message sending and streaming.
 *
 * Extracts the 8-step handleSend orchestration logic from +page.svelte into a dedicated service.
 * This improves testability, reusability, and separation of concerns.
 *
 * Steps orchestrated:
 * 1. Save user message
 * 2. Start streaming
 * 3. Execute workflow via WorkflowService
 * 4. Update tokens
 * 5. Save assistant response
 * 6. Refresh workflows
 * 7. Return result with metrics
 *
 * @module lib/services/workflowExecutor
 */

import type { Message, SubAgentSummary } from '$types/message';
import type { Workflow, WorkflowMetrics, WorkflowResult } from '$types/workflow';
import type { ChatBlock } from '$types/chat-block';
import { MessageService } from './message.service';
import { WorkflowService } from './workflow.service';
import { generateUuid } from '$lib/utils/uuid';
import { get } from 'svelte/store';
import { tokenStore } from '$lib/stores/tokens';
import { workflowStore } from '$lib/stores/workflows';
import { backgroundWorkflowsStore } from '$lib/stores/background-workflows';
import { executionBlocksStore, executionBlocks } from '$lib/stores/execution-blocks';
import { toastStore } from '$lib/stores/toast';
import { t } from '$lib/i18n';
import { getErrorMessage } from '$lib/utils/error';

/**
 * Parameters for executing a workflow message.
 */
export interface ExecutionParams {
	/** ID of the workflow to execute */
	workflowId: string;
	/** User message content */
	message: string;
	/** ID of the agent to use */
	agentId: string;
	/** User's selected locale (e.g., "en", "fr") */
	locale: string;
}

/**
 * Result of a workflow execution.
 */
export interface ExecutionResult {
	/** Whether execution completed successfully */
	success: boolean;
	/** ID of the saved user message */
	userMessageId?: string;
	/** ID of the saved assistant message (matches backend message_id for block association) */
	assistantMessageId?: string;
	/** Error message if execution failed */
	error?: string;
	/** Execution metrics (tokens, cost, duration) */
	metrics?: WorkflowMetrics;
	/** The full workflow result */
	workflowResult?: WorkflowResult;
	/** Snapshot of execution blocks captured before reset */
	blocks?: ChatBlock[];
}

/**
 * Callbacks for execution events.
 */
export interface ExecutionCallbacks {
	/** Called when user message is created locally (for immediate UI update) */
	onUserMessage?: (message: Message) => void;
	/** Called when assistant message is received (for UI update) */
	onAssistantMessage?: (message: Message) => void;
	/** Called when an error occurs (for UI error display) */
	onError?: (message: Message) => void;
	/** Called when tokens are updated (for real-time token display) */
	onTokenUpdate?: (metrics: WorkflowMetrics) => void;
	/** Called to get the updated workflow after refresh */
	onWorkflowRefresh?: (workflow: Workflow | undefined) => void;
}

/**
 * Create a local user message for immediate UI feedback.
 *
 * @param workflowId - ID of the workflow
 * @param content - Message content
 * @returns Message object for UI display
 */
function createUserMessage(workflowId: string, content: string): Message {
	return {
		id: generateUuid(),
		workflow_id: workflowId,
		role: 'user',
		content,
		tokens: 0,
		timestamp: new Date()
	};
}

/**
 * Create a local assistant message from workflow result.
 *
 * @param workflowId - ID of the workflow
 * @param result - Workflow execution result
 * @returns Message object for UI display
 */
function createAssistantMessage(workflowId: string, result: WorkflowResult): Message {
	return {
		id: result.message_id,
		workflow_id: workflowId,
		role: 'assistant',
		content: result.response,
		tokens: result.metrics.tokens_output,
		tokens_input: result.metrics.tokens_input,
		tokens_output: result.metrics.tokens_output,
		model: result.metrics.model,
		provider: result.metrics.provider,
		duration_ms: result.metrics.duration_ms,
		cost_usd: result.metrics.cost_usd,
		thinking_tokens: result.metrics.thinking_tokens,
		cached_tokens: result.metrics.cached_tokens,
		cache_write_tokens: result.metrics.cache_write_tokens,
		model_id_used: result.metrics.model_id_used,
		timestamp: new Date()
	};
}

/**
 * Create a local system message for errors.
 *
 * @param workflowId - ID of the workflow
 * @param error - Error message
 * @returns Message object for UI display
 */
function createErrorMessage(workflowId: string, error: string): Message {
	return {
		id: generateUuid(),
		workflow_id: workflowId,
		role: 'system',
		content: `Error: ${error}`,
		tokens: 0,
		timestamp: new Date()
	};
}

/**
 * Service for orchestrating workflow execution.
 *
 * Encapsulates the 7-step message sending and streaming logic:
 * 1. Save user message to database
 * 2. Start streaming state
 * 3. Execute workflow via backend
 * 4. Update token counts and cost
 * 5. Save assistant response to database
 * 6. Refresh workflows and update cumulative tokens
 * 7. Return execution result
 */
/** Tracks workflow IDs currently being executed to prevent double-submit */
const executingWorkflows = new Set<string>();

export const WorkflowExecutorService = {
	/**
	 * Check if a workflow is currently being executed.
	 * Used by the UI to disable the send button during execution.
	 */
	isExecuting(workflowId: string): boolean {
		return executingWorkflows.has(workflowId);
	},

	/**
	 * Execute a workflow message with full orchestration.
	 *
	 * This method orchestrates all 8 steps of message sending:
	 * - Message persistence (user and assistant)
	 * - Streaming state management
	 * - Token tracking and cost calculation
	 * - Activity capture
	 * - Workflow refresh
	 *
	 * @param params - Execution parameters
	 * @param callbacks - Optional callbacks for UI updates
	 * @returns Execution result with success status and metrics
	 */
	async execute(params: ExecutionParams, callbacks?: ExecutionCallbacks): Promise<ExecutionResult> {
		const { workflowId, message, agentId, locale } = params;

		// Double-submit guard: prevent concurrent executions for same workflow
		if (executingWorkflows.has(workflowId)) {
			return {
				success: false,
				error: 'A message is already being processed for this workflow'
			};
		}

		// Check concurrent limit before starting
		if (!backgroundWorkflowsStore.canStart()) {
			const max = backgroundWorkflowsStore.getMaxConcurrent();
			toastStore.add({
				type: 'warning',
				title: t('toast_workflow_limit_title'),
				message: t('toast_workflow_limit_message', { max }),
				persistent: false,
				duration: 5000
			});
			return {
				success: false,
				error: `Maximum concurrent workflows (${max}) reached`
			};
		}

		// Helper: check if user is still viewing this workflow (may have switched)
		const isStillViewed = () => backgroundWorkflowsStore.getViewedWorkflowId() === workflowId;

		// Mark workflow as executing (atomic guard)
		executingWorkflows.add(workflowId);

		try {
			// Persist user message to DB and notify UI only while still viewed
			const userMessageId = await MessageService.saveUser(workflowId, message);
			if (isStillViewed()) {
				const userMessage = createUserMessage(workflowId, message);
				callbacks?.onUserMessage?.(userMessage);
			}

			// Track execution in background store so it persists across workflow switches
			const selectedWorkflow = workflowStore.getSelected();
			backgroundWorkflowsStore.register(workflowId, agentId, selectedWorkflow?.name ?? 'Workflow');

			// Initialize execution block UI only for the viewed workflow
			if (isStillViewed()) {
				tokenStore.startStreaming();
				executionBlocksStore.start(workflowId);
			}

			// Long-running IPC call - user may switch workflows during execution
			const workflowResult = await WorkflowService.executeStreaming(
				workflowId,
				message,
				agentId,
				locale
			);

			// Post-execution: user may have switched to a different workflow.
			// DB saves always run; UI updates only if still viewing this workflow.

			// Update token counters and cost display (only if still viewing this workflow)
			if (isStillViewed()) {
				tokenStore.setSessionTokens(
					workflowResult.metrics.tokens_input,
					workflowResult.metrics.tokens_output,
					workflowResult.metrics.cached_tokens,
					workflowResult.metrics.cache_write_tokens
				);
				tokenStore.setSessionCost(workflowResult.metrics.cost_usd);
			}
			callbacks?.onTokenUpdate?.(workflowResult.metrics);

			// Persist assistant response to DB regardless of current view
			// Use backend-generated message_id to match persisted blocks
			const assistantMessageId = await MessageService.saveAssistant(
				workflowId,
				workflowResult.response,
				workflowResult.metrics,
				workflowResult.message_id
			);
			// Only push to UI if still viewing this workflow
			if (isStillViewed()) {
				const assistantMessage = createAssistantMessage(workflowId, workflowResult);
				// Capture sub-agent summaries from the bg execution (same applyChunkToState
				// writes them, but bgWorkflows survives streamingStore removal).
				const bgExec = backgroundWorkflowsStore.getExecution(workflowId);
				const subAgents = bgExec?.subAgents ?? [];
				const subAgentSummaries: SubAgentSummary[] = subAgents
					.filter((a) => a.status === 'completed' || a.status === 'error')
					.map((a) => ({
						id: a.id,
						name: a.name,
						status: a.status as 'completed' | 'error',
						duration_ms: a.metrics?.duration_ms,
						tokens_input: a.metrics?.tokens_input,
						tokens_output: a.metrics?.tokens_output
					}));
				if (subAgentSummaries.length > 0) {
					assistantMessage.sub_agents = subAgentSummaries;
				}
				callbacks?.onAssistantMessage?.(assistantMessage);
			}

			// Refresh workflow list to reflect updated state
			await workflowStore.loadWorkflows();
			if (isStillViewed()) {
				const workflow = workflowStore.getSelected();
				if (workflow) {
					tokenStore.updateFromWorkflow(workflow);
				}
				callbacks?.onWorkflowRefresh?.(workflow);
			}

			// Capture blocks snapshot BEFORE finally{} resets the store
			return {
				success: true,
				userMessageId,
				assistantMessageId,
				metrics: workflowResult.metrics,
				workflowResult,
				blocks: get(executionBlocks)
			};
		} catch (error) {
			// Handle execution errors - always save to DB
			const errorMsg = getErrorMessage(error);
			try {
				await MessageService.saveSystem(workflowId, `Error: ${errorMsg}`);
			} catch {
				// Best-effort persistence: if saving the system error row itself
				// fails, the original errorMsg is still pushed to the UI below.
			}

			// Only push error to UI if still viewing this workflow
			if (isStillViewed()) {
				const errorMessage = createErrorMessage(workflowId, errorMsg);
				callbacks?.onError?.(errorMessage);
			}

			return {
				success: false,
				error: errorMsg
			};
		} finally {
			// Release double-submit guard
			executingWorkflows.delete(workflowId);

			// Only cleanup execution-block / token UI if still viewing this workflow
			if (isStillViewed()) {
				executionBlocksStore.reset();
				tokenStore.stopStreaming();
			}
		}
	}
};
