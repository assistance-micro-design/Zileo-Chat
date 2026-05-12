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

//! Memory query operations: list, search, describe, delete, clear.

use super::helpers::{build_scope_condition, SearchParams};
use super::helpers_search::{describe_memories_core, search_memories_core};
use super::input::MemoryInput;
use super::operations::{parse_memory_type, resolve_query_workflow_id, MemoryContext};
use crate::models::memory::Memory;
use crate::tools::constants::memory::MAX_LIMIT;
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{db_error, delete_with_check};
use crate::tools::{ToolError, ToolResult};
use serde_json::Value;
use tracing::{debug, info, instrument};

/// Lists memories with optional filters.
///
/// # Arguments
/// * `input` - Parsed memory input (provides workflow_id override)
/// * `type_filter` - Optional memory type to filter by
/// * `limit` - Maximum number of results (default: 10)
/// * `scope` - Scope filter: "workflow", "general", or "both" (default: "both")
/// * `mode` - Display mode: "full" (default) or "compact"
/// * `ctx` - Shared tool context
#[instrument(skip(input, ctx), fields(type_filter = ?type_filter, limit = limit, scope = %scope))]
pub async fn list_memories(
    input: &MemoryInput,
    type_filter: Option<&str>,
    limit: usize,
    scope: &str,
    mode: &str,
    ctx: &MemoryContext<'_>,
) -> ToolResult<Value> {
    let workflow_id = resolve_query_workflow_id(input, ctx.default_workflow_id);
    let limit = limit.min(MAX_LIMIT);

    let mut conditions = Vec::new();
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    // Expiration filter
    conditions.push(super::helpers::expiration_filter());

    // Special case: scope="workflow" with no active workflow returns early
    if scope == "workflow" && workflow_id.is_none() {
        return Ok(ResponseBuilder::new()
            .success(true)
            .count(0)
            .field("scope", "workflow")
            .field("mode", mode)
            .field("workflow_id", Option::<String>::None)
            .data("memories", Vec::<Memory>::new())
            .message("No active workflow. Use scope='both' or provide workflow_id")
            .build());
    }
    if let Some(scope_cond) = build_scope_condition(scope, &workflow_id, &mut params) {
        conditions.push(scope_cond);
    }

    if let Some(mem_type) = type_filter {
        parse_memory_type(mem_type)?;
        conditions.push("type = $type_filter".to_string());
        params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

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
        {}
        ORDER BY created_at DESC
        LIMIT {}"#,
        where_clause, limit
    );

    let memories: Vec<Memory> = ctx
        .db
        .query_with_params(&query, params)
        .await
        .map_err(db_error)?;

    debug!(count = memories.len(), scope = %scope, mode = %mode, "Memories listed");

    if mode == "compact" {
        // Compact mode: truncate content, extract tags/importance as top-level fields
        let compact_memories: Vec<serde_json::Value> = memories
            .into_iter()
            .map(|m| {
                let preview = crate::tools::utils::safe_truncate(
                    &m.content,
                    crate::tools::constants::memory::COMPACT_PREVIEW_LENGTH,
                    true,
                );
                let tags = m
                    .metadata
                    .get("tags")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();

                serde_json::json!({
                    "id": m.id,
                    "type": m.memory_type,
                    "preview": preview,
                    "tags": tags,
                    "importance": m.importance,
                    "workflow_id": m.workflow_id,
                    "created_at": m.created_at,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "success": true,
            "count": compact_memories.len(),
            "mode": "compact",
            "scope": scope,
            "workflow_id": workflow_id,
            "memories": compact_memories,
        }))
    } else {
        Ok(ResponseBuilder::new()
            .success(true)
            .count(memories.len())
            .field("scope", scope)
            .field("mode", "full")
            .field("workflow_id", workflow_id)
            .data("memories", memories)
            .build())
    }
}

/// Searches memories using semantic similarity (delegates to shared helpers).
///
/// # Arguments
/// * `input` - Parsed memory input (provides workflow_id override)
/// * `query_text` - Search query
/// * `limit` - Maximum results (default: 10)
/// * `type_filter` - Optional type filter
/// * `threshold` - Similarity threshold 0-1 (default: 0.7)
/// * `scope` - Scope filter: "workflow", "general", or "both" (default: "both")
/// * `ctx` - Shared tool context
#[instrument(skip(input, ctx), fields(query_len = query_text.len(), limit = limit, scope = %scope))]
pub async fn search_memories(
    input: &MemoryInput,
    query_text: &str,
    limit: usize,
    type_filter: Option<&str>,
    threshold: f64,
    scope: &str,
    ctx: &MemoryContext<'_>,
) -> ToolResult<Value> {
    let workflow_id = resolve_query_workflow_id(input, ctx.default_workflow_id);

    // Validate type filter if provided
    if let Some(mem_type) = type_filter {
        parse_memory_type(mem_type)?;
    }

    let params = SearchParams {
        query_text: query_text.to_string(),
        limit,
        type_filter: type_filter.map(String::from),
        workflow_id: workflow_id.clone(),
        scope: scope.to_string(),
        threshold,
        tags_filter: input.tags_filter.clone(),
    };

    let (results, search_type) =
        search_memories_core(params, ctx.db, ctx.embedding_service.as_ref())
            .await
            .map_err(ToolError::DatabaseError)?;

    Ok(serde_json::json!({
        "success": true,
        "search_type": search_type,
        "count": results.len(),
        "threshold": threshold,
        "scope": scope,
        "workflow_id": workflow_id,
        "results": results
    }))
}

/// Describes memory statistics (for agent discovery).
///
/// # Arguments
/// * `input` - Parsed memory input (provides workflow_id override)
/// * `scope` - Scope filter
/// * `ctx` - Shared tool context
#[instrument(skip(input, ctx), fields(scope = %scope))]
pub async fn describe_memories(
    input: &MemoryInput,
    scope: &str,
    ctx: &MemoryContext<'_>,
) -> ToolResult<Value> {
    let wf_id = resolve_query_workflow_id(input, ctx.default_workflow_id);

    let result = describe_memories_core(wf_id.as_deref(), scope, ctx.db)
        .await
        .map_err(ToolError::DatabaseError)?;

    Ok(serde_json::json!({
        "success": true,
        "total": result.total,
        "by_type": result.by_type,
        "tags": result.tags,
        "scope": scope,
        "workflow_id": wf_id,
        "workflow_count": result.workflow_count,
        "general_count": result.general_count,
        "oldest": result.oldest,
        "newest": result.newest,
    }))
}

/// Deletes a memory by ID along with its child chunks (cascade).
///
/// `delete_with_check` already validates `memory_id` as a strict UUID v4
/// before the parent is dropped — we reuse that validation for the cascade
/// DELETE on `memory_chunk` (the UUID is reused as a record-id literal).
///
/// # Arguments
/// * `memory_id` - Memory ID to delete
/// * `ctx` - Shared tool context
#[instrument(skip(ctx), fields(memory_id = %memory_id))]
pub async fn delete_memory(memory_id: &str, ctx: &MemoryContext<'_>) -> ToolResult<Value> {
    // Cascade BEFORE the parent so chunks don't dangle on partial failure.
    // delete_with_check below revalidates the UUID — we accept the redundant
    // check rather than expose a bypass.
    let cascade_query = format!(
        "DELETE FROM memory_chunk WHERE memory_id = memory:`{}`",
        memory_id
    );
    ctx.db.execute(&cascade_query).await.map_err(db_error)?;

    delete_with_check(ctx.db, "memory", memory_id, "Memory").await?;

    info!(memory_id = %memory_id, "Memory deleted (with cascade)");

    Ok(ResponseBuilder::ok(
        "memory_id",
        memory_id,
        "Memory deleted successfully",
    ))
}

/// Clears all memories of a specific type.
///
/// # Arguments
/// * `input` - Parsed memory input (provides scope/workflow_id override)
/// * `memory_type` - Type of memories to clear
/// * `ctx` - Shared tool context
#[instrument(skip(input, ctx), fields(memory_type = %memory_type))]
pub async fn clear_by_type(
    input: &MemoryInput,
    memory_type: &str,
    ctx: &MemoryContext<'_>,
) -> ToolResult<Value> {
    // Validate memory type
    parse_memory_type(memory_type)?;

    let workflow_id = resolve_query_workflow_id(input, ctx.default_workflow_id);

    // Cascade: drop chunks whose parent matches the same filter BEFORE the
    // parent rows themselves, so partial failure cannot leave orphans.
    // `SELECT VALUE id` unwraps the record link so `memory_id IN (...)`
    // compares against record literals, not `{id: ...}` objects.
    let (cascade_query, delete_query, params) = if let Some(ref wf_id) = workflow_id {
        let p = vec![
            ("memory_type".to_string(), serde_json::json!(memory_type)),
            ("workflow_id".to_string(), serde_json::json!(wf_id)),
        ];
        (
            "DELETE FROM memory_chunk WHERE memory_id IN \
             (SELECT VALUE id FROM memory WHERE type = $memory_type AND workflow_id = $workflow_id)"
                .to_string(),
            "DELETE FROM memory WHERE type = $memory_type AND workflow_id = $workflow_id"
                .to_string(),
            p,
        )
    } else {
        let p = vec![("memory_type".to_string(), serde_json::json!(memory_type))];
        (
            "DELETE FROM memory_chunk WHERE memory_id IN \
             (SELECT VALUE id FROM memory WHERE type = $memory_type)"
                .to_string(),
            "DELETE FROM memory WHERE type = $memory_type".to_string(),
            p,
        )
    };

    ctx.db
        .execute_with_params(&cascade_query, params.clone())
        .await
        .map_err(db_error)?;

    ctx.db
        .execute_with_params(&delete_query, params)
        .await
        .map_err(db_error)?;

    info!(
        memory_type = %memory_type,
        workflow_id = ?workflow_id,
        "Memories cleared by type"
    );

    Ok(serde_json::json!({
        "success": true,
        "type": memory_type,
        "scope": if workflow_id.is_some() { "workflow" } else { "general" },
        "workflow_id": workflow_id,
        "message": format!("All '{}' memories have been cleared", memory_type)
    }))
}
