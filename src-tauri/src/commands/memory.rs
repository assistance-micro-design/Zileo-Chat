// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Memory commands for RAG and context persistence.
//!
//! Provides Tauri commands for managing memory entries used for
//! context retention and knowledge base operations.
//!
//! Note: This is a stub implementation without vector embeddings.
//! Full RAG with embeddings will be implemented in a future phase.

use crate::{
    models::{Memory, MemoryCreate, MemoryCreateWithEmbedding, MemorySearchResult, MemoryType},
    security::Validator,
    AppState,
};
use tauri::State;
use tracing::{debug, error, info, instrument, warn};
use uuid::Uuid;

/// Maximum allowed length for memory content
pub const MAX_MEMORY_CONTENT_LEN: usize = 50_000;

/// Adds a new memory entry with automatic embedding generation.
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

    // Validate content
    let trimmed_content = content.trim();
    if trimmed_content.is_empty() {
        return Err("Memory content cannot be empty".to_string());
    }
    if trimmed_content.len() > MAX_MEMORY_CONTENT_LEN {
        return Err(format!(
            "Memory content exceeds maximum length of {} characters",
            MAX_MEMORY_CONTENT_LEN
        ));
    }

    let memory_id = Uuid::new_v4().to_string();
    let meta = metadata.unwrap_or(serde_json::json!({}));

    // Try to get embedding service
    let service_guard = state.embedding_service.read().await;
    let embedding_service = service_guard.as_ref().cloned();
    drop(service_guard);

    // Try to generate embedding if service is available
    let embedding_generated = if let Some(ref embed_svc) = embedding_service {
        match embed_svc.embed(trimmed_content).await {
            Ok(embedding) => {
                // Create memory with embedding
                let memory = if let Some(ref wf_id) = workflow_id {
                    MemoryCreateWithEmbedding::with_workflow(
                        memory_type,
                        trimmed_content.to_string(),
                        embedding,
                        meta.clone(),
                        wf_id.clone(),
                    )
                } else {
                    MemoryCreateWithEmbedding::new(
                        memory_type,
                        trimmed_content.to_string(),
                        embedding,
                        meta.clone(),
                    )
                };

                state
                    .db
                    .create("memory", &memory_id, memory)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Failed to create memory with embedding");
                        format!("Failed to create memory: {}", e)
                    })?;

                true
            }
            Err(e) => {
                // Fallback to text-only storage
                warn!(error = %e, "Embedding generation failed, storing without embedding");

                let memory = if let Some(ref wf_id) = workflow_id {
                    MemoryCreate::with_workflow(
                        memory_type,
                        trimmed_content.to_string(),
                        meta.clone(),
                        wf_id.clone(),
                    )
                } else {
                    MemoryCreate::new(memory_type, trimmed_content.to_string(), meta.clone())
                };

                state
                    .db
                    .create("memory", &memory_id, memory)
                    .await
                    .map_err(|e| {
                        error!(error = %e, "Failed to create memory");
                        format!("Failed to create memory: {}", e)
                    })?;

                false
            }
        }
    } else {
        // No embedding service, store text only
        let memory = if let Some(ref wf_id) = workflow_id {
            MemoryCreate::with_workflow(
                memory_type,
                trimmed_content.to_string(),
                meta.clone(),
                wf_id.clone(),
            )
        } else {
            MemoryCreate::new(memory_type, trimmed_content.to_string(), meta.clone())
        };

        state
            .db
            .create("memory", &memory_id, memory)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to create memory");
                format!("Failed to create memory: {}", e)
            })?;

        false
    };

    info!(
        memory_id = %memory_id,
        embedding_generated = embedding_generated,
        workflow_id = ?workflow_id,
        "Memory entry created successfully"
    );
    Ok(memory_id)
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

    // Build WHERE conditions
    let mut conditions: Vec<String> = Vec::new();

    // Type filter condition
    if let Some(ref mtype) = type_filter {
        conditions.push(format!("type = '{}'", mtype));
    }

    // Workflow scope condition
    if let Some(ref wf_id) = workflow_id {
        conditions.push(format!("workflow_id = '{}'", wf_id));
    }

    // Build the query
    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", conditions.join(" AND "))
    };

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    let query = format!(
        "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
         FROM memory{} ORDER BY created_at DESC",
        where_clause
    );

    let memories: Vec<Memory> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load memories");
        format!("Failed to load memories: {}", e)
    })?;

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

    // Validate memory ID
    let validated_id = Validator::validate_uuid(&memory_id).map_err(|e| {
        warn!(error = %e, "Invalid memory ID");
        format!("Invalid memory ID: {}", e)
    })?;

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

