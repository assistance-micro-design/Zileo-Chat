// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::agents::core::{AgentOrchestrator, AgentRegistry};
use crate::db::DBClient;
use std::sync::Arc;

/// Application state shared across Tauri commands
pub struct AppState {
    /// Database client
    pub db: Arc<DBClient>,
    /// Agent registry
    pub registry: Arc<AgentRegistry>,
    /// Agent orchestrator
    pub orchestrator: Arc<AgentOrchestrator>,
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

        Ok(Self {
            db,
            registry,
            orchestrator,
        })
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

        // Strong count should be 2 for each
        assert_eq!(Arc::strong_count(&state.db), 2);
        assert_eq!(Arc::strong_count(&state.registry), 3); // registry + orchestrator + clone
        assert_eq!(Arc::strong_count(&state.orchestrator), 2);

        drop(db_clone);
        drop(registry_clone);
        drop(orchestrator_clone);

        // Back to original counts
        assert_eq!(Arc::strong_count(&state.db), 1);
    }
}
