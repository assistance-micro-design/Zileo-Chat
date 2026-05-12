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
//! The vector and text search paths target `memory_chunk` rows and traverse
//! the parent `memory` via the typed record link (`memory_id.field`). When
//! no chunks exist yet (fresh install or pre-reindex state), both paths fall
//! back to scanning `memory.content` directly so an agent never sees an
//! empty index unjustifiably.

use super::helpers::{build_scope_condition, expiration_filter, SearchParams};
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::MemoryDescribeResult;
use crate::models::Memory;
use crate::security::validate_uuid_field;
use crate::tools::constants::memory as mem_constants;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, warn};

/// Searches memories using semantic similarity with text search fallback.
///
/// Vector path runs when an `EmbeddingService` is available AND the query
/// embedding generation succeeds. Otherwise (or if vector returns 0 rows
/// and the chunk table is empty) the text fallback runs on `memory.content`
/// directly to cover the pre-reindex gap.
pub async fn search_memories_core(
    params: SearchParams,
    db: &DBClient,
    embedding_service: Option<&Arc<EmbeddingService>>,
) -> Result<(Vec<serde_json::Value>, String), String> {
    let mut params = params;
    params.limit = params.limit.min(mem_constants::MAX_LIMIT);
    params.threshold = params.threshold.clamp(0.0, 1.0);

    // Try vector search if embedding service is available
    if let Some(embed_svc) = embedding_service {
        match embed_svc.embed(&params.query_text).await {
            Ok(query_embedding) => {
                let results = vector_search_core(&query_embedding, &params, db).await?;
                if !results.is_empty() {
                    return Ok((results, "vector".to_string()));
                }
                // Empty vector results may mean: (a) no semantic match in
                // chunks, or (b) the chunk table is empty pre-reindex.
                // Fall through to text-on-memory.content so an agent still
                // gets results in the pre-reindex window.
                debug!("Vector search returned 0 rows; falling back to text on memory.content");
            }
            Err(e) => {
                warn!(error = %e, "Query embedding failed, falling back to text search");
            }
        }
    }

    let results = text_search_core(&params, db).await?;
    Ok((results, "text".to_string()))
}

/// Builds a `CONTAINSANY` filter clause + bind tuple from a tags list.
///
/// Returns `None` when the input is `None` or an empty slice — the caller
/// then skips appending anything to the WHERE clause. This keeps tagless
/// searches free of an empty array bind that some SurrealDB versions
/// short-circuit to "no match".
pub(crate) fn build_tags_filter_clause(
    tags: Option<&[String]>,
    field_prefix: &str,
) -> Option<(String, (String, serde_json::Value))> {
    let tags = tags?;
    if tags.is_empty() {
        return None;
    }
    let clause = format!("{}.metadata.tags CONTAINSANY $tags_filter", field_prefix);
    let bind = ("tags_filter".to_string(), serde_json::json!(tags));
    Some((clause, bind))
}

