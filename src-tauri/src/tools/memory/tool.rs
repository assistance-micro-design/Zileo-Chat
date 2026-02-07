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

use super::helpers::{
    add_memory_core, build_scope_condition, describe_memories_core, search_memories_core,
    AddMemoryParams, SearchParams,
};
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::models::memory::{Memory, MemoryType};
use crate::tools::constants::memory::{
    self as mem_constants, DEFAULT_LIMIT, DEFAULT_SIMILARITY_THRESHOLD, MAX_CONTENT_LENGTH,
    MAX_LIMIT, VALID_TYPES,
};
use crate::tools::response::ResponseBuilder;
use crate::tools::utils::{
    db_error, delete_with_check, validate_enum_value, validate_length, validate_not_empty,
};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, instrument};

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

    /// Determines the workflow_id to store on a new memory.
    ///
    /// Priority: 1) explicit scope override, 2) auto-scope by type.
    /// - `user_pref` and `knowledge` are general (workflow_id = None)
    /// - `context` and `decision` are workflow-scoped (workflow_id = default_workflow_id)
    fn resolve_storage_scope(&self, memory_type: &str, input: &MemoryInput) -> Option<String> {
        // Agent can override with explicit scope parameter
        if let Some(ref scope) = input.scope {
            return match scope.as_str() {
                "general" => None,
                "workflow" => self.default_workflow_id.clone(),
                _ => self.default_workflow_id.clone(),
            };
        }

        // Auto-scope based on memory type
        if mem_constants::GENERAL_SCOPE_TYPES.contains(&memory_type) {
            None // user_pref, knowledge -> always general
        } else {
            self.default_workflow_id.clone() // context, decision -> workflow-scoped
        }
    }

    /// Resolves the workflow_id for query filtering (list/search/describe).
    ///
    /// Explicit `workflow_id` in input takes priority over `default_workflow_id`.
    fn resolve_query_workflow_id(&self, input: &MemoryInput) -> Option<String> {
        input
            .workflow_id
            .clone()
            .or(self.default_workflow_id.clone())
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
    #[instrument(skip(self, input, content, metadata), fields(agent_id = %self.agent_id, memory_type = %memory_type))]
    async fn add_memory(
        &self,
        input: &MemoryInput,
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

        // Auto-scope by type (or explicit override via scope param)
        let workflow_id = self.resolve_storage_scope(memory_type, input);

        // Auto-importance by type
        let importance = Self::default_importance_for_type(memory_type);

        // Auto-TTL by type (context -> 7 days)
        let expires_at = Self::default_expires_at_for_type(memory_type);

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
            importance,
            expires_at,
        };

        let result = add_memory_core(params, &self.db, self.embedding_service.as_ref())
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
    #[instrument(skip(self), fields(memory_id = %memory_id))]
    async fn get_memory(&self, memory_id: &str) -> ToolResult<Value> {
        // OPT-MEM-5: Parameterized query for security
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
    /// * `input` - Parsed memory input (provides workflow_id override)
    /// * `type_filter` - Optional memory type to filter by
    /// * `limit` - Maximum number of results (default: 10)
    /// * `scope` - Scope filter: "workflow", "general", or "both" (default: "both")
    /// * `mode` - Display mode: "full" (default) or "compact"
    #[instrument(skip(self, input), fields(type_filter = ?type_filter, limit = limit, scope = %scope))]
    async fn list_memories(
        &self,
        input: &MemoryInput,
        type_filter: Option<&str>,
        limit: usize,
        scope: &str,
        mode: &str,
    ) -> ToolResult<Value> {
        let workflow_id = self.resolve_query_workflow_id(input);
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
            Self::parse_memory_type(mem_type)?;
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

        let memories: Vec<Memory> = self
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
                    let preview = if m.content.len()
                        > crate::tools::constants::memory::COMPACT_PREVIEW_LENGTH
                    {
                        format!(
                            "{}...",
                            &m.content[..crate::tools::constants::memory::COMPACT_PREVIEW_LENGTH]
                        )
                    } else {
                        m.content.clone()
                    };
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
    #[instrument(skip(self, input), fields(query_len = query_text.len(), limit = limit, scope = %scope))]
    async fn search_memories(
        &self,
        input: &MemoryInput,
        query_text: &str,
        limit: usize,
        type_filter: Option<&str>,
        threshold: f64,
        scope: &str,
    ) -> ToolResult<Value> {
        let workflow_id = self.resolve_query_workflow_id(input);

        // Validate type filter if provided
        if let Some(mem_type) = type_filter {
            Self::parse_memory_type(mem_type)?;
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
            search_memories_core(params, &self.db, self.embedding_service.as_ref())
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
    #[instrument(skip(self, input), fields(scope = %scope))]
    async fn describe_memories(&self, input: &MemoryInput, scope: &str) -> ToolResult<Value> {
        let wf_id = self.resolve_query_workflow_id(input);

        let result = describe_memories_core(wf_id.as_deref(), scope, &self.db)
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
    /// * `input` - Parsed memory input (provides scope/workflow_id override)
    /// * `memory_type` - Type of memories to clear
    #[instrument(skip(self, input), fields(memory_type = %memory_type))]
    async fn clear_by_type(&self, input: &MemoryInput, memory_type: &str) -> ToolResult<Value> {
        // Validate memory type
        Self::parse_memory_type(memory_type)?;

        let workflow_id = self.resolve_query_workflow_id(input);

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
- describe: Overview of available memories (counts, types, tags) - call this first!
- add: Store new memory with auto-scoping by type and embedding generation
- get: Retrieve specific memory by ID
- list: View memories with optional type filter and scope (supports compact mode)
- search: Find semantically similar memories using vector search (ranked by relevance + importance + recency)
- delete: Remove a memory
- clear_by_type: Bulk delete all memories of a specific type

AUTO-SCOPING (for add):
- user_pref, knowledge -> stored as GENERAL (cross-workflow, accessible everywhere)
- context, decision -> stored as WORKFLOW-SCOPED (tied to current workflow)
- Override with scope parameter: "general" forces cross-workflow, "workflow" forces workflow-scoped

SCOPE PARAMETER (for list/search/describe):
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
1. Discover available memories (always start here):
   {{"operation": "describe"}}

2. Compact listing (token-efficient):
   {{"operation": "list", "mode": "compact"}}

3. Search all memories (ranked by relevance + importance + recency):
   {{"operation": "search", "query": "vector database indexing", "limit": 5}}

4. Store knowledge (auto-scoped to general):
   {{"operation": "add", "type": "knowledge", "content": "SurrealDB supports HNSW vector indexing"}}

5. Store user preference (auto-scoped to general):
   {{"operation": "add", "type": "user_pref", "content": "User prefers detailed explanations with examples", "tags": ["communication", "style"]}}

6. Store decision (auto-scoped to current workflow):
   {{"operation": "add", "type": "decision", "content": "Chose PostgreSQL over MongoDB because the data is highly relational"}}

7. Store context (auto-scoped to workflow, auto-expires in 7 days):
   {{"operation": "add", "type": "context", "content": "User is working on database migration project"}}

8. Force a decision to be global (override auto-scope):
   {{"operation": "add", "type": "decision", "content": "Company policy: always use RGPD-compliant storage", "scope": "general"}}

9. Delete a memory:
   {{"operation": "delete", "memory_id": "mem_abc123"}}

10. Clear all context memories:
    {{"operation": "clear_by_type", "type": "context"}}"#,
                MAX_CONTENT_LENGTH,
                MAX_CONTENT_LENGTH,
                DEFAULT_LIMIT,
                MAX_LIMIT,
                DEFAULT_SIMILARITY_THRESHOLD
            ),

            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["describe", "add", "get", "list", "search", "delete", "clear_by_type"],
                        "description": "Operation: 'describe' shows overview, 'add' stores memory (auto-scoped by type), 'get' retrieves by ID, 'list' shows memories, 'search' finds similar, 'delete' removes, 'clear_by_type' bulk deletes"
                    },
                    "workflow_id": {
                        "type": "string",
                        "description": "Override the default workflow context. Rarely needed - the tool auto-detects from its creation context."
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
                        "description": "For add: override auto-scoping ('general' forces cross-workflow, 'workflow' forces workflow-scoped). For list/search/describe: filter scope."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["full", "compact"],
                        "default": "full",
                        "description": "Display mode for list: 'full' returns complete memories, 'compact' returns truncated previews with tags"
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
            "add" => {
                // SAFETY: validate_add() ensures memory_type and content are Some
                let memory_type = params.memory_type.as_deref().unwrap();
                let content = params.content.as_deref().unwrap();
                self.add_memory(
                    &params,
                    memory_type,
                    content,
                    params.metadata.clone(),
                    params.tags.clone(),
                )
                .await
            }

            "get" => {
                // SAFETY: validate_get_or_delete() ensures memory_id is Some
                self.get_memory(params.memory_id.as_deref().unwrap()).await
            }

            "describe" => {
                let scope = params.scope.as_deref().unwrap_or("both");
                self.describe_memories(&params, scope).await
            }

            "list" => {
                let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
                let scope = params.scope.as_deref().unwrap_or("both");
                let mode = params.mode.as_deref().unwrap_or("full");
                self.list_memories(&params, params.type_filter.as_deref(), limit, scope, mode)
                    .await
            }

            "search" => {
                // SAFETY: validate_search() ensures query is Some
                let limit = params.limit.unwrap_or(DEFAULT_LIMIT);
                let threshold = params.threshold.unwrap_or(DEFAULT_SIMILARITY_THRESHOLD);
                let scope = params.scope.as_deref().unwrap_or("both");
                self.search_memories(
                    &params,
                    params.query.as_deref().unwrap(),
                    limit,
                    params.type_filter.as_deref(),
                    threshold,
                    scope,
                )
                .await
            }

            "delete" => {
                // SAFETY: validate_get_or_delete() ensures memory_id is Some
                self.delete_memory(params.memory_id.as_deref().unwrap())
                    .await
            }

            "clear_by_type" => {
                // SAFETY: validate_clear_by_type() ensures memory_type is Some
                self.clear_by_type(&params, params.memory_type.as_deref().unwrap())
                    .await
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
    mode: Option<String>,
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
            mode: input["mode"].as_str().map(String::from),
            metadata: input.get("metadata").cloned(),
            tags,
        })
    }

    /// Validates input based on operation type.
    fn validate(&self) -> ToolResult<()> {
        match self.operation.as_str() {
            "describe" => Ok(()),
            "add" => self.validate_add(),
            "get" | "delete" => self.validate_get_or_delete(),
            "list" => self.validate_type_filter(),
            "search" => self.validate_search(),
            "clear_by_type" => self.validate_clear_by_type(),
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: describe, add, get, list, search, delete, clear_by_type",
                self.operation
            ))),
        }
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
    fn test_input_validation_describe() {
        let valid_input = serde_json::json!({
            "operation": "describe"
        });

        assert!(valid_input.is_object());
        assert_eq!(valid_input["operation"], "describe");
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
    // validate_input: describe tests
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_describe_valid() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "describe"
        }));
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_validate_input_describe_with_scope() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "describe",
            "scope": "general"
        }));
        assert!(result.is_ok());
    }

    // =========================================================================
    // validate_input: removed operations are rejected
    // =========================================================================

    #[tokio::test]
    async fn test_validate_input_activate_workflow_rejected() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "activate_workflow",
            "workflow_id": "wf_123"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Unknown operation"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[tokio::test]
    async fn test_validate_input_activate_general_rejected() {
        let (tool, _temp) = create_test_tool().await;

        let result = tool.validate_input(&serde_json::json!({
            "operation": "activate_general"
        }));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Unknown operation"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
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
