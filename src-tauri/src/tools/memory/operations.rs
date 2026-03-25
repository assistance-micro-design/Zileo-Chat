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

//! Memory operation implementations (add, get, list, search, describe, delete, clear).

use super::helpers::{
    add_memory_core, build_scope_condition, describe_memories_core, search_memories_core,
    AddMemoryParams, SearchParams,
};
use super::input::MemoryInput;
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::{Memory, MemoryType};
use crate::tools::constants::memory::{self as mem_constants, MAX_CONTENT_LENGTH, MAX_LIMIT};
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{
    db_error, delete_with_check, validate_enum_value, validate_length, validate_not_empty,
};
use crate::tools::{ToolError, ToolResult};
use chrono::{Duration, Utc};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, instrument};

/// Shared context for memory operations, avoiding too_many_arguments.
///
/// Bundles the dependencies that every operation needs from MemoryTool.
pub struct MemoryContext<'a> {
    pub db: &'a Arc<DBClient>,
    pub embedding_service: &'a Option<Arc<EmbeddingService>>,
    pub default_workflow_id: &'a Option<String>,
    pub agent_id: &'a str,
}

/// Determines the workflow_id to store on a new memory.
///
/// Priority: 1) explicit scope override, 2) auto-scope by type.
/// - `user_pref` and `knowledge` are general (workflow_id = None)
/// - `context` and `decision` are workflow-scoped (workflow_id = default_workflow_id)
pub fn resolve_storage_scope(
    memory_type: &str,
    input: &MemoryInput,
    default_workflow_id: &Option<String>,
) -> Option<String> {
    // Agent can override with explicit scope parameter
    if let Some(ref scope) = input.scope {
        return match scope.as_str() {
            "general" => None,
            "workflow" => default_workflow_id.clone(),
            _ => default_workflow_id.clone(),
        };
    }

    // Auto-scope based on memory type
    if mem_constants::GENERAL_SCOPE_TYPES.contains(&memory_type) {
        None // user_pref, knowledge -> always general
    } else {
        default_workflow_id.clone() // context, decision -> workflow-scoped
    }
}

/// Resolves the workflow_id for query filtering (list/search/describe).
///
/// Explicit `workflow_id` in input takes priority over `default_workflow_id`.
pub fn resolve_query_workflow_id(
    input: &MemoryInput,
    default_workflow_id: &Option<String>,
) -> Option<String> {
    input
        .workflow_id
        .clone()
        .or(default_workflow_id.clone())
}

/// Returns the default importance for a memory type.
fn default_importance_for_type(memory_type: &str) -> f64 {
    match memory_type {
        "user_pref" => mem_constants::IMPORTANCE_USER_PREF,
        "decision" => mem_constants::IMPORTANCE_DECISION,
        "knowledge" => mem_constants::IMPORTANCE_KNOWLEDGE,
        "context" => mem_constants::IMPORTANCE_CONTEXT,
        _ => mem_constants::DEFAULT_IMPORTANCE,
    }
}

/// Returns the default expires_at for a memory type.
fn default_expires_at_for_type(memory_type: &str) -> Option<chrono::DateTime<Utc>> {
    match memory_type {
        "context" => Some(Utc::now() + Duration::days(mem_constants::DEFAULT_CONTEXT_TTL_DAYS)),
        _ => None,
    }
}

/// Parses memory type from string.
fn parse_memory_type(type_str: &str) -> ToolResult<MemoryType> {
    match type_str {
        "user_pref" => Ok(MemoryType::UserPref),
        "context" => Ok(MemoryType::Context),
        "knowledge" => Ok(MemoryType::Knowledge),
        "decision" => Ok(MemoryType::Decision),
        _ => Err(ToolError::ValidationFailed(format!(
            "Invalid memory type '{}'. Valid types: user_pref, context, knowledge, decision",
            type_str
        ))),
    }
}

