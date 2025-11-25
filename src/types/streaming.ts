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
 * Type of streaming chunk content
 */
export type ChunkType = 'token' | 'tool_start' | 'tool_end' | 'reasoning' | 'error';

/**
 * Streaming chunk emitted during workflow execution
 */
export interface StreamChunk {
  /** Associated workflow ID */
  workflow_id: string;
  /** Type of chunk content */
  chunk_type: ChunkType;
  /** Text content (for token/reasoning/error chunks) */
  content?: string;
  /** Tool name (for tool_start/tool_end chunks) */
  tool?: string;
  /** Duration in milliseconds (for tool_end chunks) */
  duration?: number;
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
