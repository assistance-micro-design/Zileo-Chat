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

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::constants::compose::MAX_CONCURRENT_COMPOSE;
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::models::ReindexJobStatus;
use crate::tools::ToolFactory;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as StdMutex, RwLock as StdRwLock};
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// Application state shared across Tauri commands
pub struct AppState {
    /// Database client
    pub db: Arc<DBClient>,
    /// Agent registry
    pub registry: Arc<AgentRegistry>,
    /// Agent orchestrator
    pub orchestrator: Arc<AgentOrchestrator>,
    /// LLM provider manager
    pub llm_manager: Arc<ProviderManager>,
    /// MCP server manager
    pub mcp_manager: Arc<MCPManager>,
    /// Tool factory for agent tool instantiation
    pub tool_factory: Arc<ToolFactory>,
    /// Embedding service for semantic search (configured via Settings UI)
    ///
    /// NOTE: This uses a double-Arc pattern `Arc<RwLock<Option<Arc<T>>>>`.
    /// Could be simplified to `Arc<RwLock<Option<T>>>` if EmbeddingService implements Clone.
    /// Deferred as Nice-to-Have due to 12+ files affected. See optimization-db.md for details.
    pub embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>>,
    /// Cancellation tokens for streaming workflows (workflow_id -> CancellationToken)
    pub streaming_cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Cancellation tokens for background reindex jobs (job_id -> CancellationToken).
    /// Mirror of `streaming_cancellations` for the memory-reindex flow.
    pub reindex_cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Live snapshot of every reindex job spawned during this app session.
    /// In-memory only — a restart wipes the map, which is fine because the
    /// resumable WHERE clause picks up unindexed parents from scratch.
    pub reindex_jobs: Arc<Mutex<HashMap<String, ReindexJobStatus>>>,
    /// Tauri app handle for event emission (set after app initialization)
    /// Uses std::sync::RwLock for synchronous access in setup hook
    pub app_handle: Arc<StdRwLock<Option<AppHandle>>>,
    /// Background task that purges expired audit_validation rows.
    /// Stored so the runtime owns the handle (instead of detaching it) and so
    /// future shutdown hooks can `abort()` it deterministically.
    pub audit_cleanup_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Kanban scheduler background task handle.
    pub kanban_scheduler_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    /// Shutdown flag polled by the kanban scheduler on every tick.
    pub kanban_scheduler_shutdown: Arc<std::sync::atomic::AtomicBool>,
    /// In-flight async card-compose registry (card_id set), bounding concurrency
    /// to `MAX_CONCURRENT_COMPOSE`. A `std::sync::Mutex` (not `tokio::Mutex`) so
    /// the RAII [`ComposeSlotGuard`] can release a slot from `Drop` without an
    /// `.await`. In-memory only — reset to empty at reboot.
    pub compose_inflight: Arc<StdMutex<HashSet<String>>>,
    /// Latched signal flipped to `true` once the deferred boot init (MCP
    /// connect, providers, embedding) has finished. Boot tasks that need those
    /// services (card promotion, catch-up analyze) wait on a subscription
    /// before running, instead of racing the init now that the window — and
    /// thus these spawns — come up before the services are ready.
    pub services_ready: tokio::sync::watch::Sender<bool>,
    /// Flipped to `true` once the UI-critical services (providers, embedding)
    /// are ready — before MCP, which keeps connecting in the background. The
    /// frontend reads this (via `boot_ready_state`) to dismiss the splash even
    /// if it attached its `boot_ready` listener after the event already fired.
    pub ui_ready: Arc<std::sync::atomic::AtomicBool>,
    /// Handle to the deferred boot-init task, parked so a quit during the
    /// splash can `abort()` it before MCP shutdown runs.
    pub boot_task_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

/// Pure concurrency gate: can a new compose slot be reserved given the current
/// number of in-flight composes? Extracted so the cap frontier is unit-testable
/// without an `AppState`.
pub fn can_reserve_compose(current_len: usize) -> bool {
    current_len < MAX_CONCURRENT_COMPOSE
}

/// RAII guard that releases a reserved compose slot when dropped — covering BOTH
/// the normal end of the detached task AND a panic unwind (the tool-loop is the
/// most panic-prone code: SSE, third-party providers, RocksDB FFI). Without it a
/// panicking compose would leak its slot until reboot, and 4 panics would
/// saturate the cap (a self-inflicted DoS, H-2).
///
/// Poison-resilient: a mutex poisoned by a panicking holder is recovered via
/// `into_inner` so the registry never wedges (MINEUR-1).
pub struct ComposeSlotGuard {
    set: Arc<StdMutex<HashSet<String>>>,
    id: String,
}

impl Drop for ComposeSlotGuard {
    fn drop(&mut self) {
        let mut set = self.set.lock().unwrap_or_else(|p| p.into_inner());
        set.remove(&self.id);
    }
}

impl AppState {
    /// Creates new application state
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        // Initialize database
        let db = Arc::new(DBClient::new(db_path).await?);
        db.initialize_schema().await?;

