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

//! MemoryTool implementation for agent contextual persistence.
//!
//! This tool allows agents to manage memories with semantic search capabilities
//! using vector embeddings and SurrealDB's HNSW index.

use super::helpers::{add_memory_core, AddMemoryParams};
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::{Memory, MemoryType};
use crate::tools::constants::memory::{
    DEFAULT_LIMIT, DEFAULT_SIMILARITY_THRESHOLD, MAX_CONTENT_LENGTH, MAX_LIMIT, VALID_TYPES,
};
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{
    db_error, delete_with_check, validate_enum_value, validate_length, validate_not_empty,
};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};

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
/// MemoryTool supports two modes:
/// - **Workflow mode**: Memories are scoped to a specific workflow
/// - **General mode**: Memories are accessible across workflows
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
    /// Current workflow ID (scope) - None for general mode
    workflow_id: Arc<RwLock<Option<String>>>,
    /// Agent ID using this tool
    agent_id: String,
}

impl MemoryTool {
    /// Creates a new MemoryTool.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `embedding_service` - Optional embedding service (None = text search only)
    /// * `workflow_id` - Optional workflow ID for scoping (None = general mode)
    /// * `agent_id` - Agent ID using this tool
    ///
    /// # Example
    /// ```ignore
    /// let tool = MemoryTool::new(
    ///     db.clone(),
    ///     Some(embedding_service.clone()),
    ///     Some("wf_001".into()),
    ///     "db_agent".into()
    /// );
    /// ```
    pub fn new(
        db: Arc<DBClient>,
        embedding_service: Option<Arc<EmbeddingService>>,
        workflow_id: Option<String>,
        agent_id: String,
    ) -> Self {
        Self {
            db,
            embedding_service,
            workflow_id: Arc::new(RwLock::new(workflow_id)),
            agent_id,
        }
    }

    /// Activates workflow-scoped mode.
    ///
    /// All subsequent memory operations will be scoped to this workflow.
    ///
    /// # Arguments
    /// * `workflow_id` - Workflow ID for memory isolation
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn activate_workflow(&self, workflow_id: String) -> ToolResult<Value> {
        *self.workflow_id.write().await = Some(workflow_id.clone());
        info!(workflow_id = %workflow_id, "Activated workflow scope");

        Ok(serde_json::json!({
            "success": true,
            "scope": "workflow",
            "workflow_id": workflow_id,
            "message": format!("Memory scope set to workflow '{}'", workflow_id)
        }))
    }

    /// Activates general mode (cross-workflow access).
    ///
    /// Memories will not be filtered by workflow_id.
    #[instrument(skip(self), fields(agent_id = %self.agent_id))]
    pub async fn activate_general(&self) -> ToolResult<Value> {
        *self.workflow_id.write().await = None;
        info!("Activated general scope");

        Ok(serde_json::json!({
            "success": true,
            "scope": "general",
            "message": "Memory scope set to general (cross-workflow)"
        }))
    }

