// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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

/// Result of testing embedding generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingTestResult {
    /// Whether the test was successful
    pub success: bool,
    /// Status message
    pub message: String,
    /// Generated embedding dimension (if successful)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<usize>,
    /// First 5 values of the embedding (preview)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<Vec<f32>>,
    /// Time taken in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
}

impl EmbeddingTestResult {
    /// Creates a successful test result
    pub fn success(dimension: usize, preview: Vec<f32>, latency_ms: u64) -> Self {
        Self {
            success: true,
            message: format!(
                "Embedding generated successfully: {}D vector in {}ms",
                dimension, latency_ms
            ),
            dimension: Some(dimension),
            preview: Some(preview),
            latency_ms: Some(latency_ms),
        }
    }

    /// Creates a failed test result
    pub fn failure(message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            dimension: None,
            preview: None,
            latency_ms: None,
        }
    }
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

/// Memory export format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    /// JSON format
    Json,
    /// CSV format
    Csv,
}

impl Default for ExportFormat {
    fn default() -> Self {
        Self::Json
    }
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
    fn test_embedding_test_result_success() {
        let result = EmbeddingTestResult::success(1024, vec![0.1, 0.2, 0.3, 0.4, 0.5], 150);
        assert!(result.success);
        assert_eq!(result.dimension, Some(1024));
        assert_eq!(result.latency_ms, Some(150));
        assert!(result.message.contains("1024D"));
    }

    #[test]
    fn test_embedding_test_result_failure() {
        let result = EmbeddingTestResult::failure("Connection failed");
        assert!(!result.success);
        assert_eq!(result.message, "Connection failed");
        assert!(result.dimension.is_none());
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
}
