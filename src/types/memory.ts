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
  type: MemoryType;
  /** Text content of the memory */
  content: string;
  /** Additional metadata */
  metadata?: Record<string, unknown>;
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
  type_filter?: MemoryType;
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