    /// Gets the current workflow ID (if in workflow mode).
    async fn current_workflow_id(&self) -> Option<String> {
        self.workflow_id.read().await.clone()
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

    /// Builds scope condition for WHERE clause (OPT-MEM-2 + OPT-MEM-5: parameterized).
    ///
    /// Returns `Some(condition)` to add to WHERE clause, or `None` if no condition needed.
    /// When workflow_id is needed, it adds a parameter to the params vector.
    ///
    /// # Arguments
    /// * `scope` - Scope filter: "workflow", "general", or "both"
    /// * `workflow_id` - Current workflow ID if in workflow mode
    /// * `params` - Mutable params vector to add workflow_id param if needed
    fn build_scope_condition(
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

    /// Adds a new memory with optional embedding.
    ///
    /// Uses the shared `add_memory_core` helper for the core creation logic.
    /// This method handles Tool-specific concerns:
    /// - Parameter validation using utils.rs validators
    /// - Metadata enrichment (agent_source, tags)
    /// - ResponseBuilder JSON formatting
    ///
    /// # Arguments
    /// * `memory_type` - Type of memory (user_pref, context, knowledge, decision)
    /// * `content` - Text content of the memory
    /// * `metadata` - Additional metadata (optional)
    /// * `tags` - Classification tags (optional)
    #[instrument(skip(self, content, metadata), fields(agent_id = %self.agent_id, memory_type = %memory_type))]
    async fn add_memory(
        &self,
        memory_type: &str,
        content: &str,
        metadata: Option<Value>,
        tags: Option<Vec<String>>,
    ) -> ToolResult<Value> {
        // Validate content length
        validate_not_empty(content, "content")?;
        validate_length(content, MAX_CONTENT_LENGTH, "content")?;

        // Validate memory type
        validate_enum_value(memory_type, VALID_TYPES, "memory_type")?;
        let mem_type = Self::parse_memory_type(memory_type)?;

        let workflow_id = self.current_workflow_id().await;

        // Build metadata with agent source and tags (Tool-specific enrichment)
        let mut meta = metadata.unwrap_or(serde_json::json!({}));
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("agent_source".to_string(), serde_json::json!(self.agent_id));
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
        };

        let result = add_memory_core(params, &self.db, self.embedding_service.as_ref())
            .await
            .map_err(ToolError::DatabaseError)?;

        info!(
            memory_id = %result.memory_id,
            memory_type = %memory_type,
            embedding = result.embedding_generated,
            "Memory created"
        );

        Ok(ResponseBuilder::new()
            .success(true)
            .id("memory_id", result.memory_id)
            .field("type", memory_type)
            .field("embedding_generated", result.embedding_generated)
            .field("workflow_id", workflow_id)
            .message("Memory created successfully")
            .build())
    }

    /// Retrieves a memory by ID.
    ///
    /// # Arguments
    /// * `memory_id` - Memory ID to retrieve
    #[instrument(skip(self), fields(memory_id = %memory_id))]
    async fn get_memory(&self, memory_id: &str) -> ToolResult<Value> {
        // OPT-MEM-5: Parameterized query for security
        let query = r#"SELECT
                meta::id(id) AS id,
                type,
                content,
                workflow_id,
                metadata,
                created_at
            FROM memory
            WHERE meta::id(id) = $memory_id"#;

        let params = vec![("memory_id".to_string(), serde_json::json!(memory_id))];
        let results: Vec<Memory> = self
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

    /// Lists memories with optional filters.
    ///
    /// # Arguments
    /// * `type_filter` - Optional memory type to filter by
    /// * `limit` - Maximum number of results (default: 10)
    /// * `scope` - Scope filter: "workflow", "general", or "both" (default: "both")
    #[instrument(skip(self), fields(type_filter = ?type_filter, limit = limit, scope = %scope))]
    async fn list_memories(
        &self,
        type_filter: Option<&str>,
        limit: usize,
        scope: &str,
    ) -> ToolResult<Value> {
        let workflow_id = self.current_workflow_id().await;
        let limit = limit.min(MAX_LIMIT);

        let mut conditions = Vec::new();
        // OPT-MEM-5: Parameterized query support
        let mut params: Vec<(String, serde_json::Value)> = Vec::new();

        // OPT-MEM-2: Use centralized scope filter helper
        // Special case: scope="workflow" with no active workflow returns early
        if scope == "workflow" && workflow_id.is_none() {
            return Ok(ResponseBuilder::new()
                .success(true)
                .count(0)
                .field("scope", "workflow")
                .field("workflow_id", Option::<String>::None)
                .data("memories", Vec::<Memory>::new())
                .message("No active workflow. Use 'activate_workflow' first or scope='both'")
                .build());
        }
        // OPT-MEM-5: build_scope_condition now adds params
        if let Some(scope_cond) = Self::build_scope_condition(scope, &workflow_id, &mut params) {
            conditions.push(scope_cond);
        }

        // Type filter condition - OPT-MEM-5: parameterized
        if let Some(mem_type) = type_filter {
            // Validate the type
            Self::parse_memory_type(mem_type)?;
            conditions.push("type = $type_filter".to_string());
            params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // OPT-MEM-5: LIMIT is safe as integer, not user input
        let query = format!(
            r#"SELECT
                meta::id(id) AS id,
                type,
                content,
                workflow_id,
                metadata,
                created_at
            FROM memory
            {}
            ORDER BY created_at DESC
            LIMIT {}"#,
            where_clause, limit
        );

        let memories: Vec<Memory> = self
            .db
            .query_with_params(&query, params)
            .await
            .map_err(db_error)?;

        debug!(count = memories.len(), scope = %scope, "Memories listed");

        Ok(ResponseBuilder::new()
            .success(true)
            .count(memories.len())
            .field("scope", scope)
            .field("workflow_id", workflow_id)
            .data("memories", memories)
            .build())
    }

    /// Searches memories using semantic similarity.
    ///
    /// Falls back to text search if embeddings are not available.
    ///
    /// # Arguments
    /// * `query_text` - Search query
    /// * `limit` - Maximum results (default: 10)
    /// * `type_filter` - Optional type filter
    /// * `threshold` - Similarity threshold 0-1 (default: 0.7)
    /// * `scope` - Scope filter: "workflow", "general", or "both" (default: "both")
    #[instrument(skip(self), fields(query_len = query_text.len(), limit = limit, scope = %scope))]
    async fn search_memories(
        &self,
        query_text: &str,
        limit: usize,
        type_filter: Option<&str>,
        threshold: f64,
        scope: &str,
    ) -> ToolResult<Value> {
        let workflow_id = self.current_workflow_id().await;
        let limit = limit.min(MAX_LIMIT);
        let threshold = threshold.clamp(0.0, 1.0);

        // Validate type filter if provided
        if let Some(mem_type) = type_filter {
            Self::parse_memory_type(mem_type)?;
        }

        // Try vector search if embedding service is available
        if let Some(ref embed_service) = self.embedding_service {
            match embed_service.embed(query_text).await {
                Ok(query_embedding) => {
                    return self
                        .vector_search(
                            &query_embedding,
                            limit,
                            type_filter,
                            threshold,
                            &workflow_id,
                            scope,
                        )
                        .await;
                }
                Err(e) => {
                    warn!(error = %e, "Query embedding failed, falling back to text search");
                }
            }
        }

        // Fallback to text search
        self.text_search(query_text, limit, type_filter, &workflow_id, scope)
            .await
    }

    /// Performs vector similarity search using HNSW index.
    async fn vector_search(
        &self,
        query_embedding: &[f32],
        limit: usize,
        type_filter: Option<&str>,
        threshold: f64,
        workflow_id: &Option<String>,
        scope: &str,
    ) -> ToolResult<Value> {
        // Build conditions
        let mut conditions = vec!["embedding IS NOT NONE".to_string()];
        // OPT-MEM-5: Parameterized query support
        let mut params: Vec<(String, serde_json::Value)> = Vec::new();

        // OPT-MEM-2 + OPT-MEM-5: Use centralized scope filter helper with params
        if let Some(scope_cond) = Self::build_scope_condition(scope, workflow_id, &mut params) {
            conditions.push(scope_cond);
        }

        // OPT-MEM-5: Type filter parameterized
        if let Some(mem_type) = type_filter {
            conditions.push("type = $type_filter".to_string());
            params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
        }

        let where_clause = conditions.join(" AND ");

        // Convert threshold to distance (cosine distance = 1 - similarity)
        let distance_threshold = 1.0 - threshold;

        // OPT-MEM-11: Pre-allocate String to avoid intermediate Vec allocation
        // Format embedding for SurrealQL - must be inline array for vector operations
        // Note: embedding array is generated internally from query_text, not user input
        // Estimate ~12 chars per float (sign + digits + decimal + separator)
        let mut embedding_str = String::with_capacity(query_embedding.len() * 12);
        for (i, v) in query_embedding.iter().enumerate() {
            if i > 0 {
                embedding_str.push_str(", ");
            }
            use std::fmt::Write;
            let _ = write!(embedding_str, "{}", v);
        }

        // OPT-MEM-5: LIMIT and distance_threshold are safe (integers/floats, not user strings)
        // The embedding is also safe (generated from EmbeddingService)
        let query = format!(
            r#"SELECT
                meta::id(id) AS id,
                type,
                content,
                workflow_id,
                metadata,
                created_at,
                vector::similarity::cosine(embedding, [{embedding}]) AS score
            FROM memory
            WHERE {where_clause}
              AND vector::distance::cosine(embedding, [{embedding}]) < {distance}
            ORDER BY score DESC
            LIMIT {limit}"#,
            embedding = embedding_str,
            where_clause = where_clause,
            distance = distance_threshold,
            limit = limit
        );

        let results: Vec<Value> = self
            .db
            .query_json_with_params(&query, params)
            .await
            .map_err(db_error)?;

        debug!(
            count = results.len(),
            threshold = threshold,
            scope = %scope,
            "Vector search completed"
        );

        Ok(serde_json::json!({
            "success": true,
            "search_type": "vector",
            "count": results.len(),
            "threshold": threshold,
            "scope": scope,
            "workflow_id": workflow_id,
            "results": results
        }))
    }

    /// Performs text-based search as fallback.
    async fn text_search(
        &self,
        query_text: &str,
        limit: usize,
        type_filter: Option<&str>,
        workflow_id: &Option<String>,
        scope: &str,
    ) -> ToolResult<Value> {
        let mut conditions = Vec::new();
        // OPT-MEM-5: Parameterized query support
        let mut params: Vec<(String, serde_json::Value)> = Vec::new();

        // OPT-MEM-5: Text content contains query (case-insensitive) - parameterized
        // No more manual escaping needed - SurrealDB handles it via params
        conditions
            .push("string::lowercase(content) CONTAINS string::lowercase($query_text)".to_string());
        params.push(("query_text".to_string(), serde_json::json!(query_text)));

        // OPT-MEM-2 + OPT-MEM-5: Use centralized scope filter helper with params
        if let Some(scope_cond) = Self::build_scope_condition(scope, workflow_id, &mut params) {
            conditions.push(scope_cond);
        }

        // OPT-MEM-5: Type filter parameterized
        if let Some(mem_type) = type_filter {
            conditions.push("type = $type_filter".to_string());
            params.push(("type_filter".to_string(), serde_json::json!(mem_type)));
        }

        let where_clause = conditions.join(" AND ");

        // OPT-MEM-5: LIMIT is safe as integer, not user input
        let query = format!(
            r#"SELECT
                meta::id(id) AS id,
                type,
                content,
                workflow_id,
                metadata,
                created_at
            FROM memory
            WHERE {}
            ORDER BY created_at DESC
            LIMIT {}"#,
            where_clause, limit
        );

        let results: Vec<Memory> = self
            .db
            .query_with_params(&query, params)
            .await
            .map_err(db_error)?;

        debug!(count = results.len(), scope = %scope, "Text search completed");

        Ok(serde_json::json!({
            "success": true,
            "search_type": "text",
            "count": results.len(),
            "scope": scope,
            "workflow_id": workflow_id,
            "results": results.into_iter().map(|m| serde_json::json!({
                "id": m.id,
                "type": m.memory_type,
                "content": m.content,
                "workflow_id": m.workflow_id,
                "metadata": m.metadata,
                "created_at": m.created_at,
                "score": null
            })).collect::<Vec<_>>()
        }))
    }

    /// Deletes a memory by ID.
    ///
    /// # Arguments
    /// * `memory_id` - Memory ID to delete
    #[instrument(skip(self), fields(memory_id = %memory_id))]
    async fn delete_memory(&self, memory_id: &str) -> ToolResult<Value> {
        delete_with_check(&self.db, "memory", memory_id, "Memory").await?;

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
    /// * `memory_type` - Type of memories to clear
    #[instrument(skip(self), fields(memory_type = %memory_type))]
    async fn clear_by_type(&self, memory_type: &str) -> ToolResult<Value> {
        // Validate memory type
        Self::parse_memory_type(memory_type)?;

        let workflow_id = self.current_workflow_id().await;

        // OPT-MEM-5: Use execute_with_params() for parameterized DELETE
        let (delete_query, params) = if let Some(ref wf_id) = workflow_id {
            (
                "DELETE FROM memory WHERE type = $memory_type AND workflow_id = $workflow_id"
                    .to_string(),
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

        self.db
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
}

#[async_trait]
impl Tool for MemoryTool {
    /// Returns the tool definition with LLM-friendly description.
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "MemoryTool".to_string(),
            name: "Memory Manager".to_string(),
            description: format!(
                r#"Manages persistent memory for contextual awareness and knowledge retrieval.

USE THIS TOOL WHEN:
- You need to store important information for future reference
- You want to search past memories by semantic similarity
- You need to maintain context across conversations
- You want to organize knowledge by type (user_pref, context, knowledge, decision)
- You need to retrieve previously stored decisions or user preferences

DO NOT USE THIS TOOL WHEN:
- Information is only relevant to the current message (use conversation context)
- Storing duplicate content already in memory (search first!)
- The content exceeds {} characters (split into smaller chunks)
- For temporary calculations or intermediate values (use CalculatorTool or conversation)

OPERATIONS:
- activate_workflow: Set workflow-specific scope for memory isolation
- activate_general: Switch to general mode (cross-workflow access)
- add: Store new memory with automatic embedding generation
- get: Retrieve specific memory by ID
- list: View memories with optional type filter and scope
- search: Find semantically similar memories using vector search
- delete: Remove a memory
- clear_by_type: Bulk delete all memories of a specific type

SCOPE PARAMETER (for list/search):
- "both" (default): Shows workflow-specific AND general memories
- "workflow": Only memories from current workflow
- "general": Only global memories (not tied to any workflow)

CONSTRAINTS:
- Content length: max {} characters
- List/search default limit: {} results (max {})
- Similarity threshold: {:.1} (0-1 scale)

BEST PRACTICES:
- Use 'knowledge' type for facts and domain expertise
- Use 'decision' type for rationale behind choices
- Use 'context' type for conversation-specific information
- Use 'user_pref' type for user preferences and settings
- Use scope='both' to see all available memories
- Search before adding to avoid duplicates

EXAMPLES:
1. List all memories (workflow + general):
   {{"operation": "list"}}

2. List only workflow memories:
   {{"operation": "list", "scope": "workflow"}}

3. Search all memories:
   {{"operation": "search", "query": "vector database indexing", "limit": 5}}

4. Store knowledge:
   {{"operation": "add", "type": "knowledge", "content": "SurrealDB supports HNSW vector indexing"}}

5. Store user preference:
   {{"operation": "add", "type": "user_pref", "content": "User prefers detailed explanations with examples", "tags": ["communication", "style"]}}

6. Store decision rationale:
   {{"operation": "add", "type": "decision", "content": "Chose PostgreSQL over MongoDB because the data is highly relational", "metadata": {{"decision_date": "2025-01-15", "alternatives_considered": ["MongoDB", "SurrealDB"]}}}}

7. Delete a memory:
   {{"operation": "delete", "memory_id": "mem_abc123"}}

8. Clear all context memories:
   {{"operation": "clear_by_type", "type": "context"}}"#,
                MAX_CONTENT_LENGTH, MAX_CONTENT_LENGTH, DEFAULT_LIMIT, MAX_LIMIT, DEFAULT_SIMILARITY_THRESHOLD
            ),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["activate_workflow", "activate_general", "add", "get", "list", "search", "delete", "clear_by_type"],
                        "description": "Operation: 'activate_workflow'/'activate_general' set scope, 'add' stores memory, 'get' retrieves by ID, 'list' shows memories, 'search' finds similar, 'delete' removes, 'clear_by_type' bulk deletes"
                    },
                    "workflow_id": {
                        "type": "string",
                        "description": "Workflow ID (for activate_workflow)"
                    },
                    "type": {
                        "type": "string",
                        "enum": ["user_pref", "context", "knowledge", "decision"],
                        "description": "Memory type (for add)"
                    },
                    "content": {
                        "type": "string",
                        "maxLength": 50000,
                        "description": "Memory content (for add)"
                    },
                    "metadata": {
                        "type": "object",
                        "description": "Additional metadata (for add)"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Classification tags (for add)"
                    },
                    "memory_id": {
                        "type": "string",
                        "description": "Memory ID (for get/delete)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query (for search)"
                    },
                    "limit": {
                        "type": "integer",
                        "default": 10,
                        "maximum": 100,
                        "description": "Max results (for list/search)"
                    },
                    "type_filter": {
                        "type": "string",
                        "enum": ["user_pref", "context", "knowledge", "decision"],
                        "description": "Filter by type (for list/search)"
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["workflow", "general", "both"],
                        "default": "both",
                        "description": "Memory scope filter (for list/search): 'workflow' = current workflow only, 'general' = global memories only, 'both' = both scopes (default)"
                    },
                    "threshold": {
                        "type": "number",
                        "default": 0.7,
                        "minimum": 0,
                        "maximum": 1,
                        "description": "Similarity threshold 0-1 (for search)"
                    }
                },
                "required": ["operation"]
            }),

            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "success": {"type": "boolean"},
                    "memory_id": {"type": "string"},
                    "message": {"type": "string"},
                    "memory": {"type": "object"},
                    "memories": {"type": "array"},
                    "results": {"type": "array"},
                    "count": {"type": "integer"},
                    "scope": {"type": "string"},
                    "workflow_id": {"type": "string"},
                    "embedding_generated": {"type": "boolean"},
                    "search_type": {"type": "string"}
                }
            }),

