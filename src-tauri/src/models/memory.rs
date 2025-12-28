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
    /// Creation timestamp (set by database)
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

/// Memory creation payload - only fields needed for creation
/// ID is passed separately to db.create() using table:id format
/// Datetime field is handled by database default
/// Enum fields are converted to strings for SurrealDB compatibility
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
}

impl MemoryCreate {
    /// Creates a new MemoryCreate with the given parameters (general scope)
    #[allow(dead_code)] // Convenience method for simple cases
    pub fn new(memory_type: MemoryType, content: String, metadata: serde_json::Value) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            workflow_id: None,
            metadata,
        }
    }

    /// Creates a new MemoryCreate with workflow scope
    #[allow(dead_code)] // Used by MemoryTool in Phase 3
    pub fn with_workflow(
        memory_type: MemoryType,
        content: String,
        metadata: serde_json::Value,
        workflow_id: String,
    ) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            workflow_id: Some(workflow_id),
            metadata,
        }
    }

    /// OPT-MEM-10: Unified builder accepting optional workflow_id
    /// Eliminates the match branches in callers
    pub fn build(
        memory_type: MemoryType,
        content: String,
        metadata: serde_json::Value,
        workflow_id: Option<String>,
    ) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            workflow_id,
            metadata,
        }
    }
}

/// Memory creation payload with embedding vector
/// Used by MemoryTool for creating memories with vector embeddings
#[allow(dead_code)] // Used by MemoryTool in Phase 3
#[derive(Debug, Clone, Serialize)]
pub struct MemoryCreateWithEmbedding {
    /// Type of memory content (as string for SurrealDB)
    #[serde(rename = "type")]
    pub memory_type: String,
    /// Text content of the memory
    pub content: String,
    /// Vector embedding for semantic search
    pub embedding: Vec<f32>,
    /// Optional workflow ID for scoped memories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

#[allow(dead_code)] // Used by MemoryTool in Phase 3
impl MemoryCreateWithEmbedding {
    /// Creates a new MemoryCreateWithEmbedding with the given parameters
    pub fn new(
        memory_type: MemoryType,
        content: String,
        embedding: Vec<f32>,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            embedding,
            workflow_id: None,
            metadata,
        }
    }

    /// Creates a new MemoryCreateWithEmbedding with workflow scope
    pub fn with_workflow(
        memory_type: MemoryType,
        content: String,
        embedding: Vec<f32>,
        metadata: serde_json::Value,
        workflow_id: String,
    ) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            embedding,
            workflow_id: Some(workflow_id),
            metadata,
        }
    }

    /// OPT-MEM-10: Unified builder accepting optional workflow_id
    /// Eliminates the match branches in callers
    pub fn build(
        memory_type: MemoryType,
        content: String,
        embedding: Vec<f32>,
        metadata: serde_json::Value,
        workflow_id: Option<String>,
    ) -> Self {
        Self {
            memory_type: memory_type.to_string(),
            content,
            embedding,
            workflow_id,
            metadata,
        }
    }
}

/// Memory entity with embedding vector (for DB storage)
#[allow(dead_code)] // API type for semantic search/RAG operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryWithEmbedding {
    /// Unique identifier (deserialized from SurrealDB Thing type)
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    /// Type of memory content
    #[serde(rename = "type")]
    pub memory_type: MemoryType,
    /// Text content of the memory
    pub content: String,
    /// Vector embedding (1024D for Mistral/Ollama)
    pub embedding: Vec<f32>,
    /// Optional workflow ID for scoped memories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow_id: Option<String>,
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Creation timestamp
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

/// Memory search result with relevance score
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchResult {
    /// Memory entity
    pub memory: Memory,
    /// Relevance score (0-1, higher is more relevant)
    pub score: f64,
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
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"type\":\"context\""));
        assert!(json.contains("\"content\":\"User prefers dark mode\""));
        assert!(json.contains("\"source\":\"settings\""));
        // workflow_id should be omitted when None
        assert!(!json.contains("workflow_id"));

        let deserialized: Memory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, memory.id);
        assert_eq!(deserialized.memory_type, memory.memory_type);
        assert_eq!(deserialized.content, memory.content);
    }

    #[test]
    fn test_memory_with_workflow() {
        let memory = Memory {
            id: "mem_001a".to_string(),
            memory_type: MemoryType::Context,
            content: "Workflow specific memory".to_string(),
            workflow_id: Some("wf_123".to_string()),
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"workflow_id\":\"wf_123\""));
    }

    #[test]
    fn test_memory_with_embedding() {
        let memory = MemoryWithEmbedding {
            id: "mem_002".to_string(),
            memory_type: MemoryType::Knowledge,
            content: "Rust is a systems programming language".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            workflow_id: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"embedding\":[0.1,0.2,0.3]"));
    }

    #[test]
    fn test_memory_create_with_embedding() {
        let memory = MemoryCreateWithEmbedding::new(
            MemoryType::Knowledge,
            "Test content".to_string(),
            vec![0.1, 0.2, 0.3],
            serde_json::json!({"tags": ["test"]}),
        );

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"type\":\"knowledge\""));
        assert!(json.contains("\"embedding\":[0.1,0.2,0.3]"));
        assert!(!json.contains("workflow_id"));
    }

    #[test]
    fn test_memory_create_with_workflow() {
        let memory = MemoryCreateWithEmbedding::with_workflow(
            MemoryType::Context,
            "Workflow memory".to_string(),
            vec![0.5, 0.6],
            serde_json::json!({}),
            "wf_abc".to_string(),
        );

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"workflow_id\":\"wf_abc\""));
    }

    #[test]
    fn test_memory_search_result() {
        let memory = Memory {
            id: "mem_003".to_string(),
            memory_type: MemoryType::Decision,
            content: "Chose SurrealDB for embedded database".to_string(),
            workflow_id: None,
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let result = MemorySearchResult {
            memory,
            score: 0.95,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"score\":0.95"));
        assert!(json.contains("\"type\":\"decision\""));
    }
}
