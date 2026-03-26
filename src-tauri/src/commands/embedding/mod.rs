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

//! Embedding & Memory Commands
//!
//! Tauri commands for managing embedding configuration, memory operations,
//! and statistics from the Settings UI.
//!
//! ## Config Commands
//! - `get_embedding_config` / `save_embedding_config` - Settings CRUD
//! - `reinit_embedding_service` / `test_embedding` - Service management
//!
//! ## Memory Commands
//! - `update_memory` - Update a memory entry
//! - `export_memories` / `import_memories` - Bulk operations
//! - `regenerate_embeddings` - Regenerate all embeddings
//!
//! ## Stats Commands
//! - `get_memory_stats` / `get_memory_token_stats` - Dashboard statistics

pub mod config;
pub mod operations;
pub mod stats;

#[cfg(test)]
mod tests {
    use crate::models::{EmbeddingConfigSettings, ExportFormat};

    #[test]
    fn test_embedding_config_settings_default() {
        let config = EmbeddingConfigSettings::default();
        assert_eq!(config.provider, "mistral");
        assert_eq!(config.model, "mistral-embed");
        assert_eq!(config.dimension, 1024);
    }

    #[test]
    fn test_export_format_serialization() {
        let json = ExportFormat::Json;
        let csv = ExportFormat::Csv;

        assert_eq!(serde_json::to_string(&json).unwrap(), "\"json\"");
        assert_eq!(serde_json::to_string(&csv).unwrap(), "\"csv\"");
    }

    #[tokio::test]
    async fn test_import_memory_injection_safe() {
        let state = crate::test_utils::setup_test_state().await;
        // First seed a legitimate memory
        crate::test_utils::seed_test_memory(&state.db).await;

        // Attempt injection via memory content
        let memory_id = uuid::Uuid::new_v4().to_string();
        let injection_content = "Normal text'; DELETE memory WHERE '1'='1; --";
        let sanitized = injection_content.replace('\0', "");
        let create_query = format!(
            "CREATE memory:`{}` CONTENT {{ type: $mtype, content: $content, metadata: $metadata }}",
            memory_id
        );
        state
            .db
            .execute_with_params(
                &create_query,
                vec![
                    ("mtype".to_string(), serde_json::json!("knowledge")),
                    ("content".to_string(), serde_json::json!(sanitized)),
                    ("metadata".to_string(), serde_json::json!({})),
                ],
            )
            .await
            .unwrap();

        // Verify the original memory still exists
        let all: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT meta::id(id) AS id FROM memory")
            .await
            .unwrap();
        assert!(
            all.len() >= 2,
            "Both memories should exist (original + injected content)"
        );
    }

    #[tokio::test]
    async fn test_regenerate_type_filter_injection_safe() {
        let state = crate::test_utils::setup_test_state().await;
        crate::test_utils::seed_test_memory(&state.db).await;

        // Attempt injection via type_filter parameter
        let results: Vec<serde_json::Value> = state
            .db
            .query_json_with_params(
                "SELECT meta::id(id) AS id, content FROM memory WHERE type = $mtype",
                vec![(
                    "mtype".to_string(),
                    serde_json::json!("'; DROP TABLE memory; --"),
                )],
            )
            .await
            .unwrap();
        assert!(results.is_empty(), "Injection string should match nothing");

        // Verify data is intact
        let all: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT meta::id(id) AS id FROM memory")
            .await
            .unwrap();
        assert!(!all.is_empty(), "Memory data should be preserved");
    }
}