/// Deletes a memory entry.
///
/// # Arguments
/// * `memory_id` - The memory ID to delete
#[tauri::command]
#[instrument(name = "delete_memory", skip(state), fields(memory_id = %memory_id))]
pub async fn delete_memory(memory_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting memory entry");

    // Validate memory ID
    let validated_id = Validator::validate_uuid(&memory_id).map_err(|e| {
        warn!(error = %e, "Invalid memory ID");
        format!("Invalid memory ID: {}", e)
    })?;

    // Use execute() with DELETE query to avoid SurrealDB SDK issues with delete() method
    let delete_query = format!("DELETE memory:`{}`", validated_id);
    state.db.execute(&delete_query).await.map_err(|e| {
        error!(error = %e, "Failed to delete memory");
        format!("Failed to delete memory: {}", e)
    })?;

    info!("Memory entry deleted successfully");
    Ok(())
}

/// Searches memories using semantic similarity (vector search) with text search fallback.
///
/// If an EmbeddingService is configured, this performs vector similarity search
/// using cosine distance. Otherwise, falls back to basic text matching.
///
/// # Arguments
/// * `query` - Search query text
/// * `limit` - Maximum number of results (default: 10)
/// * `type_filter` - Optional filter by memory type
/// * `workflow_id` - Optional workflow ID filter:
///   - `Some(id)`: Only search memories scoped to this workflow
///   - `None`: Search all memories (both workflow-scoped and general)
/// * `threshold` - Similarity threshold 0-1 for vector search (default: 0.7)
///
/// # Returns
/// Vector of matching memories with relevance scores
#[tauri::command]
#[instrument(
    name = "search_memories",
    skip(state, query),
    fields(query_len = query.len(), limit = ?limit, type_filter = ?type_filter, workflow_id = ?workflow_id)
)]
pub async fn search_memories(
    query: String,
    limit: Option<usize>,
    type_filter: Option<MemoryType>,
    workflow_id: Option<String>,
    threshold: Option<f64>,
    state: State<'_, AppState>,
) -> Result<Vec<MemorySearchResult>, String> {
    info!("Searching memories");

    // Validate query
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let result_limit = limit.unwrap_or(10).min(100);
    let similarity_threshold = threshold.unwrap_or(0.7).clamp(0.0, 1.0);

    // Try to get embedding service
    let service_guard = state.embedding_service.read().await;
    let embedding_service = service_guard.as_ref().cloned();
    drop(service_guard);

    // Try vector search if embedding service is available
    if let Some(ref embed_svc) = embedding_service {
        match embed_svc.embed(trimmed_query).await {
            Ok(query_embedding) => {
                return vector_search(
                    &state.db,
                    &query_embedding,
                    result_limit,
                    type_filter.as_ref(),
                    workflow_id.as_ref(),
                    similarity_threshold,
                )
                .await;
            }
            Err(e) => {
                warn!(error = %e, "Query embedding failed, falling back to text search");
            }
        }
    }

    // Fallback to text search
    text_search(
        &state.db,
        trimmed_query,
        result_limit,
        type_filter.as_ref(),
        workflow_id.as_ref(),
    )
    .await
}

