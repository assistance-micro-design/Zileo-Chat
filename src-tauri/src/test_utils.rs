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
        reindex_cancellations: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        reindex_jobs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        app_handle: Arc::new(std::sync::RwLock::new(None)),
        audit_cleanup_handle: Arc::new(tokio::sync::Mutex::new(None)),
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

// ============================================================================
// Agent test fixtures (cross-module use)
// ============================================================================

/// Minimal `Agent` implementation usable across crate test modules.
///
/// Carries an `AgentConfig` with caller-controlled `provider` and `model`
/// fields, which is the only data downstream pricing code reads from the
/// registry. The trait body returns degenerate values: registering this
/// instance is enough to make `AgentRegistry::get` resolve the id.
pub struct TestRegistryAgent {
    config: crate::models::AgentConfig,
}

impl TestRegistryAgent {
    /// Builds a test agent whose `config().llm` reports the given provider /
    /// api_name. Used by sub-agent cost tests to assert that pricing lookup
    /// uses the SUB-agent's model, not the parent's.
    pub fn new(id: &str, provider: &str, api_name: &str) -> Self {
        use crate::models::{AgentConfig, LLMConfig, Lifecycle};
        Self {
            config: AgentConfig {
                id: id.to_string(),
                name: format!("Test Agent {}", id),
                lifecycle: Lifecycle::Temporary,
                llm: LLMConfig {
                    provider: provider.to_string(),
                    model: api_name.to_string(),
                    temperature: 0.0,
                    max_tokens: 0,
                    is_reasoning: false,
                    context_window: None,
                },
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
                folders: vec![],
                require_file_confirmation: false,
                system_prompt: String::new(),
                max_tool_iterations: 0,
                reasoning_effort: None,
            },
        }
    }
}

#[async_trait::async_trait]
impl crate::agents::core::agent::Agent for TestRegistryAgent {
    async fn execute(
        &self,
        _task: crate::agents::core::agent::Task,
    ) -> anyhow::Result<crate::agents::core::agent::Report> {
        anyhow::bail!("TestRegistryAgent::execute is intentionally unsupported")
    }
    fn capabilities(&self) -> Vec<String> {
        vec![]
    }
    fn lifecycle(&self) -> crate::models::Lifecycle {
        self.config.lifecycle.clone()
    }
    fn tools(&self) -> Vec<String> {
        vec![]
    }
    fn mcp_servers(&self) -> Vec<String> {
        vec![]
    }
    fn config(&self) -> &crate::models::AgentConfig {
        &self.config
    }
}

/// Seeds an `llm_model` row with explicit per-MTok prices.
///
/// Returns the model id. Used by pricing/aggregation tests so
/// `load_pricing_row` finds a row with deterministic prices.
pub async fn seed_llm_model(
    db: &DBClient,
    provider: &str,
    api_name: &str,
    input_price: f64,
    output_price: f64,
) -> String {
    seed_llm_model_with_cache(db, provider, api_name, input_price, output_price, 0.0, 0.0).await
}

/// Seeds an `llm_model` row including cache pricing (for cache-aware tests).
pub async fn seed_llm_model_with_cache(
    db: &DBClient,
    provider: &str,
    api_name: &str,
    input_price: f64,
    output_price: f64,
    cache_read_price: f64,
    cache_write_price: f64,
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let provider_lower = provider.to_lowercase();
    let query = "CREATE llm_model:`{id}` SET \
            id = $id, \
            provider = $provider, \
            name = $api_name, \
            api_name = $api_name, \
            context_window = 128000, \
            max_output_tokens = 4096, \
            temperature_default = 0.0, \
            is_builtin = false, \
            is_reasoning = false, \
            input_price_per_mtok = $input_price, \
            output_price_per_mtok = $output_price, \
            cache_read_price_per_mtok = $cache_read_price, \
            cache_write_price_per_mtok = $cache_write_price"
        .replace("{id}", &id);
    db.db
        .query(&query)
        .bind(("id", id.clone()))
        .bind(("provider", provider_lower))
        .bind(("api_name", api_name.to_string()))
        .bind(("input_price", input_price))
        .bind(("output_price", output_price))
        .bind(("cache_read_price", cache_read_price))
        .bind(("cache_write_price", cache_write_price))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE llm_model failed validation");
    id
}

/// Seeds a minimal `workflow` row so subsequent UPDATEs land somewhere.
///
/// Returns the workflow id. Used by aggregation tests to verify that
/// `aggregate_sub_agent_metrics` writes onto the right row.
pub async fn seed_test_workflow(db: &DBClient) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE workflow:`{id}` SET \
            name = 'Test Workflow', \
            agent_id = 'test-agent', \
            status = 'idle', \
            pinned = false"
    );
    db.db
        .query(&query)
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE workflow failed validation");
    id
}

/// Inserts a `sub_agent_execution` row with arbitrary metrics + status.
///
/// `cost_usd = None` is stored as SurrealQL NONE so the aggregation can
/// verify that legacy rows (no cost) cleanly sum to 0.
#[allow(clippy::too_many_arguments)]
pub async fn seed_sub_agent_execution(
    db: &DBClient,
    workflow_id: &str,
    sub_agent_id: &str,
    status: &str,
    tokens_input: i64,
    tokens_output: i64,
    cost_usd: Option<f64>,
) {
    let id = uuid::Uuid::new_v4().to_string();
    let cost_clause = match cost_usd {
        Some(c) => format!("cost_usd = {}", c),
        None => "cost_usd = NONE".to_string(),
    };
    let query = format!(
        "CREATE sub_agent_execution:`{id}` SET \
            workflow_id = $wf_id, \
            parent_agent_id = 'test-parent', \
            sub_agent_id = $sub_id, \
            sub_agent_name = 'Test SubAgent', \
            task_description = 'Test task', \
            status = $status, \
            duration_ms = 100, \
            tokens_input = {tokens_input}, \
            tokens_output = {tokens_output}, \
            {cost_clause}"
    );
    db.db
        .query(&query)
        .bind(("wf_id", workflow_id.to_string()))
        .bind(("sub_id", sub_agent_id.to_string()))
        .bind(("status", status.to_string()))
        .await
        .expect("Query execution failed")
        .check()
        .expect("CREATE sub_agent_execution failed validation");
}
