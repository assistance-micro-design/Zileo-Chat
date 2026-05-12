// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Memory commands for RAG and context persistence.
//!
//! Provides Tauri commands for managing memory entries used for
//! context retention and knowledge base operations.
//!
//! Note: This is a stub implementation without vector embeddings.
//! Full RAG with embeddings will be implemented in a future phase.

use crate::{
    constants::query_limits,
    db::extract_count,
    db::queries::cleanup,
    models::{ChunkSearchResult, Memory, MemoryType},
    security::{serialize_for_query, validate_uuid_field},
    tools::constants::memory as memory_constants,
    tools::memory::{add_memory_core, search_memories_core, AddMemoryParams, SearchParams},
    AppState,
};
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{debug, error, info, instrument, warn};

/// Result returned by `purge_expired_memories` to the frontend.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredResult {
    /// Number of parent `memory` rows removed.
    pub memories_purged: usize,
    /// Number of `memory_chunk` rows removed alongside their parents.
    pub chunks_purged: usize,
}

/// Adds a new memory entry with automatic embedding generation.
///
/// Uses the shared `add_memory_core` helper for the core creation logic.
/// This command handles Tauri-specific concerns:
/// - Parameter extraction from State
/// - Content validation (trim, empty check, length check)
/// - Embedding service access from shared state
///
/// # Arguments
/// * `memory_type` - Type of memory content
/// * `content` - Text content of the memory
/// * `metadata` - Additional metadata
/// * `workflow_id` - Optional workflow ID for scoped memories (None = general)
///
/// # Returns
/// The ID of the created memory
///
/// # Embedding Behavior
/// If an EmbeddingService is configured, the memory will be stored with
/// a vector embedding enabling semantic search. Otherwise, only text-based
/// search will be available.
#[tauri::command]
#[instrument(
    name = "add_memory",
    skip(state, content, metadata),
    fields(memory_type = ?memory_type, content_len = content.len(), workflow_id = ?workflow_id)
)]
pub async fn add_memory(
    memory_type: MemoryType,
    content: String,
    metadata: Option<serde_json::Value>,
    workflow_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Adding memory entry");

    // Validate content (Tauri command specific validation)
    let trimmed_content = content.trim();
    if trimmed_content.is_empty() {
        return Err("Memory content cannot be empty".to_string());
    }
    if trimmed_content.len() > memory_constants::MAX_CONTENT_LENGTH {
        return Err(format!(
            "Memory content exceeds maximum length of {} characters",
            memory_constants::MAX_CONTENT_LENGTH
        ));
    }

    // Try to get embedding service from shared state
    let service_guard = state.embedding_service.read().await;
    let embedding_service = service_guard.as_ref().cloned();
    drop(service_guard);

    // Prepare parameters for shared helper
    let params = AddMemoryParams {
        memory_type,
        content: trimmed_content.to_string(),
        metadata: metadata.unwrap_or(serde_json::json!({})),
        workflow_id: workflow_id.clone(),
        importance: memory_constants::DEFAULT_IMPORTANCE,
        expires_at: None,
    };

    // Use shared helper for core creation logic
    let result = add_memory_core(params, &state.db, embedding_service.as_ref()).await?;

    info!(
        memory_id = %result.memory_id,
        embedding_generated = result.embedding_generated,
        workflow_id = ?workflow_id,
        "Memory entry created successfully"
    );
    Ok(result.memory_id)
}

/// Lists memory entries with optional type and workflow filters.
///
/// # Arguments
/// * `type_filter` - Optional filter by memory type
/// * `workflow_id` - Optional workflow ID filter:
///   - `Some(id)`: Only memories scoped to this workflow
///   - `None`: All memories (both workflow-scoped and general)
///
/// # Returns
/// Vector of memory entries sorted by creation time (newest first)
#[tauri::command]
#[instrument(name = "list_memories", skip(state), fields(type_filter = ?type_filter, workflow_id = ?workflow_id))]
pub async fn list_memories(
    type_filter: Option<MemoryType>,
    workflow_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Memory>, String> {
    info!("Loading memories");

    // Build WHERE conditions and parameters
    let mut conditions: Vec<String> = Vec::new();
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    // Type filter condition (use bind parameter)
    if let Some(ref mtype) = type_filter {
        let type_str = serialize_for_query(mtype, "memory type")?
            .trim_matches('"')
            .to_string();
        conditions.push("type = $type".to_string());
        params.push(("type".to_string(), serde_json::json!(type_str)));
    }

    // Workflow scope condition (use bind parameter)
    if let Some(ref wf_id) = workflow_id {
        conditions.push("workflow_id = $workflow_id".to_string());
        params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
    }

    // Build the query
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    // Add LIMIT to prevent memory explosion
    let query = format!(
        "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
         FROM memory{} ORDER BY created_at DESC LIMIT {}",
        where_clause,
        query_limits::DEFAULT_LIST_LIMIT
    );

    // Use parameterized query if we have parameters, otherwise standard query
    let memories: Vec<Memory> = if params.is_empty() {
        state.db.query(&query).await.map_err(|e| {
            error!(error = %e, "Failed to load memories");
            format!("Failed to load memories: {}", e)
        })?
    } else {
        state
            .db
            .query_with_params(&query, params)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to load memories");
                format!("Failed to load memories: {}", e)
            })?
    };

    debug!(
        count = memories.len(),
        workflow_id = ?workflow_id,
        type_filter = ?type_filter,
        "Memories loaded"
    );
    Ok(memories)
}

