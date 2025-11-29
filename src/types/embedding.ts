// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Embedding configuration types for Memory Tool settings.
 *
 * These types are synchronized with Rust backend types (src-tauri/src/llm/embedding.rs)
 * to ensure type safety for embedding and memory settings operations.
 *
 * @module types/embedding
 */

/**
 * Embedding provider type
 */
export type EmbeddingProviderType = 'mistral' | 'ollama';

/**
 * Chunking strategy for text processing
 */
export type ChunkingStrategy = 'fixed' | 'semantic' | 'recursive';

/**
 * Embedding configuration settings
 */
export interface EmbeddingConfig {
  /** Embedding provider: 'mistral' or 'ollama' */
  provider: EmbeddingProviderType;
  /** Embedding model name (e.g., 'mistral-embed', 'nomic-embed-text') */
  model: string;
  /** Vector dimension (auto-set based on model) */
  dimension: number;
  /** Maximum tokens per input (provider-specific) */
  max_tokens: number;
  /** Characters per chunk for long texts */
  chunk_size: number;
  /** Overlap between chunks in characters */
  chunk_overlap: number;
  /** Chunking strategy for text processing */
  strategy?: ChunkingStrategy;
}

/**
 * Memory statistics for the dashboard
 */
export interface MemoryStats {
  /** Total number of memories */
  total: number;
  /** Memories with embeddings generated */
  with_embeddings: number;
  /** Memories without embeddings */
  without_embeddings: number;
  /** Memory count by type */
  by_type: Record<string, number>;
  /** Memory count by agent source */
  by_agent: Record<string, number>;
}

/**
 * Parameters for updating a memory
 */
export interface UpdateMemoryParams {
  /** Memory ID to update */
  memory_id: string;
  /** New content (optional) */
  content?: string;
  /** New metadata (optional) */
  metadata?: Record<string, unknown>;
}

/**
 * Export format for memories
 */
export type ExportFormat = 'json' | 'csv';

/**
 * Parameters for exporting memories
 */
export interface ExportMemoriesParams {
  /** Export format: 'json' or 'csv' */
  format: ExportFormat;
  /** Optional type filter */
  type_filter?: string;
}

/**
 * Parameters for importing memories
 */
export interface ImportMemoriesParams {
  /** JSON array of memories to import */
  data: string;
}

/**
 * Result of memory import operation
 */
export interface ImportResult {
  /** Number of memories successfully imported */
  imported: number;
  /** Number of memories that failed to import */
  failed: number;
  /** Error messages for failed imports */
  errors: string[];
}

/**
 * Parameters for regenerating embeddings
 */
export interface RegenerateEmbeddingsParams {
  /** Optional type filter (regenerate only this type) */
  type_filter?: string;
}

/**
 * Result of embedding regeneration
 */
export interface RegenerateResult {
  /** Number of memories processed */
  processed: number;
  /** Number of embeddings successfully generated */
  success: number;
  /** Number of failures */
  failed: number;
}

/**
 * Available embedding models per provider
 */
export const EMBEDDING_MODELS: Record<EmbeddingProviderType, { value: string; label: string; dimension: number }[]> = {
  mistral: [
    { value: 'mistral-embed', label: 'Mistral Embed (1024D)', dimension: 1024 }
  ],
  ollama: [
    { value: 'nomic-embed-text', label: 'Nomic Embed Text (768D)', dimension: 768 },
    { value: 'mxbai-embed-large', label: 'MxBai Embed Large (1024D)', dimension: 1024 }
  ]
};

/**
 * Default embedding configuration
 */
export const DEFAULT_EMBEDDING_CONFIG: EmbeddingConfig = {
  provider: 'mistral',
  model: 'mistral-embed',
  dimension: 1024,
  max_tokens: 8192,
  chunk_size: 512,
  chunk_overlap: 50,
  strategy: 'fixed'
};
