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
 * @fileoverview Streaming event types for real-time workflow execution.
 *
 * These types are synchronized with Rust backend types (src-tauri/src/models/streaming.rs)
 * to ensure type safety for Tauri event streaming.
 *
 * @module types/streaming
 */

/**
 * Type of streaming chunk content.
 *
 * Synchronized with Rust `ChunkType` enum in `src-tauri/src/models/streaming.rs`.
 */
export type ChunkType =
  | 'token'
  | 'tool_start'
  | 'tool_end'
  | 'reasoning'
  | 'error'
  | 'sub_agent_start'
  | 'sub_agent_progress'
  | 'sub_agent_complete'
  | 'sub_agent_error'
  | 'task_create'
  | 'task_update'
  | 'task_complete';

/**
 * Metrics included in sub-agent complete events.
 *
 * Synchronized with Rust `SubAgentStreamMetrics` in streaming.rs.
 */
export interface SubAgentStreamMetrics {
  /** Execution duration in milliseconds */
  duration_ms: number;
  /** Input tokens consumed */
  tokens_input: number;
  /** Output tokens generated */
  tokens_output: number;
}

/**
 * Streaming chunk emitted during workflow execution.
 *
 * Synchronized with Rust `StreamChunk` in `src-tauri/src/models/streaming.rs`.
 */
export interface StreamChunk {
  /** Associated workflow ID */
  workflow_id: string;
  /** Type of chunk content */
  chunk_type: ChunkType;
  /** Text content (for token/reasoning/error/sub_agent chunks) */
  content?: string;
  /** Tool name (for tool_start/tool_end chunks) */
  tool?: string;
  /** Duration in milliseconds (for tool_end/sub_agent_complete/sub_agent_error/task_complete chunks) */
  duration?: number;
  /** Sub-agent ID (for sub_agent_* chunks) */
  sub_agent_id?: string;
  /** Sub-agent name (for sub_agent_* chunks) */
  sub_agent_name?: string;
  /** Parent agent ID (for sub_agent_* chunks) */
  parent_agent_id?: string;
  /** Sub-agent metrics (for sub_agent_complete chunks) */
  metrics?: SubAgentStreamMetrics;
  /** Progress percentage 0-100 (for sub_agent_progress chunks) */
  progress?: number;
  /** Task ID (for task_* chunks) */
  task_id?: string;
  /** Task name (for task_* chunks) */
  task_name?: string;
  /** Task status (for task_* chunks) */
  task_status?: 'pending' | 'in_progress' | 'completed' | 'blocked';
  /** Task priority (for task_* chunks) */
  task_priority?: 1 | 2 | 3 | 4 | 5;
}

/**
 * Event emitted when workflow execution completes
 */
export interface WorkflowComplete {
  /** Associated workflow ID */
  workflow_id: string;
  /** Final workflow status */
  status: 'completed' | 'error' | 'cancelled';
  /** Error message if status is 'error' */
  error?: string;
}

/**
 * Event names for Tauri event listeners
 */
export const STREAM_EVENTS = {
  /** Streaming chunk event */
  WORKFLOW_STREAM: 'workflow_stream',
  /** Workflow completion event */
  WORKFLOW_COMPLETE: 'workflow_complete',
} as const;
