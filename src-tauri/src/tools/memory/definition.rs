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

use crate::tools::constants::memory::{
    DEFAULT_LIMIT, DEFAULT_SIMILARITY_THRESHOLD, MAX_CONTENT_LENGTH, MAX_LIMIT,
};
use crate::tools::ToolDefinition;

/// Builds the ToolDefinition for MemoryTool.
///
/// This is extracted from the `Tool::definition()` trait method to keep
/// the main tool.rs file focused on struct + dispatch logic.
pub fn build_definition() -> ToolDefinition {
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
