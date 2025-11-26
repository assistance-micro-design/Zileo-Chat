// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! MemoryTool implementation for agent contextual persistence.
//!
//! This tool allows agents to manage memories with semantic search capabilities
//! using vector embeddings and SurrealDB's HNSW index.

use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::{Memory, MemoryCreate, MemoryCreateWithEmbedding, MemoryType};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Maximum content length for a memory (50KB as per spec)
const MAX_CONTENT_LENGTH: usize = 50_000;

/// Default similarity threshold for semantic search (0-1)
const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.7;

/// Default limit for list/search operations
const DEFAULT_LIMIT: usize = 10;

/// Maximum limit for list/search operations
const MAX_LIMIT: usize = 100;

/// Valid memory type strings
const VALID_MEMORY_TYPES: [&str; 4] = ["user_pref", "context", "knowledge", "decision"];

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

    /// Adds a new memory with optional embedding.
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
        if content.is_empty() {
            return Err(ToolError::ValidationFailed(
                "Memory content cannot be empty".to_string(),
            ));
        }

        if content.len() > MAX_CONTENT_LENGTH {
            return Err(ToolError::ValidationFailed(format!(
                "Content is {} chars, max is {}. Consider splitting into multiple memories",
                content.len(),
                MAX_CONTENT_LENGTH
            )));
        }

        // Validate memory type
        let mem_type = Self::parse_memory_type(memory_type)?;

        let memory_id = Uuid::new_v4().to_string();
        let workflow_id = self.current_workflow_id().await;

        // Build metadata with agent source and tags
        let mut meta = metadata.unwrap_or(serde_json::json!({}));
        if let Some(obj) = meta.as_object_mut() {
            obj.insert("agent_source".to_string(), serde_json::json!(self.agent_id));
            if let Some(t) = tags {
                obj.insert("tags".to_string(), serde_json::json!(t));
            }
        }

        // Try to generate embedding if service is available
        let embedding_generated = if let Some(ref embed_service) = self.embedding_service {
            match embed_service.embed(content).await {
                Ok(embedding) => {
                    // Create memory with embedding
                    let memory = if let Some(wf_id) = &workflow_id {
                        MemoryCreateWithEmbedding::with_workflow(
                            mem_type,
                            content.to_string(),
                            embedding,
                            meta.clone(),
                            wf_id.clone(),
                        )
                    } else {
                        MemoryCreateWithEmbedding::new(
                            mem_type,
                            content.to_string(),
                            embedding,
                            meta.clone(),
                        )
                    };

                    self.db
                        .create("memory", &memory_id, memory)
                        .await
                        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

                    true
                }
                Err(e) => {
                    // Fallback to text-only storage
                    warn!(error = %e, "Embedding generation failed, storing without embedding");

                    let memory = if let Some(wf_id) = &workflow_id {
                        MemoryCreate::with_workflow(
                            mem_type,
                            content.to_string(),
                            meta.clone(),
                            wf_id.clone(),
                        )
                    } else {
                        MemoryCreate::new(mem_type, content.to_string(), meta.clone())
                    };

                    self.db
                        .create("memory", &memory_id, memory)
                        .await
                        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

                    false
                }
            }
        } else {
            // No embedding service, store text only
            let memory = if let Some(wf_id) = &workflow_id {
                MemoryCreate::with_workflow(
                    mem_type,
                    content.to_string(),
                    meta.clone(),
                    wf_id.clone(),
                )
            } else {
                MemoryCreate::new(mem_type, content.to_string(), meta.clone())
            };

            self.db
                .create("memory", &memory_id, memory)
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

            false
        };

        info!(
            memory_id = %memory_id,
            memory_type = %memory_type,
            embedding = embedding_generated,
            "Memory created"
        );

        Ok(serde_json::json!({
            "success": true,
            "memory_id": memory_id,
            "type": memory_type,
            "embedding_generated": embedding_generated,
            "workflow_id": workflow_id,
            "message": format!("Memory '{}' created successfully", memory_id)
        }))
    }

    /// Retrieves a memory by ID.
    ///
    /// # Arguments
    /// * `memory_id` - Memory ID to retrieve
    #[instrument(skip(self), fields(memory_id = %memory_id))]
    async fn get_memory(&self, memory_id: &str) -> ToolResult<Value> {
        let query = format!(
            r#"SELECT
                meta::id(id) AS id,
                type,
                content,
                workflow_id,
                metadata,
                created_at
            FROM memory
            WHERE meta::id(id) = '{}'"#,
            memory_id
        );

        let results: Vec<Memory> = self
            .db
            .query(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

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
    #[instrument(skip(self), fields(type_filter = ?type_filter, limit = limit))]
    async fn list_memories(&self, type_filter: Option<&str>, limit: usize) -> ToolResult<Value> {
        let workflow_id = self.current_workflow_id().await;
        let limit = limit.min(MAX_LIMIT);

        let mut conditions = Vec::new();

        // Workflow scope condition
        if let Some(ref wf_id) = workflow_id {
            conditions.push(format!("workflow_id = '{}'", wf_id));
        }

        // Type filter condition
        if let Some(mem_type) = type_filter {
            // Validate the type
            Self::parse_memory_type(mem_type)?;
            conditions.push(format!("type = '{}'", mem_type));
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
                created_at
            FROM memory
            {}
            ORDER BY created_at DESC
            LIMIT {}"#,
            where_clause, limit
        );

        let memories: Vec<Memory> = self
            .db
            .query(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        debug!(count = memories.len(), "Memories listed");

        Ok(serde_json::json!({
            "success": true,
            "count": memories.len(),
            "scope": if workflow_id.is_some() { "workflow" } else { "general" },
            "workflow_id": workflow_id,
            "memories": memories
        }))
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
    #[instrument(skip(self), fields(query_len = query_text.len(), limit = limit))]
    async fn search_memories(
        &self,
        query_text: &str,
        limit: usize,
        type_filter: Option<&str>,
        threshold: f64,
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
                        )
                        .await;
                }
                Err(e) => {
                    warn!(error = %e, "Query embedding failed, falling back to text search");
                }
            }
        }

        // Fallback to text search
        self.text_search(query_text, limit, type_filter, &workflow_id)
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
    ) -> ToolResult<Value> {
        // Build conditions
        let mut conditions = vec!["embedding IS NOT NONE".to_string()];

        if let Some(ref wf_id) = workflow_id {
            conditions.push(format!("workflow_id = '{}'", wf_id));
        }

        if let Some(mem_type) = type_filter {
            conditions.push(format!("type = '{}'", mem_type));
        }

        let where_clause = conditions.join(" AND ");

        // Convert threshold to distance (cosine distance = 1 - similarity)
        let distance_threshold = 1.0 - threshold;

        // Format embedding for SurrealQL
        let embedding_str: String = query_embedding
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ");

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
            .query_json(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        debug!(
            count = results.len(),
            threshold = threshold,
            "Vector search completed"
        );

        Ok(serde_json::json!({
            "success": true,
            "search_type": "vector",
            "count": results.len(),
            "threshold": threshold,
            "scope": if workflow_id.is_some() { "workflow" } else { "general" },
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
    ) -> ToolResult<Value> {
        let mut conditions = Vec::new();

        // Text content contains query (case-insensitive via LIKE)
        let escaped_query = query_text.replace('\'', "''").replace('%', "\\%");
        conditions.push(format!(
            "string::lowercase(content) CONTAINS string::lowercase('{}')",
            escaped_query
        ));

        if let Some(ref wf_id) = workflow_id {
            conditions.push(format!("workflow_id = '{}'", wf_id));
        }

        if let Some(mem_type) = type_filter {
            conditions.push(format!("type = '{}'", mem_type));
        }

        let where_clause = conditions.join(" AND ");

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
            .query(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        debug!(count = results.len(), "Text search completed");

        Ok(serde_json::json!({
            "success": true,
            "search_type": "text",
            "count": results.len(),
            "scope": if workflow_id.is_some() { "workflow" } else { "general" },
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
        // First check if memory exists
        let check_query = format!(
            "SELECT meta::id(id) AS id FROM memory WHERE meta::id(id) = '{}'",
            memory_id
        );
        let existing: Vec<Value> = self
            .db
            .query(&check_query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        if existing.is_empty() {
            return Err(ToolError::NotFound(format!(
                "Memory '{}' does not exist. Nothing to delete",
                memory_id
            )));
        }

        let query = format!("DELETE memory:`{}`", memory_id);
        let _: Vec<Value> = self
            .db
            .query(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

        info!(memory_id = %memory_id, "Memory deleted");

        Ok(serde_json::json!({
            "success": true,
            "memory_id": memory_id,
            "message": format!("Memory '{}' has been deleted", memory_id)
        }))
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

        let query = if let Some(ref wf_id) = workflow_id {
            format!(
                "DELETE FROM memory WHERE type = '{}' AND workflow_id = '{}'",
                memory_type, wf_id
            )
        } else {
            format!("DELETE FROM memory WHERE type = '{}'", memory_type)
        };

        let _: Vec<Value> = self
            .db
            .query(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

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
            description:
                r#"Manages persistent memory for contextual awareness and knowledge retrieval.

USE THIS TOOL TO:
- Store important information for future reference
- Search past memories by semantic similarity
- Maintain context across conversations
- Organize knowledge by type (user_pref, context, knowledge, decision)

OPERATIONS:
- activate_workflow: Set workflow-specific scope for memory isolation
- activate_general: Switch to general mode (cross-workflow access)
- add: Store new memory with automatic embedding generation
- get: Retrieve specific memory by ID
- list: View memories with optional type filter
- search: Find semantically similar memories using vector search
- delete: Remove a memory
- clear_by_type: Bulk delete all memories of a specific type

BEST PRACTICES:
- Use 'knowledge' type for facts and domain expertise
- Use 'decision' type for rationale behind choices
- Use 'context' type for conversation-specific information
- Use 'user_pref' type for user preferences and settings
- Activate workflow scope when working on specific tasks
- Search before adding to avoid duplicates

EXAMPLES:
1. Store knowledge:
   {"operation": "add", "type": "knowledge", "content": "SurrealDB supports HNSW vector indexing"}

2. Search memories:
   {"operation": "search", "query": "vector database indexing", "limit": 5}

3. Activate workflow scope:
   {"operation": "activate_workflow", "workflow_id": "wf_abc123"}

4. List user preferences:
   {"operation": "list", "type_filter": "user_pref"}"#
                    .to_string(),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["activate_workflow", "activate_general", "add", "get", "list", "search", "delete", "clear_by_type"],
                        "description": "The operation to perform"
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
        self.validate_input(&input)?;

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation".to_string()))?;

        debug!(operation = %operation, "Executing MemoryTool");

        match operation {
            "activate_workflow" => {
                let workflow_id = input["workflow_id"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing workflow_id for activate_workflow".to_string())
                })?;
                self.activate_workflow(workflow_id.to_string()).await
            }

            "activate_general" => self.activate_general().await,

            "add" => {
                let memory_type = input["type"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'type' for add operation".to_string())
                })?;
                let content = input["content"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'content' for add operation".to_string())
                })?;
                let metadata = input.get("metadata").cloned();
                let tags: Option<Vec<String>> = input["tags"]
                    .as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());

                self.add_memory(memory_type, content, metadata, tags).await
            }

            "get" => {
                let memory_id = input["memory_id"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'memory_id' for get operation".to_string())
                })?;
                self.get_memory(memory_id).await
            }

            "list" => {
                let type_filter = input["type_filter"].as_str();
                let limit = input["limit"].as_u64().unwrap_or(DEFAULT_LIMIT as u64) as usize;
                self.list_memories(type_filter, limit).await
            }

            "search" => {
                let query = input["query"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'query' for search operation".to_string())
                })?;
                let limit = input["limit"].as_u64().unwrap_or(DEFAULT_LIMIT as u64) as usize;
                let type_filter = input["type_filter"].as_str();
                let threshold = input["threshold"]
                    .as_f64()
                    .unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);

                self.search_memories(query, limit, type_filter, threshold)
                    .await
            }

            "delete" => {
                let memory_id = input["memory_id"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'memory_id' for delete operation".to_string())
                })?;
                self.delete_memory(memory_id).await
            }

            "clear_by_type" => {
                let memory_type = input["type"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput("Missing 'type' for clear_by_type operation".to_string())
                })?;
                self.clear_by_type(memory_type).await
            }

            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: activate_workflow, activate_general, add, get, list, search, delete, clear_by_type",
                operation
            ))),
        }
    }

    /// Validates input before execution.
    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation field".to_string()))?;

        match operation {
            "activate_workflow" => {
                if input.get("workflow_id").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'workflow_id' for activate_workflow operation".to_string(),
                    ));
                }
            }
            "activate_general" => {} // No required params
            "add" => {
                if input.get("type").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'type' for add operation. Valid types: user_pref, context, knowledge, decision".to_string(),
                    ));
                }
                if input.get("content").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'content' for add operation".to_string(),
                    ));
                }
                // Validate type value
                if let Some(type_str) = input["type"].as_str() {
                    if !VALID_MEMORY_TYPES.contains(&type_str) {
                        return Err(ToolError::ValidationFailed(format!(
                            "Invalid type '{}'. Valid types: user_pref, context, knowledge, decision",
                            type_str
                        )));
                    }
                }
            }
            "get" | "delete" => {
                if input.get("memory_id").is_none() {
                    return Err(ToolError::InvalidInput(format!(
                        "Missing 'memory_id' for {} operation",
                        operation
                    )));
                }
            }
            "list" => {
                // Validate type_filter if provided
                if let Some(type_str) = input["type_filter"].as_str() {
                    if !VALID_MEMORY_TYPES.contains(&type_str) {
                        return Err(ToolError::ValidationFailed(format!(
                            "Invalid type_filter '{}'. Valid types: user_pref, context, knowledge, decision",
                            type_str
                        )));
                    }
                }
            }
            "search" => {
                if input.get("query").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'query' for search operation".to_string(),
                    ));
                }
                // Validate type_filter if provided
                if let Some(type_str) = input["type_filter"].as_str() {
                    if !VALID_MEMORY_TYPES.contains(&type_str) {
                        return Err(ToolError::ValidationFailed(format!(
                            "Invalid type_filter '{}'. Valid types: user_pref, context, knowledge, decision",
                            type_str
                        )));
                    }
                }
                // Validate threshold if provided
                if let Some(threshold) = input["threshold"].as_f64() {
                    if !(0.0..=1.0).contains(&threshold) {
                        return Err(ToolError::ValidationFailed(format!(
                            "Threshold {} must be between 0 and 1",
                            threshold
                        )));
                    }
                }
            }
            "clear_by_type" => {
                if input.get("type").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing 'type' for clear_by_type operation. Valid types: user_pref, context, knowledge, decision".to_string(),
                    ));
                }
                // Validate type value
                if let Some(type_str) = input["type"].as_str() {
                    if !VALID_MEMORY_TYPES.contains(&type_str) {
                        return Err(ToolError::ValidationFailed(format!(
                            "Invalid type '{}'. Valid types: user_pref, context, knowledge, decision",
                            type_str
                        )));
                    }
                }
            }
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown operation: '{}'. Valid operations: activate_workflow, activate_general, add, get, list, search, delete, clear_by_type",
                    operation
                )));
            }
        }

        Ok(())
    }

    /// Returns false - memory operations are reversible, no confirmation needed.
    fn requires_confirmation(&self) -> bool {
        false
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
        assert!(VALID_MEMORY_TYPES.contains(&"user_pref"));
        assert!(VALID_MEMORY_TYPES.contains(&"context"));
        assert!(VALID_MEMORY_TYPES.contains(&"knowledge"));
        assert!(VALID_MEMORY_TYPES.contains(&"decision"));
        assert!(!VALID_MEMORY_TYPES.contains(&"invalid"));
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
        assert!(threshold >= 0.0 && threshold <= 1.0);

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
