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
use std::sync::Arc;
use tempfile::tempdir;

/// Creates a fully initialized AppState with an ephemeral SurrealDB instance.
///
/// The temp directory is intentionally leaked (`std::mem::forget`) to keep
/// the database alive for the duration of the test.
pub async fn setup_test_state() -> AppState {
    let temp_dir = tempdir().expect("Failed to create temp dir");
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

    // Leak temp_dir to keep it alive during test
    std::mem::forget(temp_dir);

    let embedding_service = Arc::new(tokio::sync::RwLock::new(None));

    AppState {
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
    }
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
/// Used for testing migration guards (SA-005 H3) to verify
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
