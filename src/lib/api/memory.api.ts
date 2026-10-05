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
 * @fileoverview Typed Tauri API client for the memory domain.
 *
 * Covers memories and the embedding service (configuration, statistics,
 * import/export, reindex jobs).
 *
 * @module lib/api/memory
 */

import { invokeCommand } from './internal';
import type { ChunkSearchResult, Memory, MemoryType, PurgeExpiredResult } from '$types/memory';
import type {
	EmbeddingConfig,
	EmbeddingTestResult,
	ExportFormat,
	ImportResult,
	MemoryStats,
	MemoryTokenStats,
	ReindexJobStatus
} from '$types/embedding';

// ============================================================================
// Memories
// ============================================================================

/** Add a memory entry, returning its ID. */
export async function addMemory(
	memoryType: MemoryType,
	content: string,
	metadata?: unknown,
	workflowId?: string | null
): Promise<string> {
	return invokeCommand<string>('add_memory', {
		memoryType,
		content,
		metadata: metadata,
		workflowId: workflowId
	});
}

/** List memories with optional filters. */
export async function listMemories(
	typeFilter?: MemoryType | null,
	workflowId?: string | null
): Promise<Memory[]> {
	return invokeCommand<Memory[]>('list_memories', {
		typeFilter: typeFilter,
		workflowId: workflowId
	});
}

/** Get a single memory. */
export async function getMemory(memoryId: string): Promise<Memory> {
	return invokeCommand<Memory>('get_memory', { memoryId });
}

/** Delete a memory entry. */
export async function deleteMemory(memoryId: string): Promise<void> {
	return invokeCommand<void>('delete_memory', { memoryId });
}

/** Input for `searchMemories`. */
export interface SearchMemoriesInput {
	query: string;
	limit?: number | null;
	typeFilter?: MemoryType | null;
	workflowId?: string | null;
	threshold?: number | null;
	tagsFilter?: string[] | null;
}

/** Search memories (vector + text). */
export async function searchMemories(input: SearchMemoriesInput): Promise<ChunkSearchResult[]> {
	return invokeCommand<ChunkSearchResult[]>('search_memories', {
		query: input.query,
		limit: input.limit,
		typeFilter: input.typeFilter,
		workflowId: input.workflowId,
		threshold: input.threshold,
		tagsFilter: input.tagsFilter
	});
}

/** Delete all memories of a type. Returns the number deleted. */
export async function clearMemoriesByType(memoryType: MemoryType): Promise<number> {
	return invokeCommand<number>('clear_memories_by_type', { memoryType });
}

/** Purge expired memories. */
export async function purgeExpiredMemories(): Promise<PurgeExpiredResult> {
	return invokeCommand<PurgeExpiredResult>('purge_expired_memories');
}

/** Update a memory's content and/or metadata. */
export async function updateMemory(
	memoryId: string,
	content?: string | null,
	metadata?: unknown
): Promise<Memory> {
	return invokeCommand<Memory>('update_memory', {
		memoryId,
		content: content,
		metadata: metadata
	});
}

// ============================================================================
// Embedding service
// ============================================================================

/** Get the embedding configuration (`null` when unconfigured). */
export async function getEmbeddingConfig(): Promise<EmbeddingConfig | null> {
	return invokeCommand<EmbeddingConfig | null>('get_embedding_config');
}

/** Save the embedding configuration. */
export async function saveEmbeddingConfig(config: EmbeddingConfig): Promise<void> {
	return invokeCommand<void>('save_embedding_config', { config });
}

/** Delete the embedding configuration. */
export async function deleteEmbeddingConfig(): Promise<void> {
	return invokeCommand<void>('delete_embedding_config');
}

/** Reinitialize the embedding service. */
export async function reinitEmbeddingService(): Promise<void> {
	return invokeCommand<void>('reinit_embedding_service');
}

/** Test embedding generation on a sample text. */
export async function testEmbedding(text: string): Promise<EmbeddingTestResult> {
	return invokeCommand<EmbeddingTestResult>('test_embedding', { text });
}

/** Export memories, returning the file content. */
export async function exportMemories(
	format: ExportFormat,
	typeFilter?: string | null
): Promise<string> {
	return invokeCommand<string>('export_memories', {
		format,
		typeFilter: typeFilter
	});
}

/** Import memories from a JSON payload. */
export async function importMemories(data: string): Promise<ImportResult> {
	return invokeCommand<ImportResult>('import_memories', { data });
}

/** Start a background reindex job, returning its ID. */
export async function reindexMemoryChunks(): Promise<string> {
	return invokeCommand<string>('reindex_memory_chunks');
}

/** Cancel a reindex job. */
export async function cancelReindexJob(jobId: string): Promise<void> {
	return invokeCommand<void>('cancel_reindex_job', { jobId });
}

/** Get the status of a reindex job (`null` when unknown). */
export async function getReindexJobStatus(jobId: string): Promise<ReindexJobStatus | null> {
	return invokeCommand<ReindexJobStatus | null>('get_reindex_job_status', { jobId });
}

/** Get memory statistics for the dashboard. */
export async function getMemoryStats(): Promise<MemoryStats> {
	return invokeCommand<MemoryStats>('get_memory_stats');
}

/** Get token statistics per memory category. */
export async function getMemoryTokenStats(typeFilter?: string | null): Promise<MemoryTokenStats> {
	return invokeCommand<MemoryTokenStats>('get_memory_token_stats', {
		typeFilter: typeFilter
	});
}
