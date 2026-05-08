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
 * @fileoverview Message service for encapsulating message-related Tauri IPC calls.
 *
 * @module lib/services/message
 */

import { tauriInvoke as invoke } from '$lib/tauri';
import type { Message, MessageMetrics, SubAgentSummary } from '$types/message';
import type { SubAgentExecution } from '$types/sub-agent';
import type { WorkflowMetrics } from '$types/workflow';
import { getErrorMessage } from '$lib/utils/error';
import { SubAgentExecutionService } from '$lib/services/sub-agent-execution.service';

/**
 * Parameters for creating a message via save_message command.
 */
interface MessageCreate {
	workflowId: string;
	role: 'user' | 'assistant' | 'system';
	content: string;
	metrics?: WorkflowMetrics;
	/** Pre-generated message ID for block association */
	messageId?: string;
}

/**
 * Enrich messages with sub-agent execution data from the database.
 *
 * For each completed/error sub-agent execution, finds the first assistant message
 * whose timestamp >= the execution's completion time (or creation time as fallback),
 * then attaches a SubAgentSummary to that message's sub_agents array.
 *
 * @param messages - Messages sorted chronologically
 * @param executions - Sub-agent executions for the same workflow
 * @returns Messages with sub_agents populated where applicable
 */
function enrichMessagesWithSubAgents(
	messages: Message[],
	executions: SubAgentExecution[]
): Message[] {
	// Include cancelled sub-agents as completed (they were intentionally stopped)
	const terminal = executions.filter(
		(e) => e.status === 'completed' || e.status === 'error' || e.status === 'cancelled'
	);
	if (terminal.length === 0) return messages;

	const assistantMessages = messages.filter((m) => m.role === 'assistant');
	if (assistantMessages.length === 0) return messages;

	for (const exec of terminal) {
		const execTime = exec.completed_at ?? exec.created_at;
		const execDate = new Date(execTime);

		const target = assistantMessages.find((m) => new Date(m.timestamp) >= execDate);
		if (!target) continue;

		const summary: SubAgentSummary = {
			id: exec.id,
			name: exec.sub_agent_name,
			status: exec.status === 'cancelled' ? 'completed' : (exec.status as 'completed' | 'error'),
			duration_ms: exec.duration_ms,
			tokens_input: exec.tokens_input,
			tokens_output: exec.tokens_output
		};

		if (!target.sub_agents) {
			target.sub_agents = [];
		}

		const alreadyExists = target.sub_agents.some((s) => s.id === summary.id);
		if (!alreadyExists) {
			target.sub_agents.push(summary);
		}
	}

	return messages;
}

/**
 * Service for message operations.
 *
 * Encapsulates all message-related Tauri IPC commands.
 */
export const MessageService = {
	/**
	 * Load all messages for a workflow.
	 *
	 * @param workflowId - Workflow ID to load messages for
	 * @returns Object containing messages array and optional error message
	 */
	async load(workflowId: string): Promise<{ messages: Message[]; error?: string }> {
		try {
			const messages = await invoke<Message[]>('load_workflow_messages', { workflowId });
			return { messages };
		} catch (e) {
			return { messages: [], error: getErrorMessage(e) };
		}
	},

	/**
	 * Load messages with sub-agent data enrichment.
	 *
	 * Loads messages and sub-agent executions in parallel, then correlates
	 * executions to assistant messages by timestamp.
	 *
	 * @param workflowId - Workflow ID to load messages for
	 * @returns Object containing enriched messages array and optional error message
	 */
	async loadWithSubAgents(workflowId: string): Promise<{ messages: Message[]; executions: SubAgentExecution[]; error?: string }> {
		try {
			const [messages, executions] = await Promise.all([
				invoke<Message[]>('load_workflow_messages', { workflowId }),
				SubAgentExecutionService.loadSubAgentExecutions(workflowId)
			]);
			return { messages: enrichMessagesWithSubAgents(messages, executions), executions };
		} catch (e) {
			return { messages: [], executions: [], error: getErrorMessage(e) };
		}
	},

	/**
	 * Save a message to the database.
	 *
	 * @param params - Message creation parameters
	 * @returns ID of the saved message
	 */
	async save(params: MessageCreate): Promise<string> {
		return invoke<string>('save_message', {
			workflowId: params.workflowId,
			role: params.role,
			content: params.content,
			tokensInput: params.metrics?.tokens_input ?? null,
			tokensOutput: params.metrics?.tokens_output ?? null,
			model: params.metrics?.model ?? null,
			provider: params.metrics?.provider ?? null,
			durationMs: params.metrics?.duration_ms ?? null,
			thinkingTokens: params.metrics?.thinking_tokens ?? null,
			costUsd: params.metrics?.cost_usd ?? null,
			cachedTokens: params.metrics?.cached_tokens ?? null,
			cacheWriteTokens: params.metrics?.cache_write_tokens ?? null,
			modelIdUsed: params.metrics?.model_id_used ?? null,
			messageId: params.messageId ?? null
		});
	},

	/**
	 * Save a user message.
	 *
	 * @param workflowId - Workflow ID
	 * @param content - Message content
	 * @returns ID of the saved message
	 */
	async saveUser(workflowId: string, content: string): Promise<string> {
		return this.save({ workflowId, role: 'user', content });
	},

	/**
	 * Save an assistant message with optional metrics.
	 *
	 * @param workflowId - Workflow ID
	 * @param content - Message content
	 * @param metrics - Optional workflow execution metrics
	 * @returns ID of the saved message
	 */
	async saveAssistant(workflowId: string, content: string, metrics?: WorkflowMetrics, messageId?: string): Promise<string> {
		return this.save({ workflowId, role: 'assistant', content, metrics, messageId });
	},

	/**
	 * Save a system message (for errors, notifications).
	 *
	 * @param workflowId - Workflow ID
	 * @param content - Message content
	 * @returns ID of the saved message
	 */
	async saveSystem(workflowId: string, content: string): Promise<string> {
		return this.save({ workflowId, role: 'system', content });
	},

	/**
	 * Clear all messages for a workflow.
	 *
	 * @param workflowId - Workflow ID
	 */
	async clear(workflowId: string): Promise<void> {
		return invoke<void>('clear_workflow_messages', { workflowId });
	},

	/**
	 * Returns lightweight metrics from the most recent assistant message of a
	 * workflow. `null` when the workflow has no assistant message yet, or when
	 * the call fails (caller falls back to a blank session).
	 */
	async getLastAssistantMetrics(workflowId: string): Promise<MessageMetrics | null> {
		try {
			return await invoke<MessageMetrics | null>(
				'get_workflow_last_assistant_message_metrics',
				{ workflowId }
			);
		} catch {
			// Non-blocking: fall back to an empty session display. The user is
			// not waiting on this metrics call; surfacing the error here would
			// be more noise than signal.
			return null;
		}
	}
};