/// Adds a new memory with optional embedding.
///
/// Uses auto-scoping by type, auto-importance, and auto-TTL.
/// The agent can override auto-scoping via the `scope` parameter.
///
/// # Arguments
/// * `input` - Parsed memory input (provides scope override, workflow_id, etc.)
/// * `memory_type` - Type of memory (user_pref, context, knowledge, decision)
/// * `content` - Text content of the memory
/// * `metadata` - Additional metadata (optional)
/// * `tags` - Classification tags (optional)
/// * `ctx` - Shared tool context (db, embedding, workflow_id, agent_id)
#[instrument(skip(input, content, metadata, ctx), fields(agent_id = %ctx.agent_id, memory_type = %memory_type))]
pub async fn add_memory(
    input: &MemoryInput,
    memory_type: &str,
    content: &str,
    metadata: Option<Value>,
    tags: Option<Vec<String>>,
    ctx: &MemoryContext<'_>,
) -> ToolResult<Value> {
    // Validate content length
    validate_not_empty(content, "content")?;
    validate_length(content, MAX_CONTENT_LENGTH, "content")?;

    // Validate memory type
    validate_enum_value(memory_type, mem_constants::VALID_TYPES, "memory_type")?;
    let mem_type = parse_memory_type(memory_type)?;

    // Auto-scope by type (or explicit override via scope param)
    let workflow_id = resolve_storage_scope(memory_type, input, ctx.default_workflow_id);

    // Auto-importance by type
    let importance = default_importance_for_type(memory_type);

    // Auto-TTL by type (context -> 7 days)
    let expires_at = default_expires_at_for_type(memory_type);

    // Build metadata with agent source and tags (Tool-specific enrichment)
    let mut meta = metadata.unwrap_or(serde_json::json!({}));
    if let Some(obj) = meta.as_object_mut() {
        obj.insert("agent_source".to_string(), serde_json::json!(ctx.agent_id));
        if let Some(t) = tags {
            obj.insert("tags".to_string(), serde_json::json!(t));
        }
    }

    // Use shared helper for core creation logic
    let params = AddMemoryParams {
        memory_type: mem_type,
        content: content.to_string(),
        metadata: meta,
        workflow_id: workflow_id.clone(),
        importance,
        expires_at,
    };

    let result = add_memory_core(params, ctx.db, ctx.embedding_service.as_ref())
        .await
        .map_err(ToolError::DatabaseError)?;

    info!(
        memory_id = %result.memory_id,
        memory_type = %memory_type,
        embedding = result.embedding_generated,
        scope = ?workflow_id,
        "Memory created"
    );

    Ok(ResponseBuilder::new()
        .success(true)
        .id("memory_id", result.memory_id)
        .field("type", memory_type)
        .field("embedding_generated", result.embedding_generated)
        .field("workflow_id", workflow_id)
        .field("importance", importance)
        .message("Memory created successfully")
        .build())
}

/// Retrieves a memory by ID.
///
/// # Arguments
/// * `memory_id` - Memory ID to retrieve
/// * `ctx` - Shared tool context
#[instrument(skip(ctx), fields(memory_id = %memory_id))]
pub async fn get_memory(memory_id: &str, ctx: &MemoryContext<'_>) -> ToolResult<Value> {
    // Parameterized query for security
    let query = r#"SELECT
            meta::id(id) AS id,
            type,
            content,
            workflow_id,
            metadata,
            importance,
            expires_at,
            created_at
        FROM memory
        WHERE meta::id(id) = $memory_id"#;

    let params = vec![("memory_id".to_string(), serde_json::json!(memory_id))];
    let results: Vec<Memory> = ctx.db.query_with_params(query, params).await.map_err(db_error)?;

    match results.into_iter().next() {
        Some(memory) => Ok(serde_json::json!({
            "success": true,
            "memory": memory
        })),
        None => Err(ToolError::NotFound(format!(
            "Memory '{}' does not exist. Use 'list' to see available memories",
            memory_id
        ))),
    }
}

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

    let memories: Vec<Memory> = ctx.db.query_with_params(&query, params).await.map_err(db_error)?;

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

