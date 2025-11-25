// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Creation timestamp (set by database)
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

/// Memory creation payload - only fields needed for creation
/// Datetime field is handled by database default
/// Enum fields are converted to strings for SurrealDB compatibility
#[derive(Debug, Clone, Serialize)]
pub struct MemoryCreate {
    /// Unique identifier
    pub id: String,
    /// Type of memory content (as string for SurrealDB)
    #[serde(rename = "type")]
    pub memory_type: String,
    /// Text content of the memory
    pub content: String,
    /// Additional metadata
    pub metadata: serde_json::Value,
}

impl MemoryCreate {
    /// Creates a new MemoryCreate with the given parameters
    pub fn new(
        id: String,
        memory_type: MemoryType,
        content: String,
        metadata: serde_json::Value,
    ) -> Self {
        Self {
            id,
            memory_type: memory_type.to_string(),
            content,
            metadata,
        }
    }
}

/// Memory entity with embedding vector (for DB storage)
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
    /// Vector embedding (1536D for OpenAI/Mistral compatibility)
    pub embedding: Vec<f32>,
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
            metadata: serde_json::json!({"source": "settings"}),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"type\":\"context\""));
        assert!(json.contains("\"content\":\"User prefers dark mode\""));
        assert!(json.contains("\"source\":\"settings\""));

        let deserialized: Memory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, memory.id);
        assert_eq!(deserialized.memory_type, memory.memory_type);
        assert_eq!(deserialized.content, memory.content);
    }

    #[test]
    fn test_memory_with_embedding() {
        let memory = MemoryWithEmbedding {
            id: "mem_002".to_string(),
            memory_type: MemoryType::Knowledge,
            content: "Rust is a systems programming language".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
            metadata: serde_json::json!({}),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&memory).unwrap();
        assert!(json.contains("\"embedding\":[0.1,0.2,0.3]"));
    }

    #[test]
    fn test_memory_search_result() {
        let memory = Memory {
            id: "mem_003".to_string(),
            memory_type: MemoryType::Decision,
            content: "Chose SurrealDB for embedded database".to_string(),
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
