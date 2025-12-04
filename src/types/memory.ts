// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Memory types for RAG and context persistence.
 *
 * These types are synchronized with Rust backend types (src-tauri/src/models/memory.rs)
 * to ensure type safety for memory operations.
 *
 * @module types/memory
 */

/**
 * Type of memory content
 */
export type MemoryType = 'user_pref' | 'context' | 'knowledge' | 'decision';

/**
 * Memory entity for persistent context and RAG
 */
export interface Memory {
  /** Unique identifier */
  id: string;
  /** Type of memory content */
  type: MemoryType;
  /** Text content of the memory */
  content: string;
  /** Optional workflow ID for scoped memories (null = general) */
  workflow_id?: string | null;
  /** Additional metadata */
  metadata: Record<string, unknown>;
  /** Creation timestamp (ISO string from backend) */
  created_at: string;
}

/**
 * Parameters for creating a new memory
 */
export interface CreateMemoryParams {
  /** Type of memory content */
  memoryType: MemoryType;
  /** Text content of the memory */
  content: string;
  /** Additional metadata */
  metadata?: Record<string, unknown>;
  /** Optional workflow ID for scoped memories (None = general) */
  workflowId?: string;
}

/**
 * Parameters for listing memories
 */
export interface ListMemoryParams {
  /** Filter by memory type */
  typeFilter?: MemoryType;
  /** Optional workflow ID filter (None = all memories) */
  workflowId?: string;
}

/**
 * Parameters for searching memories
 */
export interface SearchMemoryParams {
  /** Search query text */
  query: string;
  /** Maximum number of results */
  limit?: number;
  /** Filter by memory type */
  typeFilter?: MemoryType;
  /** Optional workflow ID filter (None = all memories) */
  workflowId?: string;
  /** Similarity threshold 0-1 for vector search (default: 0.7) */
  threshold?: number;
}

/**
 * Memory search result with relevance score
 */
export interface MemorySearchResult {
  /** Memory entity */
  memory: Memory;
  /** Relevance score (0-1, higher is more relevant) */
  score: number;
}
