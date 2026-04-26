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

//! MemoryTool struct, constructor, and Tool trait implementation.
//!
//! Operation logic is in [`super::operations`], input parsing in [`super::input`],
//! and the tool definition in [`super::definition`].

use super::definition::build_definition;
use super::input::MemoryInput;
use super::operations::{self, MemoryContext};
use super::operations_query;
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::tools::constants::memory::{DEFAULT_LIMIT, DEFAULT_SIMILARITY_THRESHOLD};
use crate::tools::{Tool, ToolDefinition, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Tool for managing agent memories with semantic search.
///
/// This tool allows agents to:
/// - Store memories with automatic embedding generation
/// - Retrieve memories by ID or semantic similarity
/// - Manage workflow-scoped vs general memories
/// - Search using vector similarity (HNSW index)
///
/// # Scope
///
/// MemoryTool uses auto-scoping based on memory type:
/// - `user_pref` and `knowledge` are stored as **general** (cross-workflow)
/// - `context` and `decision` are stored as **workflow-scoped**
///
/// Agents can override auto-scoping via the `scope` parameter.
///
/// # Embedding Support
///
/// If an EmbeddingService is configured, memories are stored with vector
/// embeddings enabling semantic search. Without embeddings, only text-based
/// filtering is available.
pub struct MemoryTool {
    /// Database client for persistence
    db: Arc<DBClient>,
    /// Embedding service for vector generation (optional)
    embedding_service: Option<Arc<EmbeddingService>>,
    /// Default workflow ID set at creation, immutable
    default_workflow_id: Option<String>,
    /// Agent ID using this tool
    agent_id: String,
}

impl MemoryTool {
    /// Creates a new MemoryTool.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `embedding_service` - Optional embedding service (None = text search only)
    /// * `default_workflow_id` - Workflow ID set at creation (used for auto-scoping)
    /// * `agent_id` - Agent ID using this tool
    pub fn new(
        db: Arc<DBClient>,
        embedding_service: Option<Arc<EmbeddingService>>,
        default_workflow_id: Option<String>,
        agent_id: String,
    ) -> Self {
        Self {
            db,
            embedding_service,
            default_workflow_id,
            agent_id,
        }
    }
}

#[async_trait]
impl Tool for MemoryTool {
    fn id(&self) -> &str {
        "MemoryTool"
    }

    /// Returns the tool definition with LLM-friendly description.
    fn definition(&self) -> ToolDefinition {
        build_definition()
    }

    /// Executes the tool with JSON input.
    #[instrument(skip(self, input), fields(agent_id = %self.agent_id))]
    async fn execute(&self, input: Value) -> ToolResult<Value> {
        // Parse and validate input once using MemoryInput
        let params = MemoryInput::from_json(&input)?;
        params.validate()?;

        debug!(operation = %params.operation, "Executing MemoryTool");

        let ctx = MemoryContext {
            db: &self.db,
            embedding_service: &self.embedding_service,
            default_workflow_id: &self.default_workflow_id,
            agent_id: &self.agent_id,
        };

        // Dispatch to operation handlers using pre-parsed params
        // Fields are guaranteed to be present after validation
        match params.operation.as_str() {
            "add" => {
                let memory_type = params
                    .memory_type
                    .as_deref()
                    .expect("BUG: validate_add() must ensure memory_type is Some");
                let content = params
                    .content
                    .as_deref()
                    .expect("BUG: validate_add() must ensure content is Some");
                operations::add_memory(
                    &params,
                    memory_type,
                    content,
                    params.metadata.clone(),
                    params.tags.clone(),
                    &ctx,
                )
                .await
            }

            "get" => {
                operations::get_memory(
                    params
                        .memory_id
                        .as_deref()
                        .expect("BUG: validate_get_or_delete() must ensure memory_id is Some"),
                    &ctx,
                )
                .await
            }

            "describe" => {
                let scope = params.scope.as_deref().unwrap_or("both");
                operations_query::describe_memories(&params, scope, &ctx).await
            }

            "list" => {
                let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
                let scope = params.scope.as_deref().unwrap_or("both");
                let mode = params.mode.as_deref().unwrap_or("full");
                operations_query::list_memories(
                    &params,
                    params.type_filter.as_deref(),
                    limit,
                    scope,
                    mode,
                    &ctx,
                )
                .await
            }

            "search" => {
                let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
                let threshold = params.threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);
                let scope = params.scope.as_deref().unwrap_or("both");
                operations_query::search_memories(
                    &params,
                    params
                        .query
                        .as_deref()
                        .expect("BUG: validate_search() must ensure query is Some"),
                    limit,
                    params.type_filter.as_deref(),
                    threshold,
                    scope,
                    &ctx,
                )
                .await
            }

            "delete" => {
                operations_query::delete_memory(
                    params
                        .memory_id
                        .as_deref()
                        .expect("BUG: validate_get_or_delete() must ensure memory_id is Some"),
                    &ctx,
                )
                .await
            }

            "clear_by_type" => {
                operations_query::clear_by_type(
                    &params,
                    params
                        .memory_type
                        .as_deref()
                        .expect("BUG: validate_clear_by_type() must ensure memory_type is Some"),
                    &ctx,
                )
                .await
            }

            // SAFETY: validate() rejects unknown operations, this branch is unreachable
            _ => unreachable!("Unknown operation should be caught by validate()"),
        }
    }

    /// Validates input before execution (trait requirement).
    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        let parsed = MemoryInput::from_json(input)?;
        parsed.validate()
    }
}
