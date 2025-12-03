// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::tools::ToolFactory;
use std::collections::HashMap;
use std::sync::{Arc, RwLock as StdRwLock};
use tauri::AppHandle;
use tokio::sync::{Mutex, RwLock};
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
    /// Tool factory for agent tool instantiation (used in Phase 6)
    #[allow(dead_code)]
    pub tool_factory: Arc<ToolFactory>,
    /// Embedding service for semantic search (configured via Settings UI)
    #[allow(dead_code)]
    pub embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>>,
    /// Cancellation tokens for streaming workflows (workflow_id -> CancellationToken)
    pub streaming_cancellations: Arc<Mutex<HashMap<String, CancellationToken>>>,
    /// Tauri app handle for event emission (set after app initialization)
    /// Uses std::sync::RwLock for synchronous access in setup hook
    pub app_handle: Arc<StdRwLock<Option<AppHandle>>>,
}

impl AppState {
    /// Creates new application state
    pub async fn new(db_path: &str) -> anyhow::Result<Self> {
        // Initialize database
        let db = Arc::new(DBClient::new(db_path).await?);
        db.initialize_schema().await?;

        // Initialize agent registry and orchestrator
        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));

        // Initialize LLM provider manager
        let llm_manager = Arc::new(ProviderManager::new());

        // Initialize MCP manager
        let mcp_manager = Arc::new(
            MCPManager::new(db.clone())
                .await
                .expect("Failed to initialize MCP manager"),
        );

        // Initialize embedding service as None (configured via Settings UI)
        let embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>> =
            Arc::new(RwLock::new(None));

        // Initialize tool factory (will use embedding_service when configured)
        let tool_factory = Arc::new(ToolFactory::new(db.clone(), None));

        // Initialize streaming cancellation token map
        let streaming_cancellations = Arc::new(Mutex::new(HashMap::new()));

        // Initialize app handle as None (set later in setup hook)
        let app_handle = Arc::new(StdRwLock::new(None));

        Ok(Self {
            db,
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory,
            embedding_service,
            streaming_cancellations,
            app_handle,
        })
    }

    /// Sets the Tauri app handle for event emission.
    ///
    /// This should be called in the Tauri setup hook after the app is built.
    /// Uses std::sync::RwLock for synchronous access.
    #[allow(dead_code)]
    pub fn set_app_handle(&self, handle: AppHandle) {
        if let Ok(mut guard) = self.app_handle.write() {
            *guard = Some(handle);
        }
    }

    /// Gets the app handle if available.
    #[allow(dead_code)]
    pub fn get_app_handle(&self) -> Option<AppHandle> {
        self.app_handle.read().ok().and_then(|guard| guard.clone())
    }

    /// Updates the embedding service configuration.
    ///
    /// Called when user configures embedding settings in the Settings UI.
    /// This will update the tool factory to use the new embedding service.
    #[allow(dead_code)]
    pub async fn set_embedding_service(&self, service: Option<Arc<EmbeddingService>>) {
        *self.embedding_service.write().await = service.clone();
        // Note: Tool instances already created won't be updated.
        // New tool instances will use the updated embedding service.
    }

    /// Gets the current embedding service if configured.
    #[allow(dead_code)]
    pub async fn get_embedding_service(&self) -> Option<Arc<EmbeddingService>> {
        self.embedding_service.read().await.clone()
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

    /// Gets the cancellation token for a workflow if it exists.
    #[allow(dead_code)]
    pub async fn get_cancellation_token(&self, workflow_id: &str) -> Option<CancellationToken> {
        self.streaming_cancellations
            .lock()
            .await
            .get(workflow_id)
            .cloned()
    }

    /// Checks if a workflow has been requested to cancel
    pub async fn is_cancelled(&self, workflow_id: &str) -> bool {
        self.streaming_cancellations
            .lock()
            .await
            .get(workflow_id)
            .map(|token| token.is_cancelled())
            .unwrap_or(false)
    }

    /// Marks a workflow for cancellation by cancelling its token
    pub async fn request_cancellation(&self, workflow_id: &str) {
        if let Some(token) = self.streaming_cancellations.lock().await.get(workflow_id) {
            token.cancel();
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
            "ollama" => Some(EmbeddingProvider::ollama_with_config(
                "http://localhost:11434",
                &config.model,
            )),
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
            let service = EmbeddingService::with_provider(provider);
            *self.embedding_service.write().await = Some(Arc::new(service));
            tracing::info!("Embedding service initialized from saved configuration");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_appstate_new_success() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
            },
            tools: vec![],
            mcp_servers: vec![],
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
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

        let result = state.orchestrator.execute("state_test_agent", task).await;
        assert!(
            result.is_ok(),
            "Orchestrator should execute via shared registry"
        );
    }

    #[tokio::test]
    async fn test_appstate_db_connection() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
        let temp_dir = tempdir().expect("Failed to create temp dir");
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

        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db7");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Initially no embedding service
        assert!(
            state.get_embedding_service().await.is_none(),
            "Embedding service should be None initially"
        );

        // Configure embedding service
        let provider = EmbeddingProvider::ollama();
        let service = Arc::new(EmbeddingService::with_provider(provider));
        state.set_embedding_service(Some(service.clone())).await;

        // Verify it's set
        let retrieved = state.get_embedding_service().await;
        assert!(
            retrieved.is_some(),
            "Embedding service should be set after configuration"
        );

        // Clear embedding service
        state.set_embedding_service(None).await;
        assert!(
            state.get_embedding_service().await.is_none(),
            "Embedding service should be None after clearing"
        );
    }

    #[tokio::test]
    async fn test_tool_factory_available() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db8");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Tool factory should be available
        let available = crate::tools::ToolFactory::available_tools();
        assert!(available.contains(&"MemoryTool"));
        assert!(available.contains(&"TodoTool"));

        // Can create tools via factory
        let tool_result = state.tool_factory.create_tool(
            "MemoryTool",
            Some("wf_test".to_string()),
            "test_agent".to_string(),
            None, // app_handle not needed in tests
        );
        assert!(tool_result.is_ok(), "Should create MemoryTool");
    }

    #[tokio::test]
    async fn test_streaming_cancellation() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db5");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();
        let workflow_id = "test_workflow_123";

        // Create a cancellation token first
        let token = state.create_cancellation_token(workflow_id).await;

        // Initially not cancelled (token exists but not cancelled)
        assert!(
            !state.is_cancelled(workflow_id).await,
            "Workflow should not be cancelled initially"
        );
        assert!(
            !token.is_cancelled(),
            "Token should not be cancelled initially"
        );

        // Request cancellation
        state.request_cancellation(workflow_id).await;
        assert!(
            state.is_cancelled(workflow_id).await,
            "Workflow should be cancelled after request"
        );
        assert!(
            token.is_cancelled(),
            "Token should be cancelled after request"
        );

        // Clear cancellation (removes token from map)
        state.clear_cancellation(workflow_id).await;
        assert!(
            !state.is_cancelled(workflow_id).await,
            "Workflow should not be in map after clearing"
        );
    }

    #[tokio::test]
    async fn test_multiple_cancellations() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db6");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Create tokens for multiple workflows
        let _token1 = state.create_cancellation_token("wf1").await;
        let _token2 = state.create_cancellation_token("wf2").await;
        let _token3 = state.create_cancellation_token("wf3").await;

        // Cancel all three
        state.request_cancellation("wf1").await;
        state.request_cancellation("wf2").await;
        state.request_cancellation("wf3").await;

        assert!(state.is_cancelled("wf1").await);
        assert!(state.is_cancelled("wf2").await);
        assert!(state.is_cancelled("wf3").await);
        assert!(!state.is_cancelled("wf4").await); // Never created

        // Clear one
        state.clear_cancellation("wf2").await;
        assert!(state.is_cancelled("wf1").await);
        assert!(!state.is_cancelled("wf2").await); // Removed from map
        assert!(state.is_cancelled("wf3").await);
    }

    #[tokio::test]
    async fn test_cancellation_token_works_with_select() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
        state.request_cancellation(workflow_id).await;

        // The task should complete quickly now
        let result = tokio::time::timeout(tokio::time::Duration::from_millis(100), handle).await;

        assert!(result.is_ok(), "Task should complete after cancellation");
        assert_eq!(result.unwrap().unwrap(), "cancelled");
    }
}