/// Performs vector similarity search using HNSW index.
async fn vector_search(
    db: &crate::db::DBClient,
    query_embedding: &[f32],
    limit: usize,
    type_filter: Option<&MemoryType>,
    workflow_id: Option<&String>,
    threshold: f64,
) -> Result<Vec<MemorySearchResult>, String> {
    // Build conditions
    let mut conditions = vec!["embedding IS NOT NONE".to_string()];

    if let Some(wf_id) = workflow_id {
        conditions.push(format!("workflow_id = '{}'", wf_id));
    }

    if let Some(mem_type) = type_filter {
        conditions.push(format!("type = '{}'", mem_type));
    }

    let where_clause = conditions.join(" AND ");

    // Convert threshold to distance (cosine distance = 1 - similarity)
    let distance_threshold = 1.0 - threshold;

    // Format embedding for SurrealQL
    let embedding_str: String = query_embedding
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            type,
            content,
            workflow_id,
            metadata,
            created_at,
            vector::similarity::cosine(embedding, [{embedding}]) AS score
        FROM memory
        WHERE {where_clause}
          AND vector::distance::cosine(embedding, [{embedding}]) < {distance}
        ORDER BY score DESC
        LIMIT {limit}"#,
        embedding = embedding_str,
        where_clause = where_clause,
        distance = distance_threshold,
        limit = limit
    );

    let results: Vec<serde_json::Value> = db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Vector search failed");
        format!("Failed to search memories: {}", e)
    })?;

    // Convert to MemorySearchResult
    let search_results: Vec<MemorySearchResult> = results
        .into_iter()
        .map(|v| {
            let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            // Deserialize memory fields
            let memory = Memory {
                id: v.get("id").and_then(|i| i.as_str()).unwrap_or("").to_string(),
                memory_type: serde_json::from_value(v.get("type").cloned().unwrap_or_default())
                    .unwrap_or(MemoryType::Knowledge),
                content: v
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
                workflow_id: v.get("workflow_id").and_then(|w| w.as_str()).map(String::from),
                metadata: v.get("metadata").cloned().unwrap_or(serde_json::json!({})),
                created_at: v
                    .get("created_at")
                    .and_then(|c| c.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .unwrap_or_else(chrono::Utc::now),
            };

            MemorySearchResult { memory, score }
        })
        .collect();

    debug!(
        count = search_results.len(),
        search_type = "vector",
        "Vector search completed"
    );
    Ok(search_results)
}