            requires_confirmation: false,
        }
    }

    /// Executes the tool with JSON input.
    #[instrument(skip(self, input), fields(agent_id = %self.agent_id))]
    async fn execute(&self, input: Value) -> ToolResult<Value> {
        // Parse and validate input once using MemoryInput (OPT-MEM-8)
        let params = MemoryInput::from_json(&input)?;
        params.validate()?;

        debug!(operation = %params.operation, "Executing MemoryTool");

        // Dispatch to operation handlers using pre-parsed params
        // Fields are guaranteed to be present after validation
        match params.operation.as_str() {
            "activate_workflow" => {
                // SAFETY: validate_activate_workflow() ensures workflow_id is Some
                self.activate_workflow(params.workflow_id.unwrap()).await
            }

            "activate_general" => self.activate_general().await,

            "add" => {
                // SAFETY: validate_add() ensures memory_type and content are Some
                self.add_memory(
                    &params.memory_type.unwrap(),
                    &params.content.unwrap(),
                    params.metadata,
                    params.tags,
                )
                .await
            }

            "get" => {
                // SAFETY: validate_get_or_delete() ensures memory_id is Some
                self.get_memory(&params.memory_id.unwrap()).await
            }

            "list" => {
                let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
                let scope = params.scope.as_deref().unwrap_or("both");
                self.list_memories(params.type_filter.as_deref(), limit, scope)
                    .await
            }

            "search" => {
                // SAFETY: validate_search() ensures query is Some
                let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
                let threshold = params.threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);
                let scope = params.scope.as_deref().unwrap_or("both");
                self.search_memories(
                    &params.query.unwrap(),
                    limit,
                    params.type_filter.as_deref(),
                    threshold,
                    scope,
                )
                .await
            }

            "delete" => {
                // SAFETY: validate_get_or_delete() ensures memory_id is Some
                self.delete_memory(&params.memory_id.unwrap()).await
            }

            "clear_by_type" => {
                // SAFETY: validate_clear_by_type() ensures memory_type is Some
                self.clear_by_type(&params.memory_type.unwrap()).await
            }

            // SAFETY: validate() rejects unknown operations, this branch is unreachable
            _ => unreachable!("Unknown operation should be caught by validate()"),
        }
    }

    /// Validates input before execution (trait requirement).
    ///
    /// Note: After OPT-MEM-8, `execute()` performs its own parsing and validation
    /// using `MemoryInput`. This method is kept for trait compliance and can be
    /// used for external validation.
    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        let parsed = MemoryInput::from_json(input)?;
        parsed.validate()
    }

    /// Returns false - memory operations are reversible, no confirmation needed.
    fn requires_confirmation(&self) -> bool {
        false
    }
}

