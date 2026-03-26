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

//! Memory operation implementations - add and get operations, plus shared helpers.
//!
//! Query operations (list, search, describe, delete, clear) are in `operations_query.rs`.

use super::helpers::{add_memory_core, AddMemoryParams};
use super::input::MemoryInput;
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::{Memory, MemoryType};
use crate::tools::constants::memory::{self as mem_constants, MAX_CONTENT_LENGTH};
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{db_error, validate_enum_value, validate_length, validate_not_empty};
use crate::tools::{ToolError, ToolResult};
use chrono::{Duration, Utc};
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, instrument};

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
    input.workflow_id.clone().or(default_workflow_id.clone())
}

/// Returns the default importance for a memory type.
pub(super) fn default_importance_for_type(memory_type: &str) -> f64 {
    match memory_type {
        "user_pref" => mem_constants::IMPORTANCE_USER_PREF,
        "decision" => mem_constants::IMPORTANCE_DECISION,
        "knowledge" => mem_constants::IMPORTANCE_KNOWLEDGE,
        "context" => mem_constants::IMPORTANCE_CONTEXT,
        _ => mem_constants::DEFAULT_IMPORTANCE,
    }
}

/// Returns the default expires_at for a memory type.
pub(super) fn default_expires_at_for_type(memory_type: &str) -> Option<chrono::DateTime<Utc>> {
    match memory_type {
        "context" => Some(Utc::now() + Duration::days(mem_constants::DEFAULT_CONTEXT_TTL_DAYS)),
        _ => None,
    }
}

/// Parses memory type from string.
pub(super) fn parse_memory_type(type_str: &str) -> ToolResult<MemoryType> {
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
    let results: Vec<Memory> = ctx
        .db
        .query_with_params(query, params)
        .await
        .map_err(db_error)?;

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

#[cfg(test)]
#[path = "operations_tests.rs"]
mod tests;