/// Performs vector similarity search on `memory_chunk` rows, traversing the
/// `memory_id` record link to filter by parent attributes (type, scope,
/// expiration, tags).
///
/// Scoring formula on the parent's `importance` and `created_at`:
///   final_score = cosine * 0.7 + importance * 0.15 + recency * 0.15
pub async fn vector_search_core(
    query_embedding: &[f32],
    search: &SearchParams,
    db: &DBClient,
) -> Result<Vec<serde_json::Value>, String> {
    let mut conditions = vec![
        "embedding IS NOT NONE".to_string(),
        "(memory_id.expires_at IS NONE OR memory_id.expires_at > time::now())".to_string(),
    ];
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    if let Some(scope_cond) =
        build_scope_condition_for_parent(&search.scope, &search.workflow_id, &mut params)
    {
        conditions.push(scope_cond);
    }

    if let Some(ref mem_type) = search.type_filter {
        conditions.push("memory_id.type = $type_filter".to_string());
        params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
    }

    if let Some((clause, bind)) =
        build_tags_filter_clause(search.tags_filter.as_deref(), "memory_id")
    {
        conditions.push(clause);
        params.push(bind);
    }

    let where_clause = conditions.join(" AND ");

    // Embedding inlined as a SurrealQL array literal — binding `Vec<f32>` as
    // JSON loses precision in the v2.x SDK path. This is internal data, not
    // user input, so format!() is safe here.
    let mut embedding_str = String::with_capacity(query_embedding.len() * 12);
    for (i, v) in query_embedding.iter().enumerate() {
        if i > 0 {
            embedding_str.push_str(", ");
        }
        use std::fmt::Write;
        let _ = write!(embedding_str, "{}", v);
    }

    let query = format!(
        r#"SELECT
            meta::id(id) AS chunk_id,
            meta::id(memory_id) AS parent_memory_id,
            chunk_index,
            chunk_count,
            content,
            memory_id.type AS memory_type,
            memory_id.workflow_id AS workflow_id,
            memory_id.metadata AS metadata,
            memory_id.importance AS importance,
            memory_id.expires_at AS expires_at,
            memory_id.created_at AS created_at,
            vector::similarity::cosine(embedding, [{embedding}]) AS cosine_score,
            (vector::similarity::cosine(embedding, [{embedding}]) * {w_cosine}
             + memory_id.importance * {w_importance}
             + (1.0 - math::clamp(
                 duration::secs(time::now() - memory_id.created_at) / ({decay_days} * 24.0 * 3600.0),
                 0.0,
                 1.0
               )) * {w_recency}
            ) AS score,
            'vector' AS search_type
        FROM memory_chunk
        WHERE {where_clause}
          AND vector::similarity::cosine(embedding, [{embedding}]) > {threshold}
        ORDER BY score DESC
        LIMIT {limit}"#,
        embedding = embedding_str,
        w_cosine = mem_constants::SCORE_WEIGHT_COSINE,
        w_importance = mem_constants::SCORE_WEIGHT_IMPORTANCE,
        w_recency = mem_constants::SCORE_WEIGHT_RECENCY,
        decay_days = mem_constants::RECENCY_DECAY_DAYS,
        where_clause = where_clause,
        threshold = search.threshold,
        limit = search.limit
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
        threshold = search.threshold,
        scope = %search.scope,
        "Vector search completed (memory_chunk)"
    );

    Ok(results)
}

