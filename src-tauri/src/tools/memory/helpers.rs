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
//! and the memory Tauri commands. It eliminates code duplication for the
//! core add_memory operation with embedding support.

use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::{MemoryCreate, MemoryCreateWithEmbedding, MemoryType};
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
                // Create memory with embedding
                let memory = match &params.workflow_id {
                    Some(wf_id) => MemoryCreateWithEmbedding::with_workflow(
                        params.memory_type.clone(),
                        params.content.clone(),
                        embedding,
                        params.metadata.clone(),
                        wf_id.clone(),
                    ),
                    None => MemoryCreateWithEmbedding::new(
                        params.memory_type.clone(),
                        params.content.clone(),
                        embedding,
                        params.metadata.clone(),
                    ),
                };

                db.create("memory", &memory_id, memory)
                    .await
                    .map_err(|e| format!("Failed to create memory with embedding: {}", e))?;

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
    let memory = match &params.workflow_id {
        Some(wf_id) => MemoryCreate::with_workflow(
            params.memory_type.clone(),
            params.content.clone(),
            params.metadata.clone(),
            wf_id.clone(),
        ),
        None => MemoryCreate::new(
            params.memory_type.clone(),
            params.content.clone(),
            params.metadata.clone(),
        ),
    };

    db.create("memory", memory_id, memory)
        .await
        .map_err(|e| format!("Failed to create memory: {}", e))?;

    Ok(())
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
        };

        assert_eq!(params.memory_type, MemoryType::Knowledge);
        assert_eq!(params.content, "Test content");
        assert!(params.workflow_id.is_some());
    }

    #[test]
    fn test_add_memory_params_general_scope() {
        let params = AddMemoryParams {
            memory_type: MemoryType::Context,
            content: "General memory".to_string(),
            metadata: serde_json::json!({}),
            workflow_id: None,
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