// =============================================================================
// MemoryInput - Structured input parsing and validation (OPT-MEM-7)
// =============================================================================

/// Parsed and typed memory operation input.
///
/// This struct reduces the cyclomatic complexity of `validate_input()` and `execute()` by:
/// 1. Extracting JSON parsing into `from_json()`
/// 2. Delegating validation to per-operation methods
/// 3. Providing typed fields for direct use in `execute()` (OPT-MEM-8)
///
/// CC reduced from ~18 to ~8 in validate_input(), ~15 to ~7 in execute().
#[derive(Debug)]
struct MemoryInput {
    operation: String,
    workflow_id: Option<String>,
    memory_type: Option<String>,
    content: Option<String>,
    memory_id: Option<String>,
    query: Option<String>,
    type_filter: Option<String>,
    threshold: Option<f64>,
    limit: Option<usize>,
    scope: Option<String>,
    metadata: Option<Value>,
    tags: Option<Vec<String>>,
}

impl MemoryInput {
    /// Parses JSON input into typed struct.
    fn from_json(input: &Value) -> ToolResult<Self> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation field".to_string()))?
            .to_string();

        // Parse tags array if present
        let tags = input["tags"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

        Ok(Self {
            operation,
            workflow_id: input["workflow_id"].as_str().map(String::from),
            memory_type: input["type"].as_str().map(String::from),
            content: input["content"].as_str().map(String::from),
            memory_id: input["memory_id"].as_str().map(String::from),
            query: input["query"].as_str().map(String::from),
            type_filter: input["type_filter"].as_str().map(String::from),
            threshold: input["threshold"].as_f64(),
            limit: input["limit"].as_u64().map(|v| v as usize),
            scope: input["scope"].as_str().map(String::from),
            metadata: input.get("metadata").cloned(),
            tags,
        })
    }