/// Deletes a memory by ID.
///
/// # Arguments
/// * `memory_id` - Memory ID to delete
/// * `ctx` - Shared tool context
#[instrument(skip(ctx), fields(memory_id = %memory_id))]
pub async fn delete_memory(memory_id: &str, ctx: &MemoryContext<'_>) -> ToolResult<Value> {
    delete_with_check(ctx.db, "memory", memory_id, "Memory").await?;

    info!(memory_id = %memory_id, "Memory deleted");

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

    // Use execute_with_params() for parameterized DELETE
    let (delete_query, params) = if let Some(ref wf_id) = workflow_id {
        (
            "DELETE FROM memory WHERE type = $memory_type AND workflow_id = $workflow_id".to_string(),
            vec![
                ("memory_type".to_string(), serde_json::json!(memory_type)),
                ("workflow_id".to_string(), serde_json::json!(wf_id)),
            ],
        )
    } else {
        (
            "DELETE FROM memory WHERE type = $memory_type".to_string(),
            vec![("memory_type".to_string(), serde_json::json!(memory_type))],
        )
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_memory_type_valid() {
        assert!(matches!(parse_memory_type("user_pref"), Ok(MemoryType::UserPref)));
        assert!(matches!(parse_memory_type("context"), Ok(MemoryType::Context)));
        assert!(matches!(parse_memory_type("knowledge"), Ok(MemoryType::Knowledge)));
        assert!(matches!(parse_memory_type("decision"), Ok(MemoryType::Decision)));
    }

    #[test]
    fn test_parse_memory_type_invalid() {
        let result = parse_memory_type("invalid");
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => assert!(msg.contains("Invalid memory type")),
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_resolve_storage_scope_auto_general() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "add",
            "type": "knowledge",
            "content": "test"
        }))
        .unwrap();
        let wf = Some("wf_001".to_string());

        // knowledge -> general (None)
        assert!(resolve_storage_scope("knowledge", &input, &wf).is_none());
        assert!(resolve_storage_scope("user_pref", &input, &wf).is_none());
    }

    #[test]
    fn test_resolve_storage_scope_auto_workflow() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "add",
            "type": "context",
            "content": "test"
        }))
        .unwrap();
        let wf = Some("wf_001".to_string());

        // context, decision -> workflow-scoped
        assert_eq!(resolve_storage_scope("context", &input, &wf), Some("wf_001".to_string()));
        assert_eq!(resolve_storage_scope("decision", &input, &wf), Some("wf_001".to_string()));
    }

    #[test]
    fn test_resolve_storage_scope_explicit_override() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "add",
            "type": "decision",
            "content": "test",
            "scope": "general"
        }))
        .unwrap();
        let wf = Some("wf_001".to_string());

        // explicit "general" overrides auto-scope
        assert!(resolve_storage_scope("decision", &input, &wf).is_none());
    }

    #[test]
    fn test_resolve_query_workflow_id_explicit_override() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "list",
            "workflow_id": "explicit_wf"
        }))
        .unwrap();
        let default = Some("default_wf".to_string());

        assert_eq!(
            resolve_query_workflow_id(&input, &default),
            Some("explicit_wf".to_string())
        );
    }

    #[test]
    fn test_resolve_query_workflow_id_falls_back_to_default() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "list"
        }))
        .unwrap();
        let default = Some("default_wf".to_string());

        assert_eq!(
            resolve_query_workflow_id(&input, &default),
            Some("default_wf".to_string())
        );
    }

    #[test]
    fn test_default_importance_for_type() {
        assert!((default_importance_for_type("user_pref") - 0.8).abs() < f64::EPSILON);
        assert!((default_importance_for_type("decision") - 0.7).abs() < f64::EPSILON);
        assert!((default_importance_for_type("knowledge") - 0.6).abs() < f64::EPSILON);
        assert!((default_importance_for_type("context") - 0.3).abs() < f64::EPSILON);
        assert!((default_importance_for_type("unknown") - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_default_expires_at_for_type() {
        // context -> Some (7-day TTL)
        assert!(default_expires_at_for_type("context").is_some());
        // others -> None
        assert!(default_expires_at_for_type("knowledge").is_none());
        assert!(default_expires_at_for_type("user_pref").is_none());
        assert!(default_expires_at_for_type("decision").is_none());
    }
}
