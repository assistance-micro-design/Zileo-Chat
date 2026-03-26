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

use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::{MemoryCreate, MemoryCreateWithEmbedding, MemoryType};
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
