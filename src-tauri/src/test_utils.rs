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
    let llm_manager = Arc::new(crate::llm::ProviderManager::new());
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

/// Seeds a test agent in the database and returns its ID.
pub async fn seed_test_agent(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE agent:`{id}` SET \
            id = '{id}', \
            name = 'Test Agent', \
            lifecycle = 'permanent', \
            llm = {{ provider: 'mistral', model: 'large', temperature: 0.7, max_tokens: 1000 }}, \
            tools = [], \
            mcp_servers = [], \
            system_prompt = 'You are a test agent.', \
            max_tool_iterations = 50, \
            enable_thinking = false, \
            created_at = time::now(), \
            updated_at = time::now()"
    );
    db.db
        .query(&query)
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE agent failed validation");
    id
}

/// Seeds a test workflow in the database and returns its ID.
pub async fn seed_test_workflow(db: &DBClient, agent_id: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE workflow:`{id}` SET \
            name = 'Test Workflow', \
            agent_id = $agent_id, \
            status = 'idle', \
            model_id = NONE, \
            created_at = time::now(), \
            updated_at = time::now()"
    );
    db.db
        .query(&query)
        .bind(("agent_id", agent_id.to_string()))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE workflow failed validation");
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
    let mut response = db
        .db
        .query(&format!("CREATE memory:`{}` CONTENT $data", id))
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
    let mut response = db
        .db
        .query(&format!("CREATE memory:`{}` CONTENT $data", id))
        .bind(("data", data))
        .await
        .expect("Query execution failed");
    response
        .check()
        .expect("CREATE memory with embedding failed validation");
    id
}

/// Seeds a test LLM model in the database and returns its ID.
pub async fn seed_test_model(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE llm_model:`{id}` SET \
            name = 'Test Model', \
            api_name = 'test-model-api', \
            provider = 'mistral', \
            context_window = 32000, \
            is_builtin = false, \
            created_at = time::now(), \
            updated_at = time::now()"
    );
    db.db
        .query(&query)
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE llm_model failed validation");
    id
}

/// Seeds a test task in the database and returns its ID.
pub async fn seed_test_task(db: &DBClient, workflow_id: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE task:`{id}` SET \
            workflow_id = $workflow_id, \
            name = 'Test Task', \
            description = 'A test task', \
            status = 'pending', \
            priority = 3, \
            created_at = time::now(), \
            updated_at = time::now()"
    );
    db.db
        .query(&query)
        .bind(("workflow_id", workflow_id.to_string()))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE task failed validation");
    id
}

/// Seeds a test message in the database and returns its ID.
pub async fn seed_test_message(db: &DBClient, workflow_id: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE message:`{id}` SET \
            workflow_id = $workflow_id, \
            role = 'user', \
            content = 'Test message content', \
            tokens = 10, \
            created_at = time::now()"
    );
    db.db
        .query(&query)
        .bind(("workflow_id", workflow_id.to_string()))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE message failed validation");
    id
}

/// Seeds a test MCP call log in the database and returns its ID.
pub async fn seed_test_mcp_call_log(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let params_json = serde_json::to_string(&serde_json::json!({
        "key": "value",
        "nested": {"a": 1, "b": "test"}
    }))
    .unwrap();
    let result_json = serde_json::to_string(&serde_json::json!([
        {"output": "hello", "type": "text"}
    ]))
    .unwrap();

    let query = format!(
        "CREATE mcp_call_log:`{id}` SET \
            workflow_id = 'test-workflow', \
            server_name = 'test-server', \
            tool_name = 'test-tool', \
            params = $params, \
            result = $result, \
            duration_ms = 100, \
            success = true, \
            timestamp = time::now()"
    );
    db.db
        .query(&query)
        .bind(("params", params_json))
        .bind(("result", result_json))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE mcp_call_log failed validation");
    id
}

/// Asserts that a query returns at least `expected_min` rows.
pub async fn assert_query_returns_rows(db: &DBClient, query: &str, expected_min: usize) {
    let results: Vec<serde_json::Value> = db
        .query_json(query)
        .await
        .expect("Query failed in assertion");
    assert!(
        results.len() >= expected_min,
        "Expected at least {} rows, got {}. Query: {}",
        expected_min,
        results.len(),
        query
    );
}

/// Asserts that a query returns zero rows.
pub async fn assert_query_returns_empty(db: &DBClient, query: &str) {
    let results: Vec<serde_json::Value> = db
        .query_json(query)
        .await
        .expect("Query failed in assertion");
    assert!(
        results.is_empty(),
        "Expected empty result, got {} rows. Query: {}",
        results.len(),
        query
    );
}
