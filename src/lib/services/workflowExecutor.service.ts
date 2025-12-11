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
 * 6. Capture activities
 * 7. Refresh workflows
 * 8. Return result with metrics
 *
 * @module lib/services/workflowExecutor
 */

import type { Message } from '$types/message';
import type { Workflow, WorkflowMetrics, WorkflowResult } from '$types/workflow';
import { MessageService } from './message.service';
import { WorkflowService } from './workflow.service';
import { streamingStore } from '$lib/stores/streaming';
import { tokenStore } from '$lib/stores/tokens';
import { activityStore } from '$lib/stores/activity';
import { workflowStore } from '$lib/stores/workflows';

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
	/** ID of the saved assistant message */
	assistantMessageId?: string;
	/** Error message if execution failed */
	error?: string;
	/** Execution metrics (tokens, cost, duration) */
	metrics?: WorkflowMetrics;
	/** The full workflow result */
	workflowResult?: WorkflowResult;
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
		id: crypto.randomUUID(),
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
		id: crypto.randomUUID(),
		workflow_id: workflowId,
		role: 'assistant',
		content: result.report,
		tokens: result.metrics.tokens_output,
		tokens_input: result.metrics.tokens_input,
		tokens_output: result.metrics.tokens_output,
		model: result.metrics.model,
		provider: result.metrics.provider,
		duration_ms: result.metrics.duration_ms,
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
		id: crypto.randomUUID(),
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
 * Encapsulates the 8-step message sending and streaming logic:
 * 1. Save user message to database
 * 2. Start streaming state
 * 3. Execute workflow via backend
 * 4. Update token counts and cost
 * 5. Save assistant response to database
 * 6. Capture streaming activities to historical
 * 7. Refresh workflows and update cumulative tokens
 * 8. Return execution result
 */
export const WorkflowExecutorService = {
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
	 *
	 * @example
	 * ```typescript
	 * const result = await WorkflowExecutorService.execute(
	 *   {
	 *     workflowId: 'wf-123',
	 *     message: 'Hello, analyze this code',
	 *     agentId: 'agent-456',
	 *     locale: 'en'
	 *   },
	 *   {
	 *     onUserMessage: (msg) => messages.push(msg),
	 *     onAssistantMessage: (msg) => messages.push(msg),
	 *     onError: (msg) => messages.push(msg)
	 *   }
	 * );
	 * ```
	 */
	async execute(params: ExecutionParams, callbacks?: ExecutionCallbacks): Promise<ExecutionResult> {
		const { workflowId, message, agentId, locale } = params;

		try {
			// Step 1: Save user message
			const userMessageId = await MessageService.saveUser(workflowId, message);
			const userMessage = createUserMessage(workflowId, message);
			callbacks?.onUserMessage?.(userMessage);

			// Step 2: Start streaming
			tokenStore.startStreaming();
			await streamingStore.start(workflowId);

			// Step 3: Execute workflow
			const workflowResult = await WorkflowService.executeStreaming(
				workflowId,
				message,
				agentId,
				locale
			);

			// Step 4: Update tokens and cost
			tokenStore.setInputTokens(workflowResult.metrics.tokens_input);
			tokenStore.updateStreamingTokens(workflowResult.metrics.tokens_output);
			tokenStore.setSessionCost(workflowResult.metrics.cost_usd);
			callbacks?.onTokenUpdate?.(workflowResult.metrics);

			// Step 5: Save assistant response
			const assistantMessageId = await MessageService.saveAssistant(
				workflowId,
				workflowResult.report,
				workflowResult.metrics
			);
			const assistantMessage = createAssistantMessage(workflowId, workflowResult);
			callbacks?.onAssistantMessage?.(assistantMessage);

			// Step 6: Capture streaming activities to historical
			activityStore.captureStreamingActivities();

			// Step 7: Refresh workflows and update cumulative tokens
			await workflowStore.loadWorkflows();
			const workflow = workflowStore.getSelected();
			if (workflow) {
				tokenStore.updateFromWorkflow(workflow);
			}
			callbacks?.onWorkflowRefresh?.(workflow);

			// Step 8: Return success result
			return {
				success: true,
				userMessageId,
				assistantMessageId,
				metrics: workflowResult.metrics,
				workflowResult
			};
		} catch (error) {
			// Handle execution errors
			const errorMsg = error instanceof Error ? error.message : String(error);
			await MessageService.saveSystem(workflowId, `Error: ${errorMsg}`);
			const errorMessage = createErrorMessage(workflowId, errorMsg);
			callbacks?.onError?.(errorMessage);

			return {
				success: false,
				error: errorMsg
			};
		} finally {
			// Always cleanup streaming state
			await streamingStore.reset();
			tokenStore.stopStreaming();
		}
	}
};
