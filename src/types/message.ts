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
 * Message role in the conversation
 */
export type MessageRole = 'user' | 'assistant' | 'system';

/**
 * Summary of a sub-agent execution (frontend-only, not persisted in message).
 * Captured from `backgroundWorkflowsStore.getExecution(workflowId).subAgents`
 * during the current session.
 */
export interface SubAgentSummary {
	/** Execution record ID for unique identification */
	id: string;
	/** Sub-agent name */
	name: string;
	/** Execution status */
	status: 'completed' | 'error';
	/** Duration in milliseconds */
	duration_ms?: number;
	/** Input tokens consumed */
	tokens_input?: number;
	/** Output tokens generated */
	tokens_output?: number;
	/**
	 * USD cost computed with the sub-agent's OWN pricing (not the parent's).
	 * Omitted when the sub-agent's `(provider, model)` has no pricing row or
	 * when the sub-agent is not registered. Sourced from the wire chunk
	 * `sub_agent_complete.metrics.cost_usd` populated by `compute_sub_agent_cost`.
	 */
	cost_usd?: number;
	/**
	 * Cached prompt tokens (cache reads). Same wire source as `cost_usd`
	 * — propagated to `MessageMetrics` for per-sub-agent chip display.
	 */
	cached_tokens?: number;
	/** Cache-write prompt tokens. Same contract as `cached_tokens`. */
	cache_write_tokens?: number;
	/** Thinking/reasoning tokens (reasoning models only). */
	thinking_tokens?: number;
}

/**
 * Message entity representing a conversation message with optional metrics.
 *
 * Includes token counts, model info, cost, and duration for analytics and state recovery.
 */
export interface Message {
	/** Unique identifier (UUID) */
	id: string;
	/** Associated workflow ID */
	workflow_id: string;
	/** Message role (user, assistant, system) */
	role: MessageRole;
	/** Message content (text) */
	content: string;
	/** Legacy token count (deprecated, use tokens_input/tokens_output) */
	tokens?: number;
	/** Input tokens consumed (for assistant messages) */
	tokens_input?: number;
	/** Output tokens generated (for assistant messages) */
	tokens_output?: number;
	/** Model used for generation (e.g., "mistral-large-latest") */
	model?: string;
	/** Provider used (e.g., "Mistral", "Ollama") */
	provider?: string;
	/** Estimated cost in USD */
	cost_usd?: number;
	/** Generation duration in milliseconds */
	duration_ms?: number;
	/** Thinking/reasoning tokens (for reasoning models) */
	thinking_tokens?: number;
	/** Cached prompt tokens (cache reads) when the provider exposes them. */
	cached_tokens?: number;
	/** Cache-write prompt tokens (priming cost on first request). */
	cache_write_tokens?: number;
	/** `llm_model.id` of the model that produced the assistant response. */
	model_id_used?: string;
	/** Message timestamp */
	timestamp: Date;
	/** Sub-agent summaries (transient, captured from backgroundWorkflowsStore) */
	sub_agents?: SubAgentSummary[];
}

/**
 * Lightweight metrics extracted from the most recent assistant message of a
 * workflow. Used by `selectWorkflow` to restore the session display when the
 * workflow has no live execution running.
 */
export interface MessageMetrics {
	tokens_input: number | null;
	tokens_output: number | null;
	cached_tokens: number | null;
	cache_write_tokens: number | null;
	thinking_tokens: number | null;
	cost_usd: number | null;
	/** `llm_model.id` of the model that produced the response (for pricing lookup). */
	model_id_used: string | null;
}
