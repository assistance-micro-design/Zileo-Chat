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

//! Shared test utilities for Zileo Chat backend tests.
//!
//! Provides a single `setup_test_state()` function and seeding helpers
//! to eliminate duplication across command test modules.

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::state::AppState;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Once};
use tempfile::TempDir;

static TEST_TMP_INIT: Once = Once::new();

/// Creates a `TempDir` rooted in `target/test-tmp/` (on the real disk) instead
/// of `/tmp` (tmpfs). Tests that initialize SurrealDB can each use ~145 MB, and
/// running them in parallel would otherwise hit the tmpfs `usrquota` limit.
pub fn test_tempdir() -> TempDir {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("test-tmp");
    TEST_TMP_INIT.call_once(|| {
        std::fs::create_dir_all(&base).expect("Failed to create target/test-tmp");
    });
    tempfile::Builder::new()
        .prefix("zileo-test-")
        .tempdir_in(&base)
        .expect("Failed to create test tempdir")
}

/// Creates a fully initialized AppState with an ephemeral SurrealDB instance.
///
/// Returns `(AppState, TempDir)`. The caller MUST bind the `TempDir` (e.g.
/// `let (state, _db_guard) = setup_test_state().await;`) so the directory
/// lives for the duration of the test and is cleaned up when the test ends.
/// Dropping the `TempDir` before the test finishes will break any further DB
/// access (RocksDB needs the directory for WAL rotation and compaction).
pub async fn setup_test_state() -> (AppState, TempDir) {
    let temp_dir = test_tempdir();
    let db_path = temp_dir.path().join("test_db");
    let db_path_str = db_path.to_str().unwrap();

    let db = Arc::new(
        DBClient::new(db_path_str)
            .await
            .expect("Failed to create test DB"),
    );
    db.initialize_schema()
        .await
        .expect("Failed to initialize schema");

    let registry = Arc::new(AgentRegistry::new());
    let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
    let llm_manager = Arc::new(
        crate::llm::ProviderManager::new().expect("Failed to create test provider manager"),
    );
    let mcp_manager = Arc::new(
        crate::mcp::MCPManager::new(db.clone())
            .await
            .expect("Failed to create MCP manager"),
    );

    let embedding_service = Arc::new(tokio::sync::RwLock::new(None));

    let state = AppState {
        db: db.clone(),
        registry,
        orchestrator,
        llm_manager,
        mcp_manager,
        tool_factory: Arc::new(crate::tools::ToolFactory::new(
            db,
            embedding_service.clone(),
        )),
        embedding_service,
        streaming_cancellations: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        app_handle: Arc::new(std::sync::RwLock::new(None)),
    };

    (state, temp_dir)
}

/// Seeds a test prompt in the database and returns its ID.
///
/// Uses SET syntax with time::now() to avoid ERR_SURREAL_007 (datetime string rejection).
pub async fn seed_test_prompt(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE prompt:`{id}` SET \
            name = 'Test Prompt', \
            description = 'A test prompt for unit testing', \
            category = 'general', \
            content = 'Hello {{{{name}}}}, this is a test prompt', \
            variables = [{{ name: 'name', description: 'User name', default_value: 'World' }}], \
            created_at = time::now(), \
            updated_at = time::now()"
    );
    db.db
        .query(&query)
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE prompt failed validation");
    id
}

/// Seeds a test prompt with a specific category and returns its ID.
pub async fn seed_test_prompt_with_category(db: &DBClient, category: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE prompt:`{id}` SET \
            name = 'Test Prompt', \
            description = 'A test prompt', \
            category = $category, \
            content = 'Test content', \
            variables = [], \
            created_at = time::now(), \
            updated_at = time::now()"
    );
    db.db
        .query(&query)
        .bind(("category", category.to_string()))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE prompt with category failed validation");
    id
}

/// Seeds a test memory in the database and returns its ID.
///
/// Memory table has no datetime fields in CONTENT, but we add .check() for safety.
pub async fn seed_test_memory(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let data = serde_json::json!({
        "type": "knowledge",
        "content": "This is a test memory for unit testing purposes.",
        "metadata": {
            "tags": ["test", "unit"],
            "priority": null,
            "agent_source": null
        },
        "importance": 5.0,
        "embedding": null,
        "workflow_id": null
    });
    let response = db
        .db
        .query(format!("CREATE memory:`{}` CONTENT $data", id))
        .bind(("data", data))
        .await
        .expect("Query execution failed");
    response.check().expect("CREATE memory failed validation");
    id
}

/// Seeds a test memory WITH a 1024-dimension embedding vector.
///
/// Used for testing migration guards to verify
/// that embeddings survive when migrations are re-run.
pub async fn seed_test_memory_with_embedding(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    // Create a 1024-dimension embedding (matching HNSW index)
    let embedding: Vec<f64> = (0..1024).map(|i| (i as f64) * 0.001).collect();
    let data = serde_json::json!({
        "type": "knowledge",
        "content": "Memory with embedding for migration guard test.",
        "metadata": {
            "tags": ["test", "embedding"],
            "priority": null,
            "agent_source": null
        },
        "importance": 0.7,
        "embedding": embedding,
        "workflow_id": null
    });
    let response = db
        .db
        .query(format!("CREATE memory:`{}` CONTENT $data", id))
        .bind(("data", data))
        .await
        .expect("Query execution failed");
    response
        .check()
        .expect("CREATE memory with embedding failed validation");
    id
}
