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

//! Shared helpers for memory operations.
//!
//! This module contains common logic used by both the MemoryTool (agent tool)
//! and the memory Tauri commands. It eliminates code duplication for
//! add_memory, search, and describe operations.

use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::MemoryDescribeResult;
use crate::models::{Memory, MemoryCreate, MemoryCreateWithEmbedding, MemoryType};
use crate::tools::constants::memory as mem_constants;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Parameters for adding a memory entry.
///
/// This struct contains pre-validated parameters. Callers are responsible for:
/// - Validating content is not empty and within length limits
/// - Validating memory_type is valid
/// - Enriching metadata as needed (e.g., adding agent_source)
#[derive(Debug, Clone)]
pub struct AddMemoryParams {
    /// Type of memory (already validated)
    pub memory_type: MemoryType,
    /// Content text (already validated - not empty, within limits)
    pub content: String,
    /// Metadata (may be enriched by caller with agent_source, tags, etc.)
    pub metadata: serde_json::Value,
    /// Optional workflow ID for scoped memories
    pub workflow_id: Option<String>,
    /// Importance score (0.0-1.0)
    pub importance: f64,
    /// Optional expiration timestamp for TTL
    pub expires_at: Option<DateTime<Utc>>,
}

/// Result of adding a memory entry.
#[derive(Debug, Clone)]
pub struct AddMemoryResult {
    /// The UUID of the created memory
    pub memory_id: String,
    /// Whether an embedding was successfully generated
    pub embedding_generated: bool,
}

/// Core logic for adding a memory with optional embedding.
///
/// This function handles the common pattern of:
/// 1. Generating UUID
/// 2. Attempting embedding generation (if service available)
/// 3. Falling back to text-only storage on embedding failure
/// 4. Creating the database record
///
/// # Arguments
/// * `params` - Pre-validated memory parameters
/// * `db` - Database client
/// * `embedding_service` - Optional embedding service
///
/// # Returns
/// * `Ok(AddMemoryResult)` with memory_id and embedding status
/// * `Err(String)` with error message on database failure
///
/// # Example
/// ```rust,ignore
/// let params = AddMemoryParams {
///     memory_type: MemoryType::Knowledge,
///     content: "Important fact".to_string(),
///     metadata: json!({}),
///     workflow_id: None,
/// };
/// let result = add_memory_core(params, &db, embedding_service.as_ref()).await?;
/// ```
pub async fn add_memory_core(
    params: AddMemoryParams,
    db: &DBClient,
    embedding_service: Option<&Arc<EmbeddingService>>,
) -> Result<AddMemoryResult, String> {
    let memory_id = Uuid::new_v4().to_string();

    let embedding_generated = if let Some(embed_svc) = embedding_service {
        match embed_svc.embed(&params.content).await {
            Ok(embedding) => {
                // Create memory with embedding using unified builder
                let memory = MemoryCreateWithEmbedding::build(
                    params.memory_type.clone(),
                    params.content.clone(),
                    embedding,
                    params.metadata.clone(),
                    params.workflow_id.clone(),
                    params.importance,
                    params.expires_at,
                );

                db.create("memory", &memory_id, memory)
                    .await
                    .map_err(|e| format!("Failed to create memory with embedding: {}", e))?;

                // Set expires_at separately (SurrealDB datetime cast)
                set_expires_at_if_present(db, &memory_id, params.expires_at).await?;

                true
            }
            Err(e) => {
                // Fallback to text-only storage
                warn!(error = %e, "Embedding generation failed, storing without embedding");

                create_memory_without_embedding(db, &memory_id, &params).await?;
                false
            }
        }
    } else {
        // No embedding service, store text only
        create_memory_without_embedding(db, &memory_id, &params).await?;
        false
    };

    info!(
        memory_id = %memory_id,
        memory_type = %params.memory_type,
        embedding = embedding_generated,
        workflow_id = ?params.workflow_id,
        "Memory created via helper"
    );

    Ok(AddMemoryResult {
        memory_id,
        embedding_generated,
    })
}

/// Helper to create a memory record without embedding.
async fn create_memory_without_embedding(
    db: &DBClient,
    memory_id: &str,
    params: &AddMemoryParams,
) -> Result<(), String> {
    let memory = MemoryCreate::build(
        params.memory_type.clone(),
        params.content.clone(),
        params.metadata.clone(),
        params.workflow_id.clone(),
        params.importance,
        params.expires_at,
    );

    db.create("memory", memory_id, memory)
        .await
        .map_err(|e| format!("Failed to create memory: {}", e))?;

    // Set expires_at separately using datetime cast (SurrealDB rejects ISO strings for datetime fields)
    set_expires_at_if_present(db, memory_id, params.expires_at).await?;

    Ok(())
}

