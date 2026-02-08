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
 * Message role in the conversation
 */
export type MessageRole = 'user' | 'assistant' | 'system';

/**
 * Summary of a sub-agent execution (frontend-only, not persisted in message).
 * Captured from StreamingState during current session.
 */
export interface SubAgentSummary {
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
}

/**
 * Message entity representing a conversation message with optional metrics.
 *
 * Extended in Phase 6 to include token counts, model info, cost, and duration
 * for analytics and state recovery.
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
  tokens: number;
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
  /** Message timestamp */
  timestamp: Date;
  /** Sub-agent summaries (transient, captured from StreamingState) */
  sub_agents?: SubAgentSummary[];
}

/**
 * Payload for creating a new message (sent to backend).
 * ID and timestamp are generated server-side.
 */
export interface MessageCreate {
  /** Associated workflow ID */
  workflow_id: string;
  /** Message role */
  role: MessageRole;
  /** Message content */
  content: string;
  /** Input tokens consumed */
  tokens_input?: number;
  /** Output tokens generated */
  tokens_output?: number;
  /** Model used */
  model?: string;
  /** Provider used */
  provider?: string;
  /** Cost in USD */
  cost_usd?: number;
  /** Duration in milliseconds */
  duration_ms?: number;
}

/**
 * Creates a user message payload.
 *
 * @param workflowId - The workflow ID
 * @param content - Message content
 * @returns MessageCreate payload for user message
 */
export function createUserMessage(workflowId: string, content: string): MessageCreate {
  return {
    workflow_id: workflowId,
    role: 'user',
    content,
  };
}

/**
 * Creates an assistant message payload with metrics.
 *
 * @param workflowId - The workflow ID
 * @param content - Message content
 * @param metrics - Optional metrics from WorkflowResult
 * @returns MessageCreate payload for assistant message
 */
export function createAssistantMessage(
  workflowId: string,
  content: string,
  metrics?: {
    tokens_input?: number;
    tokens_output?: number;
    model?: string;
    provider?: string;
    duration_ms?: number;
    cost_usd?: number;
  }
): MessageCreate {
  return {
    workflow_id: workflowId,
    role: 'assistant',
    content,
    tokens_input: metrics?.tokens_input,
    tokens_output: metrics?.tokens_output,
    model: metrics?.model,
    provider: metrics?.provider,
    duration_ms: metrics?.duration_ms,
    cost_usd: metrics?.cost_usd,
  };
}

/**
 * Creates a system message payload (for errors, notifications).
 *
 * @param workflowId - The workflow ID
 * @param content - Message content
 * @returns MessageCreate payload for system message
 */
export function createSystemMessage(workflowId: string, content: string): MessageCreate {
  return {
    workflow_id: workflowId,
    role: 'system',
    content,
  };
}

/**
 * Response for paginated message loading.
 *
 * Includes pagination metadata for cursor-based navigation.
 */
export interface PaginatedMessages {
  /** Messages in the current page */
  messages: Message[];
  /** Total number of messages available */
  total: number;
  /** Current offset (number of messages skipped) */
  offset: number;
  /** Page size limit */
  limit: number;
  /** Whether more messages are available after this page */
  has_more: boolean;
}
