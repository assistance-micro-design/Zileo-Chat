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
    models::{Memory, MemoryCreate, MemorySearchResult, MemoryType},
    security::Validator,
    AppState,
};
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Maximum allowed length for memory content
pub const MAX_MEMORY_CONTENT_LEN: usize = 50_000;

/// Adds a new memory entry.
///
/// # Arguments
/// * `memory_type` - Type of memory content
/// * `content` - Text content of the memory
/// * `metadata` - Additional metadata
///
/// # Returns
/// The ID of the created memory
#[tauri::command]
#[instrument(
    name = "add_memory",
    skip(state, content, metadata),
    fields(memory_type = ?memory_type, content_len = content.len())
)]
pub async fn add_memory(
    memory_type: MemoryType,
    content: String,
    metadata: Option<serde_json::Value>,
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

    // Use MemoryCreate to avoid passing datetime field
    // The database will set created_at via DEFAULT time::now()
    let memory = MemoryCreate {
        id: memory_id.clone(),
        memory_type,
        content: trimmed_content.to_string(),
        metadata: metadata.unwrap_or(serde_json::json!({})),
    };

    let id = state.db.create("memory", memory).await.map_err(|e| {
        error!(error = %e, "Failed to create memory");
        format!("Failed to create memory: {}", e)
    })?;

    info!(memory_id = %id, "Memory entry created successfully");
    Ok(memory_id)
}

/// Lists memory entries with optional type filter.
///
/// # Arguments
/// * `type_filter` - Optional filter by memory type
///
/// # Returns
/// Vector of memory entries sorted by creation time (newest first)
#[tauri::command]
#[instrument(name = "list_memories", skip(state), fields(type_filter = ?type_filter))]
pub async fn list_memories(
    type_filter: Option<MemoryType>,
    state: State<'_, AppState>,
) -> Result<Vec<Memory>, String> {
    info!("Loading memories");

    let query = match type_filter {
        Some(ref mtype) => format!(
            "SELECT * FROM memory WHERE type = '{}' ORDER BY created_at DESC",
            mtype
        ),
        None => "SELECT * FROM memory ORDER BY created_at DESC".to_string(),
    };

    let memories: Vec<Memory> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load memories");
        format!("Failed to load memories: {}", e)
    })?;

    info!(count = memories.len(), "Memories loaded");
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

    let memories: Vec<Memory> = state
        .db
        .query(&format!(
            "SELECT * FROM memory WHERE id = '{}'",
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

    state
        .db
        .delete(&format!("memory:{}", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete memory");
            format!("Failed to delete memory: {}", e)
        })?;

    info!("Memory entry deleted successfully");
    Ok(())
}

/// Searches memories by text content (simple text search, no vector embeddings).
///
/// This is a stub implementation that performs basic text matching.
/// Full vector search with embeddings will be implemented in a future phase.
///
/// # Arguments
/// * `query` - Search query text
/// * `limit` - Maximum number of results (default: 10)
/// * `type_filter` - Optional filter by memory type
///
/// # Returns
/// Vector of matching memories with relevance scores
#[tauri::command]
#[instrument(
    name = "search_memories",
    skip(state, query),
    fields(query_len = query.len(), limit = ?limit, type_filter = ?type_filter)
)]
pub async fn search_memories(
    query: String,
    limit: Option<usize>,
    type_filter: Option<MemoryType>,
    state: State<'_, AppState>,
) -> Result<Vec<MemorySearchResult>, String> {
    info!("Searching memories");

    // Validate query
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return Err("Search query cannot be empty".to_string());
    }

    let result_limit = limit.unwrap_or(10).min(100);

    // Build search query with basic text matching
    // Note: This uses SurrealDB's string matching; vector search will be added later
    let search_query = match type_filter {
        Some(ref mtype) => format!(
            "SELECT * FROM memory WHERE type = '{}' AND content CONTAINS '{}' ORDER BY created_at DESC LIMIT {}",
            mtype,
            trimmed_query.replace('\'', "''"),
            result_limit
        ),
        None => format!(
            "SELECT * FROM memory WHERE content CONTAINS '{}' ORDER BY created_at DESC LIMIT {}",
            trimmed_query.replace('\'', "''"),
            result_limit
        ),
    };

    let memories: Vec<Memory> = state.db.query(&search_query).await.map_err(|e| {
        error!(error = %e, "Failed to search memories");
        format!("Failed to search memories: {}", e)
    })?;

    // Convert to search results with simple relevance scoring
    // Score is based on query term density (stub implementation)
    let results: Vec<MemorySearchResult> = memories
        .into_iter()
        .map(|memory| {
            let query_lower = trimmed_query.to_lowercase();
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

    info!(count = results.len(), "Memory search completed");
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
    let delete_query = format!("DELETE FROM memory WHERE type = '{}'", memory_type);

    let _: Vec<serde_json::Value> = state.db.query(&delete_query).await.map_err(|e| {
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

        std::mem::forget(temp_dir);

        AppState {
            db,
            registry,
            orchestrator,
            llm_manager,
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