/// Sets expires_at on a memory record if a value is provided.
///
/// SurrealDB SCHEMAFULL tables reject ISO 8601 strings for `option<datetime>` fields
/// when passed via JSON CONTENT. This helper uses a `<datetime>` cast in the UPDATE query.
async fn set_expires_at_if_present(
    db: &DBClient,
    memory_id: &str,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(), String> {
    if let Some(expires) = expires_at {
        let query = format!(
            "UPDATE memory:`{}` SET expires_at = <datetime>$expires_at",
            memory_id
        );
        db.execute_with_params(
            &query,
            vec![(
                "expires_at".to_string(),
                serde_json::json!(expires.to_rfc3339()),
            )],
        )
        .await
        .map_err(|e| format!("Failed to set expires_at: {}", e))?;
    }
    Ok(())
}

// =============================================================================
// Shared search helpers (deduplicates logic between tool.rs and commands/memory.rs)
// =============================================================================

/// Parameters for searching memories.
#[derive(Debug, Clone)]
pub struct SearchParams {
    /// Search query text
    pub query_text: String,
    /// Maximum number of results
    pub limit: usize,
    /// Optional type filter
    pub type_filter: Option<String>,
    /// Optional workflow ID for scope filtering
    pub workflow_id: Option<String>,
    /// Scope: "workflow", "general", or "both"
    pub scope: String,
    /// Similarity threshold (0-1)
    pub threshold: f64,
}

/// Builds the scope condition for WHERE clause.
///
/// Returns `Some(condition)` to add to WHERE clause, or `None` if no condition needed.
/// When workflow_id is needed, it adds a parameter to the params vector.
pub fn build_scope_condition(
    scope: &str,
    workflow_id: &Option<String>,
    params: &mut Vec<(String, serde_json::Value)>,
) -> Option<String> {
    match scope {
        "workflow" => workflow_id.as_ref().map(|wf_id| {
            params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
            "workflow_id = $workflow_id".to_string()
        }),
        "general" => Some("workflow_id IS NONE".to_string()),
        // "both" or any other value - include both workflow and general
        _ => workflow_id.as_ref().map(|wf_id| {
            params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
            "(workflow_id = $workflow_id OR workflow_id IS NONE)".to_string()
        }),
    }
}

/// Builds the expiration filter for WHERE clause.
pub fn expiration_filter() -> String {
    "(expires_at IS NONE OR expires_at > time::now())".to_string()
}

/// Searches memories using semantic similarity with text search fallback.
///
/// If an EmbeddingService is available, attempts vector search first.
/// Falls back to text search on embedding failure or unavailability.
pub async fn search_memories_core(
    params: SearchParams,
    db: &DBClient,
    embedding_service: Option<&Arc<EmbeddingService>>,
) -> Result<(Vec<serde_json::Value>, String), String> {
    let limit = params.limit.min(mem_constants::MAX_LIMIT);
    let threshold = params.threshold.clamp(0.0, 1.0);

    // Try vector search if embedding service is available
    if let Some(embed_svc) = embedding_service {
        match embed_svc.embed(&params.query_text).await {
            Ok(query_embedding) => {
                let results = vector_search_core(
                    &query_embedding,
                    limit,
                    params.type_filter.as_deref(),
                    threshold,
                    &params.workflow_id,
                    &params.scope,
                    db,
                )
                .await?;
                return Ok((results, "vector".to_string()));
            }
            Err(e) => {
                warn!(error = %e, "Query embedding failed, falling back to text search");
            }
        }
    }

    // Fallback to text search
    let results = text_search_core(
        &params.query_text,
        limit,
        params.type_filter.as_deref(),
        &params.workflow_id,
        &params.scope,
        db,
    )
    .await?;
    Ok((results, "text".to_string()))
}

/// Performs vector similarity search using HNSW index with composite scoring.
///
/// Scoring formula:
///   final_score = cosine_similarity * 0.7 + importance * 0.15 + recency_score * 0.15
pub async fn vector_search_core(
    query_embedding: &[f32],
    limit: usize,
    type_filter: Option<&str>,
    threshold: f64,
    workflow_id: &Option<String>,
    scope: &str,
    db: &DBClient,
) -> Result<Vec<serde_json::Value>, String> {
    let mut conditions = vec!["embedding IS NOT NONE".to_string(), expiration_filter()];
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    if let Some(scope_cond) = build_scope_condition(scope, workflow_id, &mut params) {
        conditions.push(scope_cond);
    }

    if let Some(mem_type) = type_filter {
        conditions.push("type = $type_filter".to_string());
        params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
    }

    let where_clause = conditions.join(" AND ");
    let similarity_threshold = threshold;

    // Pre-allocate embedding string
    let mut embedding_str = String::with_capacity(query_embedding.len() * 12);
    for (i, v) in query_embedding.iter().enumerate() {
        if i > 0 {
            embedding_str.push_str(", ");
        }
        use std::fmt::Write;
        let _ = write!(embedding_str, "{}", v);
    }

    // Composite scoring: cosine * 0.7 + importance * 0.15 + recency * 0.15
    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            type,
            content,
            workflow_id,
            metadata,
            importance,
            expires_at,
            created_at,
            vector::similarity::cosine(embedding, [{embedding}]) AS cosine_score,
            (vector::similarity::cosine(embedding, [{embedding}]) * {w_cosine}
             + importance * {w_importance}
             + (1.0 - math::clamp(
                 duration::secs(time::now() - created_at) / ({decay_days} * 24.0 * 3600.0),
                 0.0,
                 1.0
               )) * {w_recency}
            ) AS score
        FROM memory
        WHERE {where_clause}
          AND vector::similarity::cosine(embedding, [{embedding}]) > {similarity}
        ORDER BY score DESC
        LIMIT {limit}"#,
        embedding = embedding_str,
        w_cosine = mem_constants::SCORE_WEIGHT_COSINE,
        w_importance = mem_constants::SCORE_WEIGHT_IMPORTANCE,
        w_recency = mem_constants::SCORE_WEIGHT_RECENCY,
        decay_days = mem_constants::RECENCY_DECAY_DAYS,
        where_clause = where_clause,
        similarity = similarity_threshold,
        limit = limit
    );

    let results: Vec<serde_json::Value> =
        db.query_json_with_params(&query, params)
            .await
            .map_err(|e| {
                error!(error = %e, "Vector search failed");
                format!("Failed to search memories: {}", e)
            })?;

    debug!(
        count = results.len(),
        threshold = threshold,
        scope = %scope,
        "Vector search completed"
    );

    Ok(results)
}

