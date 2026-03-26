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

//! Search and describe operations for memory helpers.
//!
//! Contains vector search, text search fallback, and describe (statistics)
//! operations shared between MemoryTool and memory Tauri commands.

use super::helpers::{build_scope_condition, expiration_filter, SearchParams};
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::MemoryDescribeResult;
use crate::models::Memory;
use crate::tools::constants::memory as mem_constants;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, warn};

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