/// Performs text-based search.
///
/// Primary path: scans `memory_chunk.content` and traverses the parent
/// for filters. When `memory_chunk` is empty globally (pre-reindex), falls
/// back to scanning `memory.content` directly so the agent still gets text
/// hits while waiting for reindex.
pub async fn text_search_core(
    search: &SearchParams,
    db: &DBClient,
) -> Result<Vec<serde_json::Value>, String> {
    if memory_chunk_table_is_empty(db).await? {
        return legacy_text_search(search, db).await;
    }

    let mut conditions = Vec::new();
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    conditions
        .push("string::lowercase(content) CONTAINS string::lowercase($query_text)".to_string());
    params.push((
        "query_text".to_string(),
        serde_json::json!(&search.query_text),
    ));

    conditions
        .push("(memory_id.expires_at IS NONE OR memory_id.expires_at > time::now())".to_string());

    if let Some(scope_cond) =
        build_scope_condition_for_parent(&search.scope, &search.workflow_id, &mut params)
    {
        conditions.push(scope_cond);
    }

    if let Some(ref mem_type) = search.type_filter {
        conditions.push("memory_id.type = $type_filter".to_string());
        params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
    }

    if let Some((clause, bind)) =
        build_tags_filter_clause(search.tags_filter.as_deref(), "memory_id")
    {
        conditions.push(clause);
        params.push(bind);
    }

    let where_clause = conditions.join(" AND ");

    let query = format!(
        r#"SELECT
            meta::id(id) AS chunk_id,
            meta::id(memory_id) AS parent_memory_id,
            chunk_index,
            chunk_count,
            content,
            memory_id.type AS memory_type,
            memory_id.workflow_id AS workflow_id,
            memory_id.metadata AS metadata,
            memory_id.importance AS importance,
            memory_id.expires_at AS expires_at,
            memory_id.created_at AS created_at,
            0.0 AS cosine_score,
            0.0 AS score,
            'text' AS search_type
        FROM memory_chunk
        WHERE {}
        ORDER BY memory_id.created_at DESC
        LIMIT {}"#,
        where_clause, search.limit
    );

    let results: Vec<serde_json::Value> =
        db.query_json_with_params(&query, params)
            .await
            .map_err(|e| {
                error!(error = %e, "Text search on memory_chunk failed");
                format!("Failed to search memories: {}", e)
            })?;

    // Refine the score with a simple occurrence-based heuristic (same shape
    // as the previous text path) so the API contract stays stable.
    let query_lower = search.query_text.to_lowercase();
    let mut enriched: Vec<serde_json::Value> = results
        .into_iter()
        .map(|mut row| {
            if let Some(content) = row.get("content").and_then(|c| c.as_str()) {
                let content_lower = content.to_lowercase();
                let occurrences = content_lower.matches(&query_lower).count();
                let score = if content.is_empty() {
                    0.0
                } else {
                    ((occurrences as f64 * query_lower.len() as f64) / content.len() as f64)
                        .min(1.0)
                };
                if let Some(obj) = row.as_object_mut() {
                    obj.insert(
                        "score".to_string(),
                        serde_json::Number::from_f64(score)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    );
                }
            }
            row
        })
        .collect();

    enriched.sort_by(|a, b| {
        let sa = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let sb = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    debug!(
        count = enriched.len(),
        scope = %search.scope,
        "Text search completed (memory_chunk)"
    );
    Ok(enriched)
}

/// Pre-reindex fallback: scan `memory.content` directly so an agent
/// still gets text hits before the user clicks Reindex. The output shape
/// is the same as the chunked path so callers don't branch.
async fn legacy_text_search(
    search: &SearchParams,
    db: &DBClient,
) -> Result<Vec<serde_json::Value>, String> {
    let mut conditions = Vec::new();
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    conditions
        .push("string::lowercase(content) CONTAINS string::lowercase($query_text)".to_string());
    params.push((
        "query_text".to_string(),
        serde_json::json!(&search.query_text),
    ));

    conditions.push(expiration_filter());

    if let Some(scope_cond) = build_scope_condition(&search.scope, &search.workflow_id, &mut params)
    {
        conditions.push(scope_cond);
    }

    if let Some(ref mem_type) = search.type_filter {
        conditions.push("type = $type_filter".to_string());
        params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
    }

    if let Some((clause, bind)) = build_tags_filter_clause(search.tags_filter.as_deref(), "memory")
    {
        // legacy path scans `memory` directly, so strip the `memory.` prefix
        // we just embedded — `metadata.tags` is a bare column on the parent.
        conditions.push(clause.replace("memory.metadata.tags", "metadata.tags"));
        params.push(bind);
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
        where_clause, search.limit
    );

    let memories: Vec<Memory> = db.query_with_params(&query, params).await.map_err(|e| {
        error!(error = %e, "Legacy text search on memory.content failed");
        format!("Failed to search memories: {}", e)
    })?;

    let query_lower = search.query_text.to_lowercase();
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

            // Surface this row in the same shape as a chunk result: chunk_id
            // = parent id (no real chunk to point at), chunk_index = 0,
            // chunk_count = 1. Agents reading `parent_memory_id` + get() will
            // work transparently.
            serde_json::json!({
                "chunk_id": m.id,
                "parent_memory_id": m.id,
                "chunk_index": 0,
                "chunk_count": 1,
                "content": m.content,
                "memory_type": m.memory_type,
                "workflow_id": m.workflow_id,
                "metadata": m.metadata,
                "importance": m.importance,
                "expires_at": m.expires_at,
                "created_at": m.created_at,
                "cosine_score": 0.0,
                "score": score,
                "search_type": "text",
            })
        })
        .collect();

    debug!(
        count = results.len(),
        scope = %search.scope,
        "Legacy text search completed (memory.content pre-reindex fallback)"
    );
    Ok(results)
}