/// Performs text-based search as fallback when embeddings are unavailable.
pub async fn text_search_core(
    query_text: &str,
    limit: usize,
    type_filter: Option<&str>,
    workflow_id: &Option<String>,
    scope: &str,
    db: &DBClient,
) -> Result<Vec<serde_json::Value>, String> {
    let mut conditions = Vec::new();
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    conditions
        .push("string::lowercase(content) CONTAINS string::lowercase($query_text)".to_string());
    params.push(("query_text".to_string(), serde_json::json!(query_text)));

    conditions.push(expiration_filter());

    if let Some(scope_cond) = build_scope_condition(scope, workflow_id, &mut params) {
        conditions.push(scope_cond);
    }

    if let Some(mem_type) = type_filter {
        conditions.push("type = $type_filter".to_string());
        params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
    }

    let where_clause = conditions.join(" AND ");

    let query = format!(
        r#"SELECT
            meta::id(id) AS id,
            type,
            content,
            workflow_id,
            metadata,
            importance,
            expires_at,
            created_at
        FROM memory
        WHERE {}
        ORDER BY created_at DESC
        LIMIT {}"#,
        where_clause, limit
    );

    let memories: Vec<Memory> = db.query_with_params(&query, params).await.map_err(|e| {
        error!(error = %e, "Text search failed");
        format!("Failed to search memories: {}", e)
    })?;

    // Convert to JSON values with simple relevance scoring
    let query_lower = query_text.to_lowercase();
    let results: Vec<serde_json::Value> = memories
        .into_iter()
        .map(|m| {
            let content_lower = m.content.to_lowercase();
            let occurrences = content_lower.matches(&query_lower).count();
            let score = if m.content.is_empty() {
                0.0
            } else {
                ((occurrences as f64 * query_lower.len() as f64) / m.content.len() as f64).min(1.0)
            };

            serde_json::json!({
                "id": m.id,
                "type": m.memory_type,
                "content": m.content,
                "workflow_id": m.workflow_id,
                "metadata": m.metadata,
                "importance": m.importance,
                "expires_at": m.expires_at,
                "created_at": m.created_at,
                "score": score
            })
        })
        .collect();

    debug!(count = results.len(), scope = %scope, "Text search completed");

    Ok(results)
}

