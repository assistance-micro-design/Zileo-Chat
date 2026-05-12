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

//! Memory types for RAG and context persistence.
//!
//! These types are synchronized with TypeScript frontend types (src/types/memory.ts)
//! to ensure type safety for memory operations.

use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of memory content
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryType {
    /// User preferences and settings
    UserPref,
    /// Conversation context
    Context,
    /// Domain knowledge
    Knowledge,
    /// Past decisions and rationale
    Decision,
}

impl std::fmt::Display for MemoryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MemoryType::UserPref => write!(f, "user_pref"),
            MemoryType::Context => write!(f, "context"),
            MemoryType::Knowledge => write!(f, "knowledge"),
            MemoryType::Decision => write!(f, "decision"),
        }
    }
}

/// Memory entity for persistent context and RAG
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Unique identifier (deserialized from SurrealDB Thing type)
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    /// Type of memory content
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
    /// Text content of the memory
    pub content: String,
    /// Optional workflow ID for scoped memories (None = general)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Importance score (0.0-1.0, higher = more important)
    #[serde(default = "default_importance")]
    pub importance: f64,
    /// Optional expiration timestamp for TTL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Creation timestamp (set by database)
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

fn default_importance() -> f64 {
    0.5
}

/// Memory creation payload - only fields needed for creation
/// ID is passed separately to db.create() using table:id format
/// Datetime field is handled by database default
/// Enum fields are converted to strings for SurrealDB compatibility
///
/// Note: `expires_at` is intentionally absent here. SurrealDB SCHEMAFULL rejects
/// ISO 8601 strings for `option<datetime>` via JSON CONTENT — callers set it
/// separately with a `<datetime>` cast (see `set_expires_at_if_present`).
#[derive(Debug, Clone, Serialize)]
pub struct MemoryCreate {
    /// Type of memory content (as string for SurrealDB)
    #[serde(rename = "type")]
    pub memory_type: String,
    /// Text content of the memory
    pub content: String,
    /// Optional workflow ID for scoped memories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Importance score (0.0-1.0)
    pub importance: f64,
}

impl MemoryCreate {
    /// Unified builder accepting optional workflow_id and importance.
    pub fn build(
        memory_type: MemoryType,
        content: String,
        metadata: serde_json::Value,
        workflow_id: Option<String>,
        importance: f64,
    ) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            workflow_id,
            metadata,
            importance,
        }
    }
}

/// Search result returned by [`search_memories`](crate::commands::memory::search_memories).
///
/// One row per matching chunk: `chunk_id != parent_memory_id`. The agent uses
/// `parent_memory_id` with `operation=get` to read the full parent content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSearchResult {
    /// UUID of the chunk that matched (NOT the parent memory)
    pub chunk_id: String,
    /// UUID of the parent memory — use with `operation=get` to read full content
    pub parent_memory_id: String,
    /// 0-based index of this chunk within its parent
    pub chunk_index: usize,
    /// Total number of chunks for this parent memory
    pub chunk_count: usize,
    /// Chunk text (≤ DEFAULT_CHUNK_SIZE chars)
    pub content: String,
    /// Memory type of the parent (resolved via record link traversal)
    pub memory_type: MemoryType,
    /// Workflow scope of the parent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// Metadata of the parent (tags, priority, agent_source)
    pub metadata: serde_json::Value,
    /// Importance of the parent
    pub importance: f64,
    /// Expiration of the parent (TTL)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Creation timestamp of the parent
    pub created_at: DateTime<Utc>,
    /// Composite score: cosine * 0.7 + importance * 0.15 + recency * 0.15
    pub score: f64,
    /// Raw cosine similarity between query and chunk embedding (0..1)
    pub cosine_score: f64,
    /// "vector" or "text" — which path produced this row
    pub search_type: String,
}

/// Result of the describe operation - statistics about memories
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryDescribeResult {
    /// Total number of matching memories
    pub total: usize,
    /// Count by memory type
    pub by_type: std::collections::HashMap<String, usize>,
    /// All unique tags across matching memories
    pub tags: Vec<String>,
    /// Number of workflow-scoped memories
    pub workflow_count: usize,
    /// Number of general (cross-workflow) memories
    pub general_count: usize,
    /// Oldest memory timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oldest: Option<DateTime<Utc>>,
    /// Newest memory timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub newest: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_type_serialization() {
        let mtype = MemoryType::UserPref;
        let json = serde_json::to_string(&mtype).unwrap();
        assert_eq!(json, "\"user_pref\"");

        let mtype = MemoryType::Knowledge;
        let json = serde_json::to_string(&mtype).unwrap();
        assert_eq!(json, "\"knowledge\"");
    }

    #[test]
    fn test_memory_type_deserialization() {
        let mtype: MemoryType = serde_json::from_str("\"context\"").unwrap();
        assert_eq!(mtype, MemoryType::Context);

        let mtype: MemoryType = serde_json::from_str("\"decision\"").unwrap();
        assert_eq!(mtype, MemoryType::Decision);
    }

    #[test]
    fn test_memory_type_display() {
        assert_eq!(MemoryType::UserPref.to_string(), "user_pref");
        assert_eq!(MemoryType::Context.to_string(), "context");
        assert_eq!(MemoryType::Knowledge.to_string(), "knowledge");
        assert_eq!(MemoryType::Decision.to_string(), "decision");
    }

    #[test]
    fn test_memory_serialization() {
        let memory = Memory {
            id: "mem_001".to_string(),
            memory_type: MemoryType::Context,
            content: "User prefers dark mode".to_string(),
            workflow_id: None,
            metadata: serde_json::json!({"source": "settings"}),
            importance: 0.5,
            expires_at: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"type\":\"context\""));
        assert!(json.contains("\"content\":\"User prefers dark mode\""));
        assert!(json.contains("\"source\":\"settings\""));
        assert!(json.contains("\"importance\":0.5"));
        // workflow_id should be omitted when None
        assert!(!json.contains("workflow_id"));
        // expires_at should be omitted when None
        assert!(!json.contains("expires_at"));

        let deserialized: Memory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, memory.id);
        assert_eq!(deserialized.memory_type, memory.memory_type);
        assert_eq!(deserialized.content, memory.content);
        assert!((deserialized.importance - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_memory_with_workflow() {
        let memory = Memory {
            id: "mem_001a".to_string(),
            memory_type: MemoryType::Context,
            content: "Workflow specific memory".to_string(),
            workflow_id: Some("wf_123".to_string()),
            metadata: serde_json::json!({}),
            importance: 0.3,
            expires_at: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"workflow_id\":\"wf_123\""));
    }

    #[test]
    fn test_chunk_search_result_uses_camel_case_for_ipc() {
        // The IPC contract requires camelCase field names — pin them here
        // so a future drop of `#[serde(rename_all)]` is loud.
        let result = ChunkSearchResult {
            chunk_id: "c1".to_string(),
            parent_memory_id: "m1".to_string(),
            chunk_index: 0,
            chunk_count: 2,
            content: "hello".to_string(),
            memory_type: MemoryType::Knowledge,
            workflow_id: None,
            metadata: serde_json::json!({}),
            importance: 0.5,
            expires_at: None,
            created_at: Utc::now(),
            score: 0.42,
            cosine_score: 0.55,
            search_type: "vector".to_string(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"chunkId\":\"c1\""));
        assert!(json.contains("\"parentMemoryId\":\"m1\""));
        assert!(json.contains("\"searchType\":\"vector\""));
        assert!(json.contains("\"cosineScore\":0.55"));
    }
}
