// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use super::agent::Agent;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent registry for discovering and managing agents
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<String, Arc<dyn Agent>>>>,
}

impl AgentRegistry {
    /// Creates a new agent registry
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers an agent (permanent or temporary)
    pub async fn register(&self, id: String, agent: Arc<dyn Agent>) {
        let mut agents = self.agents.write().await;
        agents.insert(id.clone(), agent);
        tracing::info!("Agent registered: {}", id);
    }

    /// Retrieves an agent by ID
    pub async fn get(&self, id: &str) -> Option<Arc<dyn Agent>> {
        let agents = self.agents.read().await;
        agents.get(id).cloned()
    }

    /// Lists all agent IDs
    pub async fn list(&self) -> Vec<String> {
        let agents = self.agents.read().await;
        agents.keys().cloned().collect()
    }

    /// Unregisters an agent (temporary only) - prepared for future phases
    #[allow(dead_code)]
    pub async fn unregister(&self, id: &str) -> anyhow::Result<()> {
        let mut agents = self.agents.write().await;

        if let Some(agent) = agents.get(id) {
            use crate::models::Lifecycle;
            if matches!(agent.lifecycle(), Lifecycle::Temporary) {
                agents.remove(id);
                tracing::info!("Agent unregistered: {}", id);
                Ok(())
            } else {
                anyhow::bail!("Cannot unregister permanent agent: {}", id)
            }
        } else {
            anyhow::bail!("Agent not found: {}", id)
        }
    }

    /// Cleans up temporary agents - prepared for future phases
    #[allow(dead_code)]
    pub async fn cleanup_temporary(&self) {
        let mut agents = self.agents.write().await;
        use crate::models::Lifecycle;

        agents.retain(|id, agent| {
            let is_permanent = matches!(agent.lifecycle(), Lifecycle::Permanent);
            if !is_permanent {
                tracing::info!("Cleaning up temporary agent: {}", id);
            }
            is_permanent
        });
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::agent::{Agent, Report, ReportMetrics, ReportStatus, Task};
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};
    use async_trait::async_trait;

    /// Test agent implementation for unit tests
    struct TestAgent {
        config: AgentConfig,
    }

    impl TestAgent {
        fn new(id: &str, lifecycle: Lifecycle) -> Self {
            Self {
                config: AgentConfig {
                    id: id.to_string(),
                    name: format!("Test Agent {}", id),
                    lifecycle,
                    llm: LLMConfig {
                        provider: "Test".to_string(),
                        model: "test-model".to_string(),
                        temperature: 0.7,
                        max_tokens: 100,
                    },
                    tools: vec![],
                    mcp_servers: vec![],
                    system_prompt: "Test prompt".to_string(),
                },
            }
        }
    }

    #[async_trait]
    impl Agent for TestAgent {
        async fn execute(&self, task: Task) -> anyhow::Result<Report> {
            Ok(Report {
                task_id: task.id,
                status: ReportStatus::Success,
                content: "Test report".to_string(),
                metrics: ReportMetrics {
                    duration_ms: 10,
                    tokens_input: 0,
                    tokens_output: 0,
                    tools_used: vec![],
                    mcp_calls: vec![],
                },
            })
        }

        fn capabilities(&self) -> Vec<String> {
            vec!["test".to_string()]
        }

        fn lifecycle(&self) -> Lifecycle {
            self.config.lifecycle.clone()
        }

        fn tools(&self) -> Vec<String> {
            self.config.tools.clone()
        }

        fn mcp_servers(&self) -> Vec<String> {
            self.config.mcp_servers.clone()
        }

        fn system_prompt(&self) -> String {
            self.config.system_prompt.clone()
        }

        fn config(&self) -> &AgentConfig {
            &self.config
        }
    }

    #[tokio::test]
    async fn test_registry_new() {
        let registry = AgentRegistry::new();
        let agents = registry.list().await;
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(TestAgent::new("test_agent", Lifecycle::Permanent));

        registry.register("test_agent".to_string(), agent).await;

        let retrieved = registry.get("test_agent").await;
        assert!(retrieved.is_some());

        let retrieved_agent = retrieved.unwrap();
        assert_eq!(retrieved_agent.config().id, "test_agent");
    }

    #[tokio::test]
    async fn test_registry_get_nonexistent() {
        let registry = AgentRegistry::new();
        let result = registry.get("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_registry_list() {
        let registry = AgentRegistry::new();

        let agent1 = Arc::new(TestAgent::new("agent1", Lifecycle::Permanent));
        let agent2 = Arc::new(TestAgent::new("agent2", Lifecycle::Temporary));

        registry.register("agent1".to_string(), agent1).await;
        registry.register("agent2".to_string(), agent2).await;

        let mut agents = registry.list().await;
        agents.sort();

        assert_eq!(agents.len(), 2);
        assert_eq!(agents, vec!["agent1", "agent2"]);
    }

    #[tokio::test]
    async fn test_registry_unregister_temporary() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(TestAgent::new("temp_agent", Lifecycle::Temporary));

        registry.register("temp_agent".to_string(), agent).await;
        assert!(registry.get("temp_agent").await.is_some());

        let result = registry.unregister("temp_agent").await;
        assert!(result.is_ok());
        assert!(registry.get("temp_agent").await.is_none());
    }

    #[tokio::test]
    async fn test_registry_unregister_permanent_fails() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(TestAgent::new("perm_agent", Lifecycle::Permanent));

        registry.register("perm_agent".to_string(), agent).await;

        let result = registry.unregister("perm_agent").await;
        assert!(result.is_err());
        assert!(registry.get("perm_agent").await.is_some());
    }

    #[tokio::test]
    async fn test_registry_unregister_nonexistent_fails() {
        let registry = AgentRegistry::new();
        let result = registry.unregister("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_cleanup_temporary() {
        let registry = AgentRegistry::new();

        let perm_agent = Arc::new(TestAgent::new("perm", Lifecycle::Permanent));
        let temp_agent1 = Arc::new(TestAgent::new("temp1", Lifecycle::Temporary));
        let temp_agent2 = Arc::new(TestAgent::new("temp2", Lifecycle::Temporary));

        registry.register("perm".to_string(), perm_agent).await;
        registry.register("temp1".to_string(), temp_agent1).await;
        registry.register("temp2".to_string(), temp_agent2).await;

        assert_eq!(registry.list().await.len(), 3);

        registry.cleanup_temporary().await;

        let remaining = registry.list().await;
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0], "perm");
    }

    #[tokio::test]
    async fn test_registry_default() {
        let registry = AgentRegistry::default();
        assert!(registry.list().await.is_empty());
    }
}