/// Retrieves statistics about memories (for the describe operation).
pub async fn describe_memories_core(
    workflow_id: Option<&str>,
    scope: &str,
    db: &DBClient,
) -> Result<MemoryDescribeResult, String> {
    // Build scope filter
    let scope_filter = match scope {
        "workflow" => {
            if let Some(wf_id) = workflow_id {
                format!("AND workflow_id = '{}'", wf_id)
            } else {
                return Ok(MemoryDescribeResult {
                    total: 0,
                    by_type: HashMap::new(),
                    tags: Vec::new(),
                    workflow_count: 0,
                    general_count: 0,
                    oldest: None,
                    newest: None,
                });
            }
        }
        "general" => "AND workflow_id IS NONE".to_string(),
        _ => {
            // "both" - workflow + general
            if let Some(wf_id) = workflow_id {
                format!("AND (workflow_id = '{}' OR workflow_id IS NONE)", wf_id)
            } else {
                String::new() // No filter needed if no workflow
            }
        }
    };

    let expiry = expiration_filter();

    // Count by type
    let type_query = format!(
        "SELECT type, count() AS cnt FROM memory WHERE {} {} GROUP BY type",
        expiry, scope_filter
    );
    let type_results: Vec<serde_json::Value> = db
        .query_json(&type_query)
        .await
        .map_err(|e| format!("Failed to count by type: {}", e))?;

    let mut by_type = HashMap::new();
    let mut total = 0usize;
    for row in &type_results {
        if let (Some(t), Some(cnt)) = (
            row.get("type").and_then(|v| v.as_str()),
            row.get("cnt").and_then(|v| v.as_u64()),
        ) {
            by_type.insert(t.to_string(), cnt as usize);
            total += cnt as usize;
        }
    }

    // Distinct tags
    let tags_query = format!(
        "SELECT array::distinct(array::flatten(metadata.tags)) AS tags FROM memory WHERE {} {}",
        expiry, scope_filter
    );
    let tags_results: Vec<serde_json::Value> = db
        .query_json(&tags_query)
        .await
        .map_err(|e| format!("Failed to get tags: {}", e))?;

    let tags: Vec<String> = tags_results
        .first()
        .and_then(|v| v.get("tags"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Date range
    let date_query = format!(
        "SELECT math::min(created_at) AS oldest, math::max(created_at) AS newest FROM memory WHERE {} {} GROUP ALL",
        expiry, scope_filter
    );
    let date_results: Vec<serde_json::Value> = db
        .query_json(&date_query)
        .await
        .map_err(|e| format!("Failed to get date range: {}", e))?;

    let oldest = date_results
        .first()
        .and_then(|v| v.get("oldest"))
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let newest = date_results
        .first()
        .and_then(|v| v.get("newest"))
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    // Workflow vs general counts
    let wf_count_query = format!(
        "SELECT count() AS cnt FROM memory WHERE {} {} AND workflow_id IS NOT NONE GROUP ALL",
        expiry, scope_filter
    );
    let wf_count_results: Vec<serde_json::Value> = db
        .query_json(&wf_count_query)
        .await
        .map_err(|e| format!("Failed to count workflow memories: {}", e))?;

    let workflow_count = wf_count_results
        .first()
        .and_then(|v| v.get("cnt"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let general_count = total.saturating_sub(workflow_count);

    Ok(MemoryDescribeResult {
        total,
        by_type,
        tags,
        workflow_count,
        general_count,
        oldest,
        newest,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_memory_params_construction() {
        let params = AddMemoryParams {
            memory_type: MemoryType::Knowledge,
            content: "Test content".to_string(),
            metadata: serde_json::json!({"source": "test"}),
            workflow_id: Some("wf_123".to_string()),
            importance: 0.6,
            expires_at: None,
        };

        assert_eq!(params.memory_type, MemoryType::Knowledge);
        assert_eq!(params.content, "Test content");
        assert!(params.workflow_id.is_some());
        assert!((params.importance - 0.6).abs() < f64::EPSILON);
    }

    #[test]
    fn test_add_memory_params_general_scope() {
        let params = AddMemoryParams {
            memory_type: MemoryType::Context,
            content: "General memory".to_string(),
            metadata: serde_json::json!({}),
            workflow_id: None,
            importance: 0.3,
            expires_at: None,
        };

        assert!(params.workflow_id.is_none());
    }

    #[test]
    fn test_add_memory_result_fields() {
        let result = AddMemoryResult {
            memory_id: "test-uuid".to_string(),
            embedding_generated: true,
        };

        assert_eq!(result.memory_id, "test-uuid");
        assert!(result.embedding_generated);
    }
}
