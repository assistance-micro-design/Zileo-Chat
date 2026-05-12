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

//! Embedding settings models for Memory Tool configuration.
//!
//! These types are synchronized with TypeScript types (src/types/embedding.ts)
//! for IPC communication via Tauri commands.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Embedding configuration for persistence in settings
///
/// This struct mirrors `EmbeddingConfig` from `llm/embedding.rs`
/// but is designed for frontend serialization.
///
/// Chunking parameters live in `tools/memory/chunker.rs` (constants) and the
/// vector dimension is fixed by the HNSW schema (1024D) — neither is user
/// configurable and both have been removed from this struct.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfigSettings {
    /// Embedding provider: "mistral" or "ollama"
    pub provider: String,
    /// Embedding model name (e.g., "mistral-embed", "mxbai-embed-large")
    pub model: String,
}

impl Default for EmbeddingConfigSettings {
    fn default() -> Self {
        Self {
            provider: "mistral".to_string(),
            model: "mistral-embed".to_string(),
        }
    }
}

/// Memory statistics for the settings dashboard
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total number of memories
    pub total: usize,
    /// Memories with embeddings generated
    pub with_embeddings: usize,
    /// Memories without embeddings
    pub without_embeddings: usize,
    /// Memory count by type
    pub by_type: HashMap<String, usize>,
    /// Memory count by agent source
    pub by_agent: HashMap<String, usize>,
}

/// Result of memory import operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    /// Number of memories successfully imported
    pub imported: usize,
    /// Number of memories that failed to import
    pub failed: usize,
    /// Error messages for failed imports
    pub errors: Vec<String>,
}

/// Result of embedding test operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingTestResult {
    /// Whether embedding was generated successfully
    pub success: bool,
    /// Vector dimension (e.g., 1024)
    pub dimension: usize,
    /// First 5 values of the embedding (preview)
    pub preview: Vec<f32>,
    /// Generation time in milliseconds
    pub duration_ms: u64,
    /// Provider used (mistral/ollama)
    pub provider: String,
    /// Model used
    pub model: String,
    /// Error message if failed (omitted from JSON when None)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Token statistics for memory categories
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryTokenStats {
    /// Statistics per memory type
    pub categories: Vec<CategoryTokenStats>,
    /// Total characters across all categories
    pub total_chars: usize,
    /// Estimated total tokens (chars / 4)
    pub total_estimated_tokens: usize,
    /// Total memories counted
    pub total_memories: usize,
}

/// Token statistics for a single category
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryTokenStats {
    /// Memory type (user_pref, context, knowledge, decision)
    pub memory_type: String,
    /// Number of memories in this category
    pub count: usize,
    /// Total characters in this category
    pub total_chars: usize,
    /// Estimated tokens (chars / 4)
    pub estimated_tokens: usize,
    /// Average characters per memory
    pub avg_chars: usize,
    /// Number with embeddings
    pub with_embeddings: usize,
}

/// Snapshot of a running or recently-finished reindex job.
///
/// Mirrored verbatim by the frontend `ReindexJobStatus` interface so the
/// in-memory job map (`AppState.reindex_jobs`) can be queried by the UI on
/// remount. `serde(rename_all = "camelCase")` keeps the IPC contract aligned
/// with TS conventions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReindexJobStatus {
    /// Job identifier returned by `reindex_memory_chunks` at spawn time.
    pub job_id: String,
    /// One of "running" | "completed" | "cancelled" | "error".
    pub status: String,
    /// Number of parent memories processed so far.
    pub processed: usize,
    /// Total number of pending parents at job start (`0` until first emit).
    pub total: usize,
    /// Cumulative count of chunks created across all processed parents.
    pub chunks_created: usize,
    /// UUID of the memory currently being processed (`None` between rows).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_memory_id: Option<String>,
    /// Error message when `status == "error"`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// Job spawn timestamp.
    pub started_at: DateTime<Utc>,
    /// Terminal timestamp (`None` while running).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
}

/// Memory export format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// JSON format
    #[default]
    Json,
    /// CSV format
    Csv,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfigSettings::default();
        assert_eq!(config.provider, "mistral");
        assert_eq!(config.model, "mistral-embed");
    }

    #[test]
    fn test_embedding_config_serialization() {
        let config = EmbeddingConfigSettings::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"provider\":\"mistral\""));
        assert!(json.contains("\"model\":\"mistral-embed\""));
    }

    #[test]
    fn test_embedding_config_tolerates_legacy_fields_in_db() {
        // Existing installs may still have rows that include the legacy
        // decorative fields (dimension/max_tokens/chunk_size/chunk_overlap/
        // strategy). serde_json must accept and ignore them so users do not
        // hit a deserialization failure after upgrading.
        let legacy_json = r#"{
            "provider": "mistral",
            "model": "mistral-embed",
            "dimension": 1024,
            "max_tokens": 8192,
            "chunk_size": 512,
            "chunk_overlap": 50,
            "strategy": "fixed"
        }"#;
        let config: EmbeddingConfigSettings = serde_json::from_str(legacy_json).unwrap();
        assert_eq!(config.provider, "mistral");
        assert_eq!(config.model, "mistral-embed");
    }

    #[test]
    fn test_memory_stats_default() {
        let stats = MemoryStats::default();
        assert_eq!(stats.total, 0);
        assert_eq!(stats.with_embeddings, 0);
        assert_eq!(stats.without_embeddings, 0);
        assert!(stats.by_type.is_empty());
        assert!(stats.by_agent.is_empty());
    }

    #[test]
    fn test_import_result_serialization() {
        let result = ImportResult {
            imported: 10,
            failed: 2,
            errors: vec!["Error 1".to_string(), "Error 2".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"imported\":10"));
        assert!(json.contains("\"failed\":2"));
    }

    #[test]
    fn test_export_format_serialization() {
        let json_format = ExportFormat::Json;
        let csv_format = ExportFormat::Csv;

        assert_eq!(serde_json::to_string(&json_format).unwrap(), "\"json\"");
        assert_eq!(serde_json::to_string(&csv_format).unwrap(), "\"csv\"");
    }

    #[test]
    fn test_embedding_test_result_serialization() {
        let result = EmbeddingTestResult {
            success: true,
            dimension: 1024,
            preview: vec![0.1, 0.2, 0.3, 0.4, 0.5],
            duration_ms: 150,
            provider: "mistral".to_string(),
            model: "mistral-embed".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"dimension\":1024"));
    }

    #[test]
    fn test_memory_token_stats_default() {
        let stats = MemoryTokenStats::default();
        assert_eq!(stats.total_chars, 0);
        assert_eq!(stats.total_estimated_tokens, 0);
        assert!(stats.categories.is_empty());
    }

    #[test]
    fn test_category_token_stats_serialization() {
        let cat = CategoryTokenStats {
            memory_type: "knowledge".to_string(),
            count: 10,
            total_chars: 5000,
            estimated_tokens: 1250,
            avg_chars: 500,
            with_embeddings: 8,
        };
        let json = serde_json::to_string(&cat).unwrap();
        assert!(json.contains("\"memory_type\":\"knowledge\""));
        assert!(json.contains("\"estimated_tokens\":1250"));
    }
}