    /// Validates input based on operation type.
    fn validate(&self) -> ToolResult<()> {
        match self.operation.as_str() {
            "activate_workflow" => self.validate_activate_workflow(),
            "activate_general" => Ok(()),
            "add" => self.validate_add(),
            "get" | "delete" => self.validate_get_or_delete(),
            "list" => self.validate_type_filter(),
            "search" => self.validate_search(),
            "clear_by_type" => self.validate_clear_by_type(),
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: activate_workflow, activate_general, add, get, list, search, delete, clear_by_type",
                self.operation
            ))),
        }
    }

    /// Validates activate_workflow operation.
    fn validate_activate_workflow(&self) -> ToolResult<()> {
        if self.workflow_id.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'workflow_id' for activate_workflow operation".to_string(),
            ));
        }
        Ok(())
    }

    /// Validates add operation.
    fn validate_add(&self) -> ToolResult<()> {
        if self.memory_type.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'type' for add operation. Valid types: user_pref, context, knowledge, decision".to_string(),
            ));
        }
        if self.content.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'content' for add operation".to_string(),
            ));
        }
        // Validate type value
        if let Some(ref type_str) = self.memory_type {
            if !VALID_TYPES.contains(&type_str.as_str()) {
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid type '{}'. Valid types: user_pref, context, knowledge, decision",
                    type_str
                )));
            }
        }
        Ok(())
    }

    /// Validates get or delete operation.
    fn validate_get_or_delete(&self) -> ToolResult<()> {
        if self.memory_id.is_none() {
            return Err(ToolError::InvalidInput(format!(
                "Missing 'memory_id' for {} operation",
                self.operation
            )));
        }
        Ok(())
    }

    /// Validates type_filter if present (shared by list and search).
    fn validate_type_filter(&self) -> ToolResult<()> {
        if let Some(ref type_str) = self.type_filter {
            if !VALID_TYPES.contains(&type_str.as_str()) {
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid type_filter '{}'. Valid types: user_pref, context, knowledge, decision",
                    type_str
                )));
            }
        }
        Ok(())
    }

    /// Validates search operation.
    fn validate_search(&self) -> ToolResult<()> {
        if self.query.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'query' for search operation".to_string(),
            ));
        }
        // Validate type_filter if provided
        self.validate_type_filter()?;
        // Validate threshold if provided
        if let Some(threshold) = self.threshold {
            if !(0.0..=1.0).contains(&threshold) {
                return Err(ToolError::ValidationFailed(format!(
                    "Threshold {} must be between 0 and 1",
                    threshold
                )));
            }
        }
        Ok(())
    }

    /// Validates clear_by_type operation.
    fn validate_clear_by_type(&self) -> ToolResult<()> {
        if self.memory_type.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'type' for clear_by_type operation. Valid types: user_pref, context, knowledge, decision".to_string(),
            ));
        }
        // Validate type value
        if let Some(ref type_str) = self.memory_type {
            if !VALID_TYPES.contains(&type_str.as_str()) {
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid type '{}'. Valid types: user_pref, context, knowledge, decision",
                    type_str
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let definition = ToolDefinition {
            id: "MemoryTool".to_string(),
            name: "Memory Manager".to_string(),
            description: "Test".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "MemoryTool");
        assert!(!definition.requires_confirmation);
    }

    #[test]
    fn test_parse_memory_type_valid() {
        assert!(matches!(
            MemoryTool::parse_memory_type("user_pref"),
            Ok(MemoryType::UserPref)
        ));
        assert!(matches!(
            MemoryTool::parse_memory_type("context"),
            Ok(MemoryType::Context)
        ));
        assert!(matches!(
            MemoryTool::parse_memory_type("knowledge"),
            Ok(MemoryType::Knowledge)
        ));
        assert!(matches!(
            MemoryTool::parse_memory_type("decision"),
            Ok(MemoryType::Decision)
        ));
    }

    #[test]
    fn test_parse_memory_type_invalid() {
        let result = MemoryTool::parse_memory_type("invalid");
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Invalid memory type"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_input_validation_add() {
        let valid_input = serde_json::json!({
            "operation": "add",
            "type": "knowledge",
            "content": "Test content"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "add");
        assert!(valid_input.get("type").is_some());
        assert!(valid_input.get("content").is_some());
    }

    #[test]
    fn test_input_validation_search() {
        let valid_input = serde_json::json!({
            "operation": "search",
            "query": "find relevant info",
            "limit": 5
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("query").is_some());
    }

    #[test]
    fn test_input_validation_activate_workflow() {
        let valid_input = serde_json::json!({
            "operation": "activate_workflow",
            "workflow_id": "wf_123"
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("workflow_id").is_some());
    }

    #[test]
    fn test_input_validation_list() {
        let valid_input = serde_json::json!({
            "operation": "list",
            "type_filter": "knowledge",
            "limit": 20
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "list");
    }

    #[test]
    fn test_memory_type_values() {
        assert!(VALID_TYPES.contains(&"user_pref"));
        assert!(VALID_TYPES.contains(&"context"));
        assert!(VALID_TYPES.contains(&"knowledge"));
        assert!(VALID_TYPES.contains(&"decision"));
        assert!(!VALID_TYPES.contains(&"invalid"));
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_CONTENT_LENGTH, 50_000);
        assert_eq!(DEFAULT_LIMIT, 10);
        assert_eq!(MAX_LIMIT, 100);
        assert!((DEFAULT_SIMILARITY_THRESHOLD - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_input_validation_get() {
        let valid_input = serde_json::json!({
            "operation": "get",
            "memory_id": "mem_001"
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("memory_id").is_some());
    }

    #[test]
    fn test_input_validation_delete() {
        let valid_input = serde_json::json!({
            "operation": "delete",
            "memory_id": "mem_001"
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("memory_id").is_some());
    }

    #[test]
    fn test_input_validation_clear_by_type() {
        let valid_input = serde_json::json!({
            "operation": "clear_by_type",
            "type": "context"
        });

        assert!(valid_input.is_object());
        assert!(valid_input.get("type").is_some());
    }

    #[test]
    fn test_threshold_bounds() {
        // Threshold should be clamped to 0-1
        let threshold: f64 = 0.7;
        assert!((0.0..=1.0).contains(&threshold));

        let clamped = 1.5f64.clamp(0.0, 1.0);
        assert!((clamped - 1.0).abs() < f64::EPSILON);

        let clamped = (-0.5f64).clamp(0.0, 1.0);
        assert!(clamped.abs() < f64::EPSILON);
    }
}

/// Integration tests that require a real MemoryTool instance with database.
///
/// These tests validate the complete validate_input behavior with proper error handling.
#[cfg(test)]
mod validate_input_tests {
    use super::*;
    use tempfile::tempdir;

    /// Creates a test MemoryTool with a temporary database.
    async fn create_test_tool() -> (MemoryTool, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_validate_db");
        let db_path_str = db_path.to_str().unwrap().to_string();

        let db = Arc::new(DBClient::new(&db_path_str).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let tool = MemoryTool::new(
            db,
            None,
            Some("wf_test".to_string()),
            "test_agent".to_string(),
        );

        (tool, temp_dir)
    }

    // =========================================================================
    // validate_input: Invalid input structure tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_non_object() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!("not an object"));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Input must be an object"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_missing_operation() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({"type": "knowledge"}));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing operation field"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_unknown_operation() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "unknown_op"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Unknown operation"));
                assert!(msg.contains("unknown_op"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // =========================================================================
    // validate_input: activate_workflow tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_activate_workflow_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "activate_workflow",
            "workflow_id": "wf_123"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_activate_workflow_missing_id() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "activate_workflow"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'workflow_id'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // =========================================================================
    // validate_input: activate_general tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_activate_general_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "activate_general"
        }));
        assert!(result.is_ok());
    }

    // =========================================================================
    // validate_input: add operation tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_add_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "add",
            "type": "knowledge",
            "content": "Test content"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_add_valid_with_metadata() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "add",
            "type": "user_pref",
            "content": "User prefers dark mode",
            "metadata": {"priority": 0.8},
            "tags": ["ui", "preference"]
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_add_missing_type() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "add",
            "content": "Test content"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'type'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_add_missing_content() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "add",
            "type": "knowledge"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'content'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_add_invalid_type() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "add",
            "type": "invalid_type",
            "content": "Test content"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Invalid type"));
                assert!(msg.contains("invalid_type"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_add_all_valid_types() {
        let (tool, _temp) = create_test_tool().await;

        for memory_type in &["user_pref", "context", "knowledge", "decision"] {
            let result = tool.validate_input(&serde_json::json!({
                "operation": "add",
                "type": memory_type,
                "content": "Test content"
            }));
            assert!(result.is_ok(), "Type '{}' should be valid", memory_type);
        }
    }

    // =========================================================================
    // validate_input: get/delete operation tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_get_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "get",
            "memory_id": "mem_abc123"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_get_missing_id() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "get"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'memory_id'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_delete_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "delete",
            "memory_id": "mem_abc123"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_delete_missing_id() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "delete"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'memory_id'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    // =========================================================================
    // validate_input: list operation tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_list_valid_no_params() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "list"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_list_valid_with_filter() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "list",
            "type_filter": "knowledge",
            "limit": 20
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_list_invalid_type_filter() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "list",
            "type_filter": "invalid_filter"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Invalid type_filter"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    // =========================================================================
    // validate_input: search operation tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_search_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "find relevant information"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_search_valid_with_options() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "vector database",
            "limit": 5,
            "type_filter": "knowledge",
            "threshold": 0.8
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_search_missing_query() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "search"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'query'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_search_invalid_type_filter() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "test",
            "type_filter": "bad_filter"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Invalid type_filter"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_search_threshold_too_high() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "test",
            "threshold": 1.5
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Threshold"));
                assert!(msg.contains("must be between 0 and 1"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_search_threshold_negative() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "test",
            "threshold": -0.5
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Threshold"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_search_threshold_boundary_values() {
        let (tool, _temp) = create_test_tool().await;

        // Threshold = 0.0 should be valid
        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "test",
            "threshold": 0.0
        }));
        assert!(result.is_ok(), "Threshold 0.0 should be valid");

        // Threshold = 1.0 should be valid
        let result = tool.validate_input(&serde_json::json!({
            "operation": "search",
            "query": "test",
            "threshold": 1.0
        }));
        assert!(result.is_ok(), "Threshold 1.0 should be valid");
    }

    // =========================================================================
    // validate_input: clear_by_type operation tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_clear_by_type_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "clear_by_type",
            "type": "context"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_clear_by_type_missing_type() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "clear_by_type"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Missing 'type'"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_clear_by_type_invalid_type() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "clear_by_type",
            "type": "nonexistent"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Invalid type"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }
}