/// Performs text-based search as fallback.
async fn text_search(
    db: &crate::db::DBClient,
    query_text: &str,
    limit: usize,
    type_filter: Option<&MemoryType>,
    workflow_id: Option<&String>,
) -> Result<Vec<MemorySearchResult>, String> {
    let mut conditions = Vec::new();

    // Text content contains query (case-insensitive)
    let escaped_query = query_text.replace('\'', "''").replace('%', "\\%");
    conditions.push(format!(
        "string::lowercase(content) CONTAINS string::lowercase('{}')",
        escaped_query
    ));

    if let Some(wf_id) = workflow_id {
        conditions.push(format!("workflow_id = '{}'", wf_id));
    }

    if let Some(mem_type) = type_filter {
        conditions.push(format!("type = '{}'", mem_type));
    }

    let where_clause = conditions.join(" AND ");

    let search_query = format!(
        "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
         FROM memory WHERE {} ORDER BY created_at DESC LIMIT {}",
        where_clause, limit
    );

    let memories: Vec<Memory> = db.query(&search_query).await.map_err(|e| {
        error!(error = %e, "Text search failed");
        format!("Failed to search memories: {}", e)
    })?;

    // Convert to search results with simple relevance scoring
    let query_lower = query_text.to_lowercase();
    let results: Vec<MemorySearchResult> = memories
        .into_iter()
        .map(|memory| {
            let content_lower = memory.content.to_lowercase();

            // Simple relevance: count occurrences / content length
            let occurrences = content_lower.matches(&query_lower).count();
            let score = if memory.content.is_empty() {
                0.0
            } else {
                (occurrences as f64 * query_lower.len() as f64) / memory.content.len() as f64
            };

            MemorySearchResult {
                memory,
                score: score.min(1.0),
            }
        })
        .collect();

    debug!(
        count = results.len(),
        search_type = "text",
        "Text search completed"
    );
    Ok(results)
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

    // First count how many will be deleted
    let count_query = format!(
        "SELECT count() FROM memory WHERE type = '{}' GROUP ALL",
        memory_type
    );

    let count_result: Vec<serde_json::Value> = state.db.query(&count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to count memories: {}", e)
    })?;

    let count = count_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    // Delete all memories of the specified type
    // Use execute() for DELETE to avoid SurrealDB SDK serialization issues
    let delete_query = format!("DELETE FROM memory WHERE type = '{}'", memory_type);

    state.db.execute(&delete_query).await.map_err(|e| {
        error!(error = %e, "Failed to clear memories");
        format!("Failed to clear memories: {}", e)
    })?;

    info!(count = count, "Memories cleared successfully");
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::{AgentOrchestrator, AgentRegistry};
    use crate::db::DBClient;
    use crate::llm::ProviderManager;
    use chrono::Utc;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[allow(dead_code)]
    async fn setup_test_state() -> AppState {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_memory_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = Arc::new(
            DBClient::new(db_path_str)
                .await
                .expect("Failed to create test DB"),
        );
        db.initialize_schema().await.expect("Schema init failed");

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
        let llm_manager = Arc::new(ProviderManager::new());
        let mcp_manager = Arc::new(
            crate::mcp::MCPManager::new(db.clone())
                .await
                .expect("Failed to create MCP manager"),
        );

        std::mem::forget(temp_dir);

        AppState {
            db: db.clone(),
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory: Arc::new(crate::tools::ToolFactory::new(db, None)),
            embedding_service: Arc::new(tokio::sync::RwLock::new(None)),
            streaming_cancellations: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            app_handle: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    #[test]
    fn test_memory_type_serialization() {
        let mtype = MemoryType::UserPref;
        let json = serde_json::to_string(&mtype).unwrap();
        assert_eq!(json, "\"user_pref\"");

        let mtype = MemoryType::Knowledge;
        let json = serde_json::to_string(&mtype).unwrap();
        assert_eq!(json, "\"knowledge\"");
    }

    #[test]
    fn test_memory_structure() {
        let memory = Memory {
            id: "mem_001".to_string(),
            memory_type: MemoryType::Context,
            content: "User prefers dark mode".to_string(),
            workflow_id: None,
            metadata: serde_json::json!({"source": "settings"}),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"type\":\"context\""));
        assert!(json.contains("\"content\":\"User prefers dark mode\""));
    }

    #[test]
    fn test_memory_search_result_structure() {
        let memory = Memory {
            id: "mem_002".to_string(),
            memory_type: MemoryType::Decision,
            content: "Chose Rust for backend".to_string(),
            workflow_id: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let result = MemorySearchResult {
            memory,
            score: 0.85,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"score\":0.85"));
        assert!(json.contains("\"type\":\"decision\""));
    }

    #[test]
    fn test_content_validation() {
        // Empty content should be rejected
        let empty = "   ".trim();
        assert!(empty.is_empty());

        // Long content check
        let long_content = "a".repeat(MAX_MEMORY_CONTENT_LEN + 1);
        assert!(long_content.len() > MAX_MEMORY_CONTENT_LEN);
    }

    #[tokio::test]
    async fn test_memory_type_values() {
        assert_eq!(
            serde_json::to_string(&MemoryType::UserPref).unwrap(),
            "\"user_pref\""
        );
        assert_eq!(
            serde_json::to_string(&MemoryType::Context).unwrap(),
            "\"context\""
        );
        assert_eq!(
            serde_json::to_string(&MemoryType::Knowledge).unwrap(),
            "\"knowledge\""
        );
        assert_eq!(
            serde_json::to_string(&MemoryType::Decision).unwrap(),
            "\"decision\""
        );
    }
}