        // One-shot data backfill for the token-cost-accuracy refactor.
        // Idempotent (gated by migration_log) and a no-op on fresh DBs:
        // touches only legacy `workflow` rows that lack the new columns.
        if let Err(e) = crate::commands::migration::run_token_cost_accuracy_v1(&db).await {
            tracing::warn!(
                error = %e,
                "token_cost_accuracy_v1 backfill failed; new fields will read as NONE on legacy rows"
            );
        }

        // Drop the removed `KanbanCardTool` entry from existing agents so it
        // doesn't trigger an "Unknown tool" warning on every workflow.
        // Idempotent and non-fatal — the factory already skips unknown tools.
        if let Err(e) = crate::commands::migration::run_remove_kanban_card_tool_v1(&db).await {
            tracing::warn!(
                error = %e,
                "remove_kanban_card_tool_v1 migration failed; unknown-tool warnings will appear at runtime"
            );
        }

        // Best-effort cleanup of memories past their TTL (and their chunks).
        // Non-fatal — search-time filtering already hides them, but purging
        // keeps the HNSW index lean and frees DB space.
        let purge = crate::db::queries::cleanup::purge_expired_memories(&db).await;
        if purge.memories_purged > 0 {
            tracing::info!(
                memories_purged = purge.memories_purged,
                chunks_purged = purge.chunks_purged,
                "Expired memories purged at startup"
            );
        }

