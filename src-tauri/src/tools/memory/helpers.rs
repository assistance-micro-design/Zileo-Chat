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
//!
//! # Architecture
//!
//! - `helpers.rs` - Add memory logic, parameters, scope/expiration builders
//! - `helpers_search.rs` - Search (vector + text) and describe operations

use super::chunker::{split_recursive, DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE};
use crate::db::{sanitize_for_surrealdb, DBClient};
use crate::llm::embedding::EmbeddingService;
use crate::models::{MemoryCreate, MemoryType};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tracing::{info, warn};
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
    /// Optional tag filter (CONTAINSANY semantics on parent `metadata.tags`)
    pub tags_filter: Option<Vec<String>>,
}

/// Core logic for adding a memory: 1 parent `memory` row + N `memory_chunk`
/// rows, each chunk linked back to the parent via a typed `record<memory>`.
///
/// 1. Generate parent memory UUID and create the `memory` row (no embedding)
/// 2. Run [`split_recursive`] over `params.content`
/// 3. For each chunk: attempt embedding, then `CREATE memory_chunk` with the
///    `memory_id` record link (embedding `NONE` on failure or when no service)
///
/// # Returns
/// * `Ok(AddMemoryResult)` with the parent memory_id and whether at least
///   one chunk got an embedding (`embedding_generated`)
/// * `Err(String)` on database failure
pub async fn add_memory_core(
    params: AddMemoryParams,
    db: &DBClient,
    embedding_service: Option<&Arc<EmbeddingService>>,
) -> Result<AddMemoryResult, String> {
    let memory_id = Uuid::new_v4().to_string();

    // Always create the parent row first — chunks reference it by record link.
    create_memory_parent(db, &memory_id, &params).await?;

    let chunks = split_recursive(&params.content, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);
    let chunk_count = chunks.len();
    let mut any_embedding = false;

    for (idx, chunk_text) in chunks.iter().enumerate() {
        let embedding = match embedding_service {
            Some(svc) => match svc.embed(chunk_text).await {
                Ok(v) => {
                    any_embedding = true;
                    Some(v)
                }
                Err(e) => {
                    warn!(error = %e, chunk_index = idx, "Embedding failed; storing chunk without embedding");
                    None
                }
            },
            None => None,
        };
        create_memory_chunk(db, &memory_id, idx, chunk_count, chunk_text, embedding).await?;
    }

    info!(
        memory_id = %memory_id,
        memory_type = %params.memory_type,
        chunks = chunk_count,
        embedded_any = any_embedding,
        workflow_id = ?params.workflow_id,
        "Memory created via helper (parent + N chunks)"
    );

    Ok(AddMemoryResult {
        memory_id,
        embedding_generated: any_embedding,
    })
}

/// Inserts the parent `memory` row (no embedding) and sets `expires_at`
/// separately when present.
async fn create_memory_parent(
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
    );

    db.create("memory", memory_id, memory)
        .await
        .map_err(|e| format!("Failed to create memory: {}", e))?;

    // Set expires_at separately using datetime cast (SurrealDB rejects ISO strings for datetime fields)
    set_expires_at_if_present(db, memory_id, params.expires_at).await?;

    Ok(())
}

/// Creates a single `memory_chunk` row linked to the given parent. The
/// `memory_id` field is a typed `record<memory>` — we use a SurrealQL
/// record-id literal in the CREATE so the schema's type check passes.
pub(crate) async fn create_memory_chunk(
    db: &DBClient,
    parent_memory_id: &str,
    chunk_index: usize,
    chunk_count: usize,
    content: &str,
    embedding: Option<Vec<f32>>,
) -> Result<(), String> {
    let chunk_id = Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "chunk_index": chunk_index,
        "chunk_count": chunk_count,
        "content": content,
        "embedding": embedding,
    });
    let payload = sanitize_for_surrealdb(payload);

    // Inject the record link via a SurrealQL literal (`memory:`<id>``). Using
    // CONTENT $data here would try to coerce the string to a record at type-
    // check time which SurrealDB rejects — the SET form lets us write the
    // record-id literal directly and bind the rest as JSON.
    let query = format!(
        "CREATE memory_chunk:`{}` SET memory_id = memory:`{}`, \
         chunk_index = $data.chunk_index, \
         chunk_count = $data.chunk_count, \
         content = $data.content, \
         embedding = $data.embedding",
        chunk_id, parent_memory_id
    );
    db.execute_with_params(&query, vec![("data".to_string(), payload)])
        .await
        .map_err(|e| format!("Failed to create memory_chunk: {}", e))?;
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

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;