/// Gets a single memory entry by ID.
///
/// # Arguments
/// * `memory_id` - The memory ID to retrieve
///
/// # Returns
/// The memory entry if found
#[tauri::command]
#[instrument(name = "get_memory", skip(state), fields(memory_id = %memory_id))]
pub async fn get_memory(memory_id: String, state: State<'_, AppState>) -> Result<Memory, String> {
    info!("Getting memory entry");

    let validated_id = validate_uuid_field(&memory_id, "memory_id")?;

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    let memories: Vec<Memory> = state
        .db
        .query(&format!(
            "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
             FROM memory WHERE meta::id(id) = '{}'",
            validated_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to get memory");
            format!("Failed to get memory: {}", e)
        })?;

    memories.into_iter().next().ok_or_else(|| {
        warn!(memory_id = %validated_id, "Memory not found");
        "Memory not found".to_string()
    })
}

/// Deletes a memory entry along with its child chunks (cascade).
///
/// Order matters: chunks are deleted first so no orphan rows remain if the
/// second DELETE fails. The DELETE on `memory` is idempotent (no-op when the
/// row is already gone) and matches the prior contract.
///
/// # Arguments
/// * `memory_id` - The memory ID to delete
#[tauri::command]
#[instrument(name = "delete_memory", skip(state), fields(memory_id = %memory_id))]
pub async fn delete_memory(memory_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting memory entry (with cascade on memory_chunk)");

    let validated_id = validate_uuid_field(&memory_id, "memory_id")?;

    // Cascade: drop the chunks first.
    let cascade_query = format!(
        "DELETE FROM memory_chunk WHERE memory_id = memory:`{}`",
        validated_id
    );
    state.db.execute(&cascade_query).await.map_err(|e| {
        error!(error = %e, "Failed to cascade-delete memory_chunk rows");
        format!("Failed to delete memory chunks: {}", e)
    })?;

    let delete_query = format!("DELETE memory:`{}`", validated_id);
    state.db.execute(&delete_query).await.map_err(|e| {
        error!(error = %e, "Failed to delete memory");
        format!("Failed to delete memory: {}", e)
    })?;

    info!("Memory entry deleted successfully");
    Ok(())
}

/// Searches memories using semantic similarity (vector) with a text fallback.
///
/// Returns one row per matching chunk. `parent_memory_id` is the UUID an
/// agent passes to `operation=get` to read the full parent content. Several
/// chunks may share the same `parent_memory_id` (deduplication is left to
/// the agent — keeps the V1 contract simple).
///
/// # Arguments
/// * `query` - Search query text
/// * `limit` - Maximum number of results (default: 10)
/// * `type_filter` - Optional filter by memory type
/// * `workflow_id` - Optional workflow ID filter
/// * `threshold` - Similarity threshold 0-1 for vector search (default: 0.7)
/// * `tags_filter` - When set, only return results whose parent memory has
///   at least ONE of these tags (CONTAINSANY semantics)
#[tauri::command]
#[instrument(
    name = "search_memories",
    skip(state, query),
    fields(query_len = query.len(), limit = ?limit, type_filter = ?type_filter, workflow_id = ?workflow_id, tags_filter = ?tags_filter)
)]
pub async fn search_memories(
    query: String,
    limit: Option<usize>,
    type_filter: Option<MemoryType>,
    workflow_id: Option<String>,
    threshold: Option<f64>,
    tags_filter: Option<Vec<String>>,
    state: State<'_, AppState>,
) -> Result<Vec<ChunkSearchResult>, String> {
    info!("Searching memories");

    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let result_limit = limit.unwrap_or(10).min(100);
    let similarity_threshold = threshold.unwrap_or(0.7).clamp(0.0, 1.0);

    let service_guard = state.embedding_service.read().await;
    let embedding_service = service_guard.as_ref().cloned();
    drop(service_guard);

    let type_filter_str = type_filter.as_ref().map(|t| {
        serde_json::to_string(t)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string()
    });

    // "workflow" when caller pinned an id, "both" otherwise (matches the
    // backward-compatible behavior of the previous command shape).
    let scope = if workflow_id.is_some() {
        "workflow".to_string()
    } else {
        "both".to_string()
    };

    let params = SearchParams {
        query_text: trimmed_query.to_string(),
        limit: result_limit,
        type_filter: type_filter_str,
        workflow_id,
        scope,
        threshold: similarity_threshold,
        tags_filter,
    };

    let (results, search_type) =
        search_memories_core(params, &state.db, embedding_service.as_ref()).await?;

    let search_results: Vec<ChunkSearchResult> = results
        .into_iter()
        .map(|v| chunk_result_from_json(&v, &search_type))
        .collect();

    debug!(
        count = search_results.len(),
        search_type = %search_type,
        "Search completed"
    );
    Ok(search_results)
}