        // Initialize agent registry and orchestrator
        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));

        // Initialize LLM provider manager
        let llm_manager = Arc::new(ProviderManager::new().map_err(|e| anyhow::anyhow!(e))?);

        // Initialize MCP manager
        let mcp_manager = Arc::new(
            MCPManager::new(db.clone())
                .await
                .map_err(|e| anyhow::anyhow!("Failed to initialize MCP manager: {}", e))?,
        );

        // Initialize embedding service as None (configured via Settings UI)
        let embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>> =
            Arc::new(RwLock::new(None));

        // Initialize tool factory with dynamic embedding service reference
        // ToolFactory reads current embedding state when creating tools
        let tool_factory = Arc::new(ToolFactory::new(db.clone(), embedding_service.clone()));

        // Initialize streaming cancellation token map
        let streaming_cancellations = Arc::new(Mutex::new(HashMap::new()));

        // Reindex job tracking + cancellation maps (in-memory, per-session).
        let reindex_cancellations = Arc::new(Mutex::new(HashMap::new()));
        let reindex_jobs = Arc::new(Mutex::new(HashMap::new()));

        // Initialize app handle as None (set later in setup hook)
        let app_handle = Arc::new(StdRwLock::new(None));

        // Audit cleanup task handle is registered later in the setup hook.
        let audit_cleanup_handle = Arc::new(Mutex::new(None));

        let kanban_scheduler_handle = Arc::new(Mutex::new(None));
        let kanban_scheduler_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));

        // In-flight compose registry starts empty (reset at every reboot).
        let compose_inflight = Arc::new(StdMutex::new(HashSet::new()));

        // Services start un-ready; the deferred boot task flips these once the
        // matching init completes (`ui_ready` after providers/embedding,
        // `services_ready` after MCP also connected).
        let (services_ready, _) = tokio::sync::watch::channel(false);
        let ui_ready = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let boot_task_handle = Arc::new(Mutex::new(None));

        Ok(Self {
            db,
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory,
            embedding_service,
            streaming_cancellations,
            reindex_cancellations,
            reindex_jobs,
            app_handle,
            audit_cleanup_handle,
            kanban_scheduler_handle,
            kanban_scheduler_shutdown,
            compose_inflight,
            services_ready,
            ui_ready,
            boot_task_handle,
        })
    }

    /// Atomically reserves a compose slot for `card_id` under a SINGLE lock
    /// (test-and-set — no TOCTOU window where two callers both read `len < cap`
    /// and then both insert). Returns a [`ComposeSlotGuard`] that releases the
    /// slot on drop, or an error string when the global cap is reached. The
    /// frontend gate is only advisory; THIS is the real anti-DoS guard.
    pub fn try_reserve_compose_slot(&self, card_id: &str) -> Result<ComposeSlotGuard, String> {
        let mut set = self
            .compose_inflight
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if !can_reserve_compose(set.len()) {
            return Err(format!(
                "Limite de {} générations simultanées atteinte",
                MAX_CONCURRENT_COMPOSE
            ));
        }
        set.insert(card_id.to_string());
        Ok(ComposeSlotGuard {
            set: self.compose_inflight.clone(),
            id: card_id.to_string(),
        })
    }

    /// Creates a cancellation token for a workflow and stores it.
    /// Returns the token for use with tokio::select!
    pub async fn create_cancellation_token(&self, workflow_id: &str) -> CancellationToken {
        let token = CancellationToken::new();
        self.streaming_cancellations
            .lock()
            .await
            .insert(workflow_id.to_string(), token.clone());
        token
    }

    /// Marks a workflow for cancellation by cancelling its token.
    ///
    /// Returns `true` when a running workflow token existed, `false` otherwise.
    pub async fn request_cancellation(&self, workflow_id: &str) -> bool {
        if let Some(token) = self.streaming_cancellations.lock().await.get(workflow_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Removes a workflow from the cancellation map
    pub async fn clear_cancellation(&self, workflow_id: &str) {
        self.streaming_cancellations
            .lock()
            .await
            .remove(workflow_id);
    }

    /// Initializes LLM providers from saved configuration.
    ///
    /// Called on app startup to restore provider configuration from the keystore.
    /// This ensures providers are ready to use without requiring the user to
    /// re-enter their API keys after each app restart.
    pub async fn initialize_providers_from_config(
        &self,
        keystore: &crate::commands::SecureKeyStore,
    ) {
        // Initialize Ollama (local provider, always available)
        if let Err(e) = self.llm_manager.configure_ollama(None).await {
            tracing::warn!(error = %e, "Failed to initialize Ollama provider");
        } else {
            tracing::info!("Ollama provider initialized");
        }

        // Initialize Mistral if API key is stored
        if let Some(api_key) = keystore.get_key("Mistral") {
            if !api_key.is_empty() {
                if let Err(e) = self.llm_manager.configure_mistral(&api_key).await {
                    tracing::warn!(error = %e, "Failed to initialize Mistral provider");
                } else {
                    tracing::info!("Mistral provider initialized from saved API key");
                }
            }
        } else {
            tracing::debug!("No Mistral API key found in keystore");
        }

        // Initialize custom providers from database
        let query = "SELECT name, base_url, supports_cache_control, supports_reasoning_param \
                     FROM custom_provider WHERE enabled = true";
        match self.db.query_json(query).await {
            Ok(results) => {
                for row in results {
                    let name = match row.get("name").and_then(|v| v.as_str()) {
                        Some(n) => n.to_string(),
                        None => continue,
                    };
                    let base_url = match row.get("base_url").and_then(|v| v.as_str()) {
                        Some(u) => u.to_string(),
                        None => continue,
                    };
                    let supports_cache_control =
                        row.get("supports_cache_control").and_then(|v| v.as_bool());
                    let supports_reasoning_param = row
                        .get("supports_reasoning_param")
                        .and_then(|v| v.as_bool());

                    let provider = std::sync::Arc::new(
                        crate::llm::openai_compatible::OpenAiCompatibleProvider::new(
                            &name,
                            self.llm_manager.http_client().clone(),
                        ),
                    );

                    // Configure with API key if available
                    if let Some(api_key) = keystore.get_key(&name) {
                        if !api_key.is_empty() {
                            if let Err(e) = provider.configure(&api_key, &base_url).await {
                                tracing::warn!(
                                    provider = %name,
                                    error = %e,
                                    "Failed to configure custom provider"
                                );
                            }
                        }
                    }

                    // Restore strict-mode toggles persisted on the row. Absent
                    // (NONE) means OpenRouter-preserving default — no-op write
                    // since `new()` already starts with `None`.
                    provider
                        .set_strict_compat(supports_cache_control, supports_reasoning_param)
                        .await;

                    self.llm_manager
                        .register_custom_provider(&name, provider)
                        .await;
                    tracing::info!(provider = %name, "Custom provider initialized from database");
                }
            }
            Err(e) => {
                tracing::debug!(error = %e, "No custom providers found (this is normal on first run)");
            }
        }
    }

    /// Initializes the embedding service from saved configuration.
    ///
    /// Called on app startup to restore embedding configuration from the database.
    /// Requires the SecureKeyStore to retrieve API keys for cloud providers.
    pub async fn initialize_embedding_from_config(
        &self,
        keystore: &crate::commands::SecureKeyStore,
    ) {
        use crate::llm::embedding::{EmbeddingProvider, EmbeddingService};
        use crate::models::EmbeddingConfigSettings;

        tracing::info!("Initializing embedding service from saved configuration...");

        // Load config from database using direct record access
        // Note: Using backtick-escaped ID for direct access instead of WHERE clause
        // to ensure correct record ID matching in SurrealDB
        // Note: Using query_json and SELECT config (not SELECT *) to avoid
        // SurrealDB SDK 2.x serialization issues with Thing enum type in id field
        let query = "SELECT config FROM settings:`settings:embedding_config`";
        let results: Vec<serde_json::Value> = match self.db.query_json(query).await {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!(error = %e, "No embedding config found in database (this is normal on first run)");
                return;
            }
        };

        // Parse config from result
        let config: EmbeddingConfigSettings = match results
            .first()
            .and_then(|row| row.get("config"))
            .and_then(|c| serde_json::from_value(c.clone()).ok())
        {
            Some(c) => c,
            None => {
                tracing::debug!("No embedding config stored, using defaults on first save");
                return;
            }
        };

        tracing::info!(
            provider = %config.provider,
            model = %config.model,
            "Loading embedding configuration from database"
        );

        // Create embedding provider based on config
        let provider = match config.provider.as_str() {
            "ollama" => {
                let base_url =
                    crate::commands::embedding::config::load_ollama_base_url(&self.db).await;
                Some(EmbeddingProvider::ollama_with_config(
                    &base_url,
                    &config.model,
                ))
            }
            "mistral" => {
                // Get API key from SecureKeyStore
                if let Some(api_key) = keystore.get_key("Mistral") {
                    Some(EmbeddingProvider::mistral_with_model(
                        &api_key,
                        &config.model,
                    ))
                } else {
                    tracing::warn!("Mistral API key not found, embedding service not initialized");
                    None
                }
            }
            _ => {
                tracing::warn!(provider = %config.provider, "Unknown embedding provider");
                None
            }
        };

        if let Some(provider) = provider {
            match EmbeddingService::with_provider(provider) {
                Ok(service) => {
                    *self.embedding_service.write().await = Some(Arc::new(service));
                    tracing::info!("Embedding service initialized from saved configuration");
                }
                Err(e) => {
                    tracing::error!("Failed to initialize embedding service: {}", e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_tempdir;

    /// H-2: the pure cap gate. With `MAX_CONCURRENT_COMPOSE = 4`, the frontier
    /// is exactly len 3 -> true, len 4 -> false.
    #[test]
    fn can_reserve_compose_frontier() {
        assert!(can_reserve_compose(0));
        assert!(can_reserve_compose(MAX_CONCURRENT_COMPOSE - 1));
        assert!(!can_reserve_compose(MAX_CONCURRENT_COMPOSE));
        assert!(!can_reserve_compose(MAX_CONCURRENT_COMPOSE + 1));
    }

    /// H-2 / MINEUR-1: `ComposeSlotGuard::drop` releases the slot even when the
    /// holder panics (unwind), so a panicking tool-loop never leaks its slot.
    #[test]
    fn compose_slot_guard_releases_on_panic_unwind() {
        let set: Arc<StdMutex<HashSet<String>>> = Arc::new(StdMutex::new(HashSet::new()));
        let set_for_closure = set.clone();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = ComposeSlotGuard {
                set: set_for_closure.clone(),
                id: "x".to_string(),
            };
            set_for_closure.lock().unwrap().insert("x".to_string());
            panic!("tool loop panicked mid-compose");
        }));
        assert!(result.is_err(), "the closure must have panicked");
        assert!(
            set.lock().unwrap().is_empty(),
            "ComposeSlotGuard::drop must release the slot during unwind"
        );
    }

    /// H-2: `try_reserve_compose_slot` reserves up to the cap, refuses the
    /// overflow, and a dropped guard frees a slot for re-reservation.
    #[tokio::test]
    async fn compose_slot_reserve_caps_and_releases() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("compose_slots_db");
        let state = AppState::new(db_path.to_str().unwrap()).await.unwrap();

        let mut guards = Vec::new();
        for i in 0..MAX_CONCURRENT_COMPOSE {
            guards.push(
                state
                    .try_reserve_compose_slot(&format!("c{i}"))
                    .expect("slot under cap must reserve"),
            );
        }
        // Cap reached -> the next reservation is refused.
        assert!(
            state.try_reserve_compose_slot("overflow").is_err(),
            "the {}-th compose must be refused",
            MAX_CONCURRENT_COMPOSE + 1
        );
        // Releasing one slot (guard drop) frees capacity for a new reservation.
        guards.pop();
        assert!(
            state.try_reserve_compose_slot("after-release").is_ok(),
            "a freed slot must be reservable again"
        );
    }

    #[tokio::test]
    async fn test_appstate_new_success() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db");
        let db_path_str = db_path.to_str().unwrap();

        let result = AppState::new(db_path_str).await;
        assert!(result.is_ok(), "AppState creation should succeed");

        let state = result.unwrap();
        // Verify all components are initialized
        let agents = state.registry.list().await;
        assert!(agents.is_empty(), "Registry should start empty");
    }

    #[tokio::test]
    async fn test_appstate_components_connected() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db2");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Register an agent
        use crate::agents::SimpleAgent;
        use crate::models::{AgentConfig, LLMConfig, Lifecycle};

        let config = AgentConfig {
            id: "state_test_agent".to_string(),
            name: "State Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Demo".to_string(),
                model: "test".to_string(),
                temperature: 0.7,
                max_tokens: 1000,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
            kind: None,
            auto_analyze_reports: false,
            mcp_tool_allowlist: Vec::new(),
        };

        let agent = SimpleAgent::new(config);
        state
            .registry
            .register("state_test_agent".to_string(), Arc::new(agent))
            .await;

        // Verify orchestrator can access agent through shared registry
        use crate::agents::core::agent::Task;
        let task = Task {
            id: "test_task".to_string(),
            description: "Test".to_string(),
            context: serde_json::json!({}),
        };

        let result = state
            .orchestrator
            .execute_with_mcp("state_test_agent", task, None, None)
            .await;
        assert!(
            result.is_ok(),
            "Orchestrator should execute via shared registry"
        );
    }

    #[tokio::test]
    async fn test_appstate_db_connection() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db3");
        let db_path_str = db_path.to_str().unwrap();

        // Test that AppState can initialize with DB
        let state = AppState::new(db_path_str).await;
        assert!(state.is_ok(), "AppState with DB should initialize");

        // Test basic query (schema creates tables)
        let state = state.unwrap();
        let result: Result<Vec<serde_json::Value>, _> = state.db.query("INFO FOR DB").await;
        assert!(result.is_ok(), "DB info query should succeed");
    }

    #[tokio::test]
    async fn test_appstate_invalid_path() {
        // Test with invalid path (directory that doesn't exist and can't be created)
        let result = AppState::new("/nonexistent/path/that/cannot/exist/db").await;
        assert!(result.is_err(), "Should fail with invalid path");
    }

    #[tokio::test]
    async fn test_appstate_arc_cloning() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db4");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Clone Arc references
        let db_clone = Arc::clone(&state.db);
        let registry_clone = Arc::clone(&state.registry);
        let orchestrator_clone = Arc::clone(&state.orchestrator);

        // Operations on clones should work
        let agents_original = state.registry.list().await;
        let agents_clone = registry_clone.list().await;
        assert_eq!(agents_original.len(), agents_clone.len());

        // Strong count should be 2 for each (except registry which is shared with orchestrator,
        // and db which is shared with mcp_manager and tool_factory)
        assert_eq!(Arc::strong_count(&state.db), 4); // db + mcp_manager + tool_factory + clone
        assert_eq!(Arc::strong_count(&state.registry), 3); // registry + orchestrator + clone
        assert_eq!(Arc::strong_count(&state.orchestrator), 2);

        drop(db_clone);
        drop(registry_clone);
        drop(orchestrator_clone);

        // Back to original counts
        assert_eq!(Arc::strong_count(&state.db), 3); // db + mcp_manager + tool_factory
    }

    #[tokio::test]
    async fn test_embedding_service_configuration() {
        use crate::llm::embedding::{EmbeddingProvider, EmbeddingService};

        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db7");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Initially no embedding service
        assert!(
            state.embedding_service.read().await.is_none(),
            "Embedding service should be None initially"
        );

        // Configure embedding service
        let provider =
            EmbeddingProvider::ollama_with_config("http://localhost:11434", "nomic-embed-text");
        let service =
            Arc::new(EmbeddingService::with_provider(provider).expect("test embedding service"));
        *state.embedding_service.write().await = Some(service.clone());

        // Verify it's set
        assert!(
            state.embedding_service.read().await.is_some(),
            "Embedding service should be set after configuration"
        );

        // Clear embedding service
        *state.embedding_service.write().await = None;
        assert!(
            state.embedding_service.read().await.is_none(),
            "Embedding service should be None after clearing"
        );
    }

    #[tokio::test]
    async fn test_tool_factory_available() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db8");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Tool factory should be available
        let available = crate::tools::ToolFactory::available_tools();
        assert!(available.contains(&"MemoryTool"));
        assert!(available.contains(&"TodoTool"));

        // Can create tools via factory (async now)
        let tool_result = state
            .tool_factory
            .create_tool(
                "MemoryTool",
                Some("wf_test".to_string()),
                "test_agent".to_string(),
                None, // app_handle not needed in tests
            )
            .await;
        assert!(tool_result.is_ok(), "Should create MemoryTool");
    }

    #[tokio::test]
    async fn test_streaming_cancellation() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db5");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();
        let workflow_id = "test_workflow_123";

        // Create a cancellation token first
        let token = state.create_cancellation_token(workflow_id).await;

        // Initially not cancelled
        assert!(
            !token.is_cancelled(),
            "Token should not be cancelled initially"
        );

        // Request cancellation
        assert!(state.request_cancellation(workflow_id).await);
        assert!(
            token.is_cancelled(),
            "Token should be cancelled after request"
        );

        // Clear cancellation (removes token from map)
        state.clear_cancellation(workflow_id).await;
        assert!(
            !state
                .streaming_cancellations
                .lock()
                .await
                .contains_key(workflow_id),
            "Workflow should not be in map after clearing"
        );
    }

    #[tokio::test]
    async fn test_multiple_cancellations() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db6");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Create tokens for multiple workflows
        let token1 = state.create_cancellation_token("wf1").await;
        let token2 = state.create_cancellation_token("wf2").await;
        let token3 = state.create_cancellation_token("wf3").await;

        // Cancel all three
        assert!(state.request_cancellation("wf1").await);
        assert!(state.request_cancellation("wf2").await);
        assert!(state.request_cancellation("wf3").await);

        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
        assert!(token3.is_cancelled());

        // Clear one
        state.clear_cancellation("wf2").await;
        let map = state.streaming_cancellations.lock().await;
        assert!(map.contains_key("wf1"));
        assert!(!map.contains_key("wf2")); // Removed from map
        assert!(map.contains_key("wf3"));
    }

    #[tokio::test]
    async fn test_cancellation_token_works_with_select() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db9");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();
        let workflow_id = "test_select_workflow";

        let token = state.create_cancellation_token(workflow_id).await;

        // Spawn a task that waits for cancellation
        let token_clone = token.clone();
        let handle = tokio::spawn(async move {
            token_clone.cancelled().await;
            "cancelled"
        });

        // Give the task a moment to start waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Request cancellation
        assert!(state.request_cancellation(workflow_id).await);

        // The task should complete quickly now
        let result = tokio::time::timeout(tokio::time::Duration::from_millis(100), handle).await;

        assert!(result.is_ok(), "Task should complete after cancellation");
        assert_eq!(result.unwrap().unwrap(), "cancelled");
    }
}
