// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;
use super::{agent::{Task, Report}, registry::AgentRegistry};

/// Agent orchestrator for coordinating agent execution
pub struct AgentOrchestrator {
    registry: Arc<AgentRegistry>,
}

impl AgentOrchestrator {
    /// Creates a new orchestrator
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self { registry }
    }

    /// Executes a task via a specific agent
    pub async fn execute(&self, agent_id: &str, task: Task) -> anyhow::Result<Report> {
        let agent = self
            .registry
            .get(agent_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;

        tracing::info!("Executing task {} with agent {}", task.id, agent_id);

        let report = agent.execute(task).await?;

        tracing::info!(
            "Task completed - Status: {:?}, Duration: {}ms",
            report.status,
            report.metrics.duration_ms
        );

        Ok(report)
    }

    /// Executes multiple tasks in parallel (if independent) - prepared for future phases
    #[allow(dead_code)]
    pub async fn execute_parallel(
        &self,
        tasks: Vec<(String, Task)>, // Vec<(agent_id, task)>
    ) -> Vec<anyhow::Result<Report>> {
        use futures::future::join_all;

        let futures = tasks
            .into_iter()
            .map(|(agent_id, task)| {
                let registry = self.registry.clone();
                async move {
                    let agent = registry
                        .get(&agent_id)
                        .await
                        .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;

                    agent.execute(task).await
                }
            });

        join_all(futures).await
    }
}