/// `true` when `memory_chunk` has zero rows (fresh install or pre-reindex).
async fn memory_chunk_table_is_empty(db: &DBClient) -> Result<bool, String> {
    let rows: Vec<serde_json::Value> = db
        .query_json("SELECT count() AS cnt FROM memory_chunk GROUP ALL")
        .await
        .map_err(|e| format!("Failed to count memory_chunk: {}", e))?;
    let cnt = rows
        .first()
        .and_then(|r| r.get("cnt"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    Ok(cnt == 0)
}

/// Same as [`build_scope_condition`] but prefixed for the parent reached
/// through the `memory_id` record link on `memory_chunk` rows.
fn build_scope_condition_for_parent(
    scope: &str,
    workflow_id: &Option<String>,
    params: &mut Vec<(String, serde_json::Value)>,
) -> Option<String> {
    match scope {
        "workflow" => workflow_id.as_ref().map(|wf_id| {
            params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
            "memory_id.workflow_id = $workflow_id".to_string()
        }),
        "general" => Some("memory_id.workflow_id IS NONE".to_string()),
        _ => workflow_id.as_ref().map(|wf_id| {
            params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
            "(memory_id.workflow_id = $workflow_id OR memory_id.workflow_id IS NONE)".to_string()
        }),
    }
}

/// Retrieves statistics about memories (for the describe operation).
///
/// Uses parameterized queries: when a workflow_id is supplied, it is validated
/// as a strict UUID v4 and bound as `$wf_id` rather than interpolated, preventing
/// SurrealQL injection through composed scope filters.
pub async fn describe_memories_core(
    workflow_id: Option<&str>,
    scope: &str,
    db: &DBClient,
) -> Result<MemoryDescribeResult, String> {
    // Validate workflow_id once if provided, then bind it as a parameter
    let validated_wf_id = match workflow_id {
        Some(id) => Some(validate_uuid_field(id, "workflow_id")?),
        None => None,
    };

    // Build scope filter using bind parameter for the workflow id
    let (scope_filter, scope_params): (String, Vec<(String, serde_json::Value)>) = match scope {
        "workflow" => match validated_wf_id.as_ref() {
            Some(wf_id) => (
                "AND workflow_id = $wf_id".to_string(),
                vec![("wf_id".to_string(), serde_json::json!(wf_id))],
            ),
            None => {
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
        },
        "general" => ("AND workflow_id IS NONE".to_string(), Vec::new()),
        _ => match validated_wf_id.as_ref() {
            Some(wf_id) => (
                "AND (workflow_id = $wf_id OR workflow_id IS NONE)".to_string(),
                vec![("wf_id".to_string(), serde_json::json!(wf_id))],
            ),
            None => (String::new(), Vec::new()),
        },
    };

    let expiry = expiration_filter();

    // Count by type
    let type_query = format!(
        "SELECT type, count() AS cnt FROM memory WHERE {} {} GROUP BY type",
        expiry, scope_filter
    );
    let type_results: Vec<serde_json::Value> = db
        .query_json_with_params(&type_query, scope_params.clone())
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
        .query_json_with_params(&tags_query, scope_params.clone())
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
        .query_json_with_params(&date_query, scope_params.clone())
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
        .query_json_with_params(&wf_count_query, scope_params)
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
#[path = "helpers_search_tests.rs"]
mod tests;
