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
 * Metadata associated with a memory entry.
 *
 * These fields are commonly used but the object can contain additional
 * custom fields as it maps to serde_json::Value in Rust.
 */
export interface MemoryMetadata {
  /** Source of the memory (e.g., agent ID, "settings", "user") */
  source?: string;
  /** Classification tags for filtering and organization */
  tags?: string[];
  /** Relevance score from vector search (0-1) */
  relevance_score?: number;
  /** Embedding model used for vector storage */
  embedding_model?: string;
  /** Allow additional custom fields */
  [key: string]: unknown;
}

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
  /** Optional workflow ID for scoped memories (absent = general) */
  workflow_id?: string;
  /** Additional metadata */
  metadata: MemoryMetadata;
  /** Importance score (0.0-1.0, higher = more important) */
  importance: number;
  /** Optional expiration timestamp (ISO string, absent = permanent) */
  expires_at?: string;
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
  metadata?: MemoryMetadata;
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

/**
 * Result of the describe operation - statistics about memories
 */
export interface MemoryDescribeResult {
  /** Total number of matching memories */
  total: number;
  /** Count by memory type */
  by_type: Record<string, number>;
  /** All unique tags across matching memories */
  tags: string[];
  /** Number of workflow-scoped memories */
  workflow_count: number;
  /** Number of general (cross-workflow) memories */
  general_count: number;
  /** Oldest memory timestamp (ISO string) */
  oldest?: string;
  /** Newest memory timestamp (ISO string) */
  newest?: string;
}
