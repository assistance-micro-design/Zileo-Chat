// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{models::AgentConfig, security::Validator, AppState};
use tauri::State;
use tracing::{info, instrument, warn};

/// Lists all available agent IDs
#[tauri::command]
#[instrument(name = "list_agents", skip(state))]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    info!("Listing agents");
    let agent_ids = state.registry.list().await;
    info!(count = agent_ids.len(), "Agents listed");
    Ok(agent_ids)
}

/// Gets agent configuration by ID
#[tauri::command]
#[instrument(name = "get_agent_config", skip(state), fields(agent_id = %agent_id))]
pub async fn get_agent_config(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<AgentConfig, String> {
    info!("Getting agent configuration");

    // Validate input
    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent ID");
        format!("Invalid agent ID: {}", e)
    })?;

    let agent = state
        .registry
        .get(&validated_agent_id)
        .await
        .ok_or_else(|| {
            warn!(agent_id = %validated_agent_id, "Agent not found");
            "Agent not found".to_string()
        })?;

    let config = agent.config().clone();
    info!(
        agent_name = %config.name,
        lifecycle = ?config.lifecycle,
        tools_count = config.tools.len(),
        "Agent configuration retrieved"
    );

    Ok(config)
}

#[cfg(test)]
mod tests {
    use crate::agents::core::{AgentOrchestrator, AgentRegistry};
    use crate::agents::SimpleAgent;
    use crate::db::DBClient;
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};
    use crate::state::AppState;
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Helper to create test AppState with registry
    async fn setup_test_state() -> AppState {
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

        // Leak temp_dir to keep it alive during test
        std::mem::forget(temp_dir);

        AppState {
            db,
            registry,
            orchestrator,
            llm_manager,
            streaming_cancellations: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }

    #[tokio::test]
    async fn test_list_agents_empty() {
        let state = setup_test_state().await;
        let agents = state.registry.list().await;
        assert!(agents.is_empty(), "New registry should be empty");
    }

    #[tokio::test]
    async fn test_list_agents_with_registered() {
        let state = setup_test_state().await;

        // Register agent
        let config = AgentConfig {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Demo".to_string(),
                model: "test".to_string(),
                temperature: 0.7,
                max_tokens: 1000,
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
            system_prompt: "Test".to_string(),
        };

        let agent = SimpleAgent::new(config);
        state
            .registry
            .register("test_agent".to_string(), Arc::new(agent))
            .await;

        let agents = state.registry.list().await;
        assert_eq!(agents.len(), 1);
        assert!(agents.contains(&"test_agent".to_string()));
    }

    #[tokio::test]
    async fn test_get_agent_config_success() {
        let state = setup_test_state().await;

        let config = AgentConfig {
            id: "config_test".to_string(),
            name: "Config Test Agent".to_string(),
            lifecycle: Lifecycle::Temporary,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.5,
                max_tokens: 2000,
            },
            tools: vec!["tool_a".to_string(), "tool_b".to_string()],
            mcp_servers: vec!["serena".to_string()],
            system_prompt: "You are a test agent".to_string(),
        };

        let agent = SimpleAgent::new(config.clone());
        state
            .registry
            .register("config_test".to_string(), Arc::new(agent))
            .await;

        // Get config
        let retrieved_agent = state.registry.get("config_test").await;
        assert!(retrieved_agent.is_some());

        let retrieved_config = retrieved_agent.unwrap().config().clone();
        assert_eq!(retrieved_config.id, "config_test");
        assert_eq!(retrieved_config.name, "Config Test Agent");
        assert_eq!(retrieved_config.llm.provider, "Mistral");
        assert_eq!(retrieved_config.tools.len(), 2);
    }

    #[tokio::test]
    async fn test_get_agent_config_not_found() {
        let state = setup_test_state().await;

        let result = state.registry.get("nonexistent").await;
        assert!(result.is_none(), "Should not find nonexistent agent");
    }

    #[tokio::test]
    async fn test_agent_config_serialization() {
        let config = AgentConfig {
            id: "serial_test".to_string(),
            name: "Serialization Test".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Ollama".to_string(),
                model: "llama3".to_string(),
                temperature: 0.8,
                max_tokens: 4096,
            },
            tools: vec![],
            mcp_servers: vec![],
            system_prompt: "Test prompt".to_string(),
        };

        // Verify JSON serialization
        let json = serde_json::to_string(&config);
        assert!(json.is_ok(), "AgentConfig should serialize to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"serial_test\""));
        assert!(json_str.contains("\"permanent\""));
        assert!(json_str.contains("\"Ollama\""));
    }

    #[tokio::test]
    async fn test_lifecycle_serialization() {
        // Test Lifecycle enum serialization
        assert_eq!(
            serde_json::to_string(&Lifecycle::Permanent).unwrap(),
            "\"permanent\""
        );
        assert_eq!(
            serde_json::to_string(&Lifecycle::Temporary).unwrap(),
            "\"temporary\""
        );
    }

    #[tokio::test]
    async fn test_multiple_agents_listing() {
        let state = setup_test_state().await;

        // Register multiple agents
        for i in 0..5 {
            let config = AgentConfig {
                id: format!("agent_{}", i),
                name: format!("Agent {}", i),
                lifecycle: Lifecycle::Temporary,
                llm: LLMConfig {
                    provider: "Demo".to_string(),
                    model: "test".to_string(),
                    temperature: 0.7,
                    max_tokens: 1000,
                },
                tools: vec![],
                mcp_servers: vec![],
                system_prompt: format!("Agent {} prompt", i),
            };

            let agent = SimpleAgent::new(config);
            state
                .registry
                .register(format!("agent_{}", i), Arc::new(agent))
                .await;
        }

        let agents = state.registry.list().await;
        assert_eq!(agents.len(), 5);
    }
}