/// Deserializes one row from the search core into a typed
/// [`ChunkSearchResult`]. Missing fields fall back to sane defaults so a
/// shape-shift in either text or vector path stays survivable.
fn chunk_result_from_json(v: &serde_json::Value, default_search_type: &str) -> ChunkSearchResult {
    let parse_dt = |key: &str| {
        v.get(key)
            .and_then(|x| x.as_str())
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
    };
    ChunkSearchResult {
        chunk_id: v
            .get("chunk_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        parent_memory_id: v
            .get("parent_memory_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        chunk_index: v.get("chunk_index").and_then(|x| x.as_u64()).unwrap_or(0) as usize,
        chunk_count: v.get("chunk_count").and_then(|x| x.as_u64()).unwrap_or(1) as usize,
        content: v
            .get("content")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        memory_type: serde_json::from_value(v.get("memory_type").cloned().unwrap_or_default())
            .unwrap_or(MemoryType::Knowledge),
        workflow_id: v
            .get("workflow_id")
            .and_then(|x| x.as_str())
            .map(String::from),
        metadata: v.get("metadata").cloned().unwrap_or(serde_json::json!({})),
        importance: v.get("importance").and_then(|x| x.as_f64()).unwrap_or(0.5),
        expires_at: parse_dt("expires_at"),
        created_at: parse_dt("created_at").unwrap_or_else(chrono::Utc::now),
        score: v.get("score").and_then(|x| x.as_f64()).unwrap_or(0.0),
        cosine_score: v
            .get("cosine_score")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0),
        search_type: v
            .get("search_type")
            .and_then(|x| x.as_str())
            .unwrap_or(default_search_type)
            .to_string(),
    }
}

/// Clears all memories of a specific type.
///
/// # Arguments
/// * `memory_type` - Type of memories to clear
///
/// # Returns
/// Number of memories deleted
#[tauri::command]
#[instrument(name = "clear_memories_by_type", skip(state), fields(memory_type = ?memory_type))]
pub async fn clear_memories_by_type(
    memory_type: MemoryType,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    info!("Clearing memories by type");

    // Convert MemoryType to string for bind parameter
    let type_str = serialize_for_query(&memory_type, "memory type")?
        .trim_matches('"')
        .to_string();

    // First count how many will be deleted using parameterized query
    let count_result: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            "SELECT count() FROM memory WHERE type = $type GROUP ALL",
            vec![("type".to_string(), serde_json::json!(type_str))],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to count memories");
            format!("Failed to count memories: {}", e)
        })?;

    let count = extract_count(&count_result) as usize;

    // Cascade: drop chunks whose parent matches the type filter BEFORE the
    // parent rows themselves, to prevent orphan rows on partial failure.
    // `SELECT VALUE id` returns the raw record values, not `{id: ...}` wrappers.
    state
        .db
        .execute_with_params(
            "DELETE FROM memory_chunk WHERE memory_id IN \
             (SELECT VALUE id FROM memory WHERE type = $type)",
            vec![("type".to_string(), serde_json::json!(type_str))],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to cascade-clear memory_chunk");
            format!("Failed to clear memory chunks: {}", e)
        })?;

    // Delete all memories of the specified type using parameterized query
    state
        .db
        .execute_with_params(
            "DELETE FROM memory WHERE type = $type",
            vec![("type".to_string(), serde_json::json!(type_str))],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to clear memories");
            format!("Failed to clear memories: {}", e)
        })?;

    info!(count = count, "Memories cleared successfully");
    Ok(count)
}

/// Removes memories whose `expires_at` is in the past plus their chunks.
///
/// Memories without `expires_at` or with a future TTL are left untouched.
/// Exposed for UI-triggered cleanup; the same helper is also called at
/// startup from `AppState::new` so this is just a manual top-up.
#[tauri::command]
#[instrument(name = "purge_expired_memories", skip(state))]
pub async fn purge_expired_memories(
    state: State<'_, AppState>,
) -> Result<PurgeExpiredResult, String> {
    info!("Purging expired memories on user request");
    let stats = cleanup::purge_expired_memories(&state.db).await;
    Ok(PurgeExpiredResult {
        memories_purged: stats.memories_purged,
        chunks_purged: stats.chunks_purged,
    })
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
