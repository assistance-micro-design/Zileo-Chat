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

//! Tool definition (schema + LLM description) for the MemoryTool.

use crate::tools::ToolDefinition;

/// Builds the ToolDefinition for MemoryTool.
///
/// This is extracted from the `Tool::definition()` trait method to keep
/// the main tool.rs file focused on struct + dispatch logic.
pub fn build_definition() -> ToolDefinition {
    ToolDefinition {
        id: "MemoryTool".to_string(),
        name: "Memory Manager".to_string(),
        summary: "Store, search, and retrieve persistent memories by semantic similarity"
            .to_string(),
        description: r#"Manages persistent memory for contextual awareness and knowledge retrieval.

USE THIS TOOL WHEN:
- You need to store important information for future reference
- You want to search past memories by semantic similarity
- You need to maintain context across conversations
- You want to organize knowledge by type (user_pref, context, knowledge, decision)

DO NOT USE THIS TOOL WHEN:
- Information is only relevant to the current message (use conversation context)
- Storing duplicate content already in memory (search first)
- For temporary calculations or intermediate values

OPERATIONS:
- describe: Overview of available memories (counts, types, tags)
- add: Store new memory (type required: user_pref, context, knowledge, decision)
- get: Retrieve specific memory by ID
- list: View memories with optional type filter and scope
- search: Find semantically similar memories via vector search
- delete: Remove a memory
- clear_by_type: Bulk delete all memories of a specific type

EXAMPLES:
1. Describe: {"operation": "describe"}
2. Search: {"operation": "search", "query": "vector database indexing", "limit": 5}
3. Add: {"operation": "add", "type": "knowledge", "content": "SurrealDB supports HNSW vector indexing"}"#
            .to_string(),

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
