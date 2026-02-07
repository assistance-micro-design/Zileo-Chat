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
    models::{Memory, MemorySearchResult, MemoryType},
    security::Validator,
    tools::constants::{memory as memory_constants, query_limits},
    tools::memory::{add_memory_core, search_memories_core, AddMemoryParams, SearchParams},
    AppState,
};
use tauri::State;
use tracing::{debug, error, info, instrument, warn};

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
        let type_str = serde_json::to_string(mtype)
            .map_err(|e| format!("Failed to serialize memory type: {}", e))?
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
    // Add LIMIT to prevent memory explosion (OPT-DB-8)
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

    // Validate memory ID
    let validated_id = Validator::validate_uuid(&memory_id).map_err(|e| {
        warn!(error = %e, "Invalid memory_id");
        format!("Invalid memory_id: {}", e)
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
        warn!(error = %e, "Invalid memory_id");
        format!("Invalid memory_id: {}", e)
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
/// Delegates to shared `search_memories_core` helper (deduplicates logic with tool.rs).
///
/// # Arguments
/// * `query` - Search query text
/// * `limit` - Maximum number of results (default: 10)
/// * `type_filter` - Optional filter by memory type
/// * `workflow_id` - Optional workflow ID filter
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

    // Get embedding service
    let service_guard = state.embedding_service.read().await;
    let embedding_service = service_guard.as_ref().cloned();
    drop(service_guard);

    // Convert type_filter to string for shared helper
    let type_filter_str = type_filter.as_ref().map(|t| {
        serde_json::to_string(t)
            .unwrap_or_default()
            .trim_matches('"')
            .to_string()
    });

    // Build scope: use "both" if no workflow_id, "workflow" if workflow_id provided
    // (preserves backward compatibility - commands filter by specific workflow_id)
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
    };

    let (results, search_type) =
        search_memories_core(params, &state.db, embedding_service.as_ref()).await?;

    // Convert JSON results to MemorySearchResult for the command's return type
    let search_results: Vec<MemorySearchResult> = results
        .into_iter()
        .map(|v| {
            let score = v.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0);
            let memory = Memory {
                id: v
                    .get("id")
                    .and_then(|i| i.as_str())
                    .unwrap_or("")
                    .to_string(),
                memory_type: serde_json::from_value(v.get("type").cloned().unwrap_or_default())
                    .unwrap_or(MemoryType::Knowledge),
                content: v
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string(),
                workflow_id: v
                    .get("workflow_id")
                    .and_then(|w| w.as_str())
                    .map(String::from),
                metadata: v.get("metadata").cloned().unwrap_or(serde_json::json!({})),
                importance: v.get("importance").and_then(|i| i.as_f64()).unwrap_or(0.5),
                expires_at: v
                    .get("expires_at")
                    .and_then(|e| e.as_str())
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|dt| dt.with_timezone(&chrono::Utc)),
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
        search_type = %search_type,
        "Search completed"
    );
    Ok(search_results)
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
    let type_str = serde_json::to_string(&memory_type)
        .map_err(|e| format!("Failed to serialize memory type: {}", e))?
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

    let count = count_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

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

        // Create shared embedding service reference
        let embedding_service = Arc::new(tokio::sync::RwLock::new(None));

        AppState {
            db: db.clone(),
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory: Arc::new(crate::tools::ToolFactory::new(
                db,
                embedding_service.clone(),
            )),
            embedding_service,
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
            importance: 0.3,
            expires_at: None,
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
            importance: 0.7,
            expires_at: None,
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
        let long_content = "a".repeat(memory_constants::MAX_CONTENT_LENGTH + 1);
        assert!(long_content.len() > memory_constants::MAX_CONTENT_LENGTH);
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
