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

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Embedding configuration for persistence in settings
///
/// This struct mirrors `EmbeddingConfig` from `llm/embedding.rs`
/// but is designed for frontend serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfigSettings {
    /// Embedding provider: "mistral" or "ollama"
    pub provider: String,
    /// Embedding model name (e.g., "mistral-embed", "nomic-embed-text")
    pub model: String,
    /// Vector dimension (auto-set based on model)
    pub dimension: usize,
    /// Maximum tokens per input (provider-specific)
    pub max_tokens: usize,
    /// Characters per chunk for long texts
    pub chunk_size: usize,
    /// Overlap between chunks in characters
    pub chunk_overlap: usize,
    /// Chunking strategy: "fixed", "semantic", or "recursive"
    pub strategy: Option<String>,
}

impl Default for EmbeddingConfigSettings {
    fn default() -> Self {
        Self {
            provider: "mistral".to_string(),
            model: "mistral-embed".to_string(),
            dimension: 1024,
            max_tokens: 8192,
            chunk_size: 512,
            chunk_overlap: 50,
            strategy: Some("fixed".to_string()),
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

/// Result of embedding regeneration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegenerateResult {
    /// Number of memories processed
    pub processed: usize,
    /// Number of embeddings successfully generated
    pub success: usize,
    /// Number of failures
    pub failed: usize,
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
    /// Error message if failed
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
        assert_eq!(config.dimension, 1024);
        assert_eq!(config.chunk_size, 512);
        assert_eq!(config.chunk_overlap, 50);
    }

    #[test]
    fn test_embedding_config_serialization() {
        let config = EmbeddingConfigSettings::default();
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"provider\":\"mistral\""));
        assert!(json.contains("\"model\":\"mistral-embed\""));
        assert!(json.contains("\"dimension\":1024"));
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
