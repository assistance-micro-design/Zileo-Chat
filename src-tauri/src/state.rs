// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    /// Set of workflow IDs that have been requested to cancel
    pub streaming_cancellations: Arc<Mutex<HashSet<String>>>,
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

        // Initialize streaming cancellation tracker
        let streaming_cancellations = Arc::new(Mutex::new(HashSet::new()));

        Ok(Self {
            db,
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            streaming_cancellations,
        })
    }

    /// Checks if a workflow has been requested to cancel
    pub async fn is_cancelled(&self, workflow_id: &str) -> bool {
        self.streaming_cancellations
            .lock()
            .await
            .contains(workflow_id)
    }

    /// Marks a workflow for cancellation
    pub async fn request_cancellation(&self, workflow_id: &str) {
        self.streaming_cancellations
            .lock()
            .await
            .insert(workflow_id.to_string());
    }

    /// Removes a workflow from the cancellation set
    pub async fn clear_cancellation(&self, workflow_id: &str) {
        self.streaming_cancellations
            .lock()
            .await
            .remove(workflow_id);
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
        // and db which is shared with mcp_manager)
        assert_eq!(Arc::strong_count(&state.db), 3); // db + mcp_manager + clone
        assert_eq!(Arc::strong_count(&state.registry), 3); // registry + orchestrator + clone
        assert_eq!(Arc::strong_count(&state.orchestrator), 2);

        drop(db_clone);
        drop(registry_clone);
        drop(orchestrator_clone);

        // Back to original counts
        assert_eq!(Arc::strong_count(&state.db), 2); // db + mcp_manager
    }

    #[tokio::test]
    async fn test_streaming_cancellation() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db5");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();
        let workflow_id = "test_workflow_123";

        // Initially not cancelled
        assert!(
            !state.is_cancelled(workflow_id).await,
            "Workflow should not be cancelled initially"
        );

        // Request cancellation
        state.request_cancellation(workflow_id).await;
        assert!(
            state.is_cancelled(workflow_id).await,
            "Workflow should be cancelled after request"
        );

        // Clear cancellation
        state.clear_cancellation(workflow_id).await;
        assert!(
            !state.is_cancelled(workflow_id).await,
            "Workflow should not be cancelled after clearing"
        );
    }

    #[tokio::test]
    async fn test_multiple_cancellations() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db6");
        let db_path_str = db_path.to_str().unwrap();

        let state = AppState::new(db_path_str).await.unwrap();

        // Cancel multiple workflows
        state.request_cancellation("wf1").await;
        state.request_cancellation("wf2").await;
        state.request_cancellation("wf3").await;

        assert!(state.is_cancelled("wf1").await);
        assert!(state.is_cancelled("wf2").await);
        assert!(state.is_cancelled("wf3").await);
        assert!(!state.is_cancelled("wf4").await);

        // Clear one
        state.clear_cancellation("wf2").await;
        assert!(state.is_cancelled("wf1").await);
        assert!(!state.is_cancelled("wf2").await);
        assert!(state.is_cancelled("wf3").await);
    }
}
