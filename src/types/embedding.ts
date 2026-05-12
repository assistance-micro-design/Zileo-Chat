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
 * Embedding configuration settings.
 *
 * Chunking parameters and vector dimension are no longer user-configurable:
 * chunking is fixed by `tools/memory/chunker.rs` constants (512/50) and the
 * dimension is locked at 1024 by the HNSW index schema.
 */
export interface EmbeddingConfig {
	/** Embedding provider: 'mistral' or 'ollama' */
	provider: EmbeddingProviderType;
	/** Embedding model name (e.g., 'mistral-embed', 'mxbai-embed-large') */
	model: string;
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
 * Result of embedding test operation
 */
export interface EmbeddingTestResult {
	/** Whether embedding was generated successfully */
	success: boolean;
	/** Vector dimension (e.g., 1024) */
	dimension: number;
	/** First 5 values of the embedding (preview) */
	preview: number[];
	/** Generation time in milliseconds */
	duration_ms: number;
	/** Provider used (mistral/ollama) */
	provider: string;
	/** Model used */
	model: string;
	/** Error message if failed */
	error?: string;
}

/**
 * Token statistics for a single memory category
 */
export interface CategoryTokenStats {
	/** Memory type (user_pref, context, knowledge, decision) */
	memory_type: string;
	/** Number of memories in this category */
	count: number;
	/** Total characters in this category */
	total_chars: number;
	/** Estimated tokens (chars / 4) */
	estimated_tokens: number;
	/** Average characters per memory */
	avg_chars: number;
	/** Number with embeddings generated */
	with_embeddings: number;
}

/**
 * Token statistics for all memory categories
 */
export interface MemoryTokenStats {
	/** Statistics per memory type */
	categories: CategoryTokenStats[];
	/** Total characters across all categories */
	total_chars: number;
	/** Estimated total tokens (chars / 4) */
	total_estimated_tokens: number;
	/** Total memories counted */
	total_memories: number;
}

/**
 * Status snapshot of a `reindex_memory_chunks` background job.
 *
 * Mirrors the Rust `ReindexJobStatus` struct (camelCase via serde rename).
 * `status` transitions: "running" → ("completed" | "cancelled" | "error").
 * `currentMemoryId` is populated only between two processed parents.
 */
export interface ReindexJobStatus {
	jobId: string;
	status: 'running' | 'completed' | 'cancelled' | 'error';
	processed: number;
	total: number;
	chunksCreated: number;
	currentMemoryId?: string;
	errorMessage?: string;
	startedAt: string;
	finishedAt?: string;
}

/**
 * Available embedding models per provider.
 *
 * All listed models produce 1024-dimensional vectors to match the HNSW
 * index schema (`memory_chunk_vec_idx DIMENSION 1024`). `nomic-embed-text`
 * (768D) was removed because it is incompatible with this index.
 */
export const EMBEDDING_MODELS: Record<
	EmbeddingProviderType,
	{ value: string; label: string; dimension: number }[]
> = {
	mistral: [{ value: 'mistral-embed', label: 'Mistral Embed (1024D)', dimension: 1024 }],
	ollama: [{ value: 'mxbai-embed-large', label: 'MxBai Embed Large (1024D)', dimension: 1024 }]
};

/**
 * Default embedding configuration
 */
export const DEFAULT_EMBEDDING_CONFIG: EmbeddingConfig = {
	provider: 'mistral',
	model: 'mistral-embed'
};
