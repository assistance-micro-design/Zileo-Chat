// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use super::agent::Agent;

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
