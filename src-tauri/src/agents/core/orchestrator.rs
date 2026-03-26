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

use super::{
    agent::{Report, Task},
    registry::AgentRegistry,
};
use crate::mcp::MCPManager;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn};

/// Agent orchestrator for coordinating agent execution
pub struct AgentOrchestrator {
    registry: Arc<AgentRegistry>,
}

impl AgentOrchestrator {
    /// Creates a new orchestrator
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self { registry }
    }

    /// Executes a task via a specific agent with MCP tool support
    ///
    /// # Arguments
    /// * `agent_id` - ID of the agent to execute the task
    /// * `task` - The task to execute
    /// * `mcp_manager` - Optional MCP manager for tool invocation
    /// * `cancellation_token` - Optional cancellation token for graceful shutdown
    #[instrument(
        name = "orchestrator_execute_with_mcp",
        skip(self, task, mcp_manager, cancellation_token),
        fields(
            task_id = %task.id,
            agent_id = %agent_id,
            task_description_len = task.description.len(),
            has_mcp = mcp_manager.is_some()
        )
    )]
    pub async fn execute_with_mcp(
        &self,
        agent_id: &str,
        task: Task,
        mcp_manager: Option<Arc<MCPManager>>,
        cancellation_token: Option<CancellationToken>,
    ) -> anyhow::Result<Report> {
        debug!("Looking up agent in registry");

        let agent = self.registry.get(agent_id).await.ok_or_else(|| {
            warn!(agent_id = %agent_id, "Agent not found in registry");
            anyhow::anyhow!("Agent not found: {}", agent_id)
        })?;

        info!(
            agent_lifecycle = ?agent.lifecycle(),
            capabilities = ?agent.capabilities(),
            mcp_servers = ?agent.mcp_servers(),
            has_mcp_manager = mcp_manager.is_some(),
            "Starting agent execution with MCP support"
        );

        let report = agent
            .execute_with_mcp(task, mcp_manager, cancellation_token)
            .await
            .map_err(|e| {
                error!(error = %e, "Agent execution failed");
                e
            })?;

        info!(
            status = ?report.status,
            duration_ms = report.metrics.duration_ms,
            tokens_input = report.metrics.tokens_input,
            tokens_output = report.metrics.tokens_output,
            tools_used = ?report.metrics.tools_used,
            mcp_calls = ?report.metrics.mcp_calls,
            "Agent execution completed"
        );

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::agent::{Agent, Report, ReportMetrics, ReportStatus};
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};
    use async_trait::async_trait;

    /// Test agent for orchestrator tests
    struct OrchestratorTestAgent {
        config: AgentConfig,
        delay_ms: u64,
    }

    impl OrchestratorTestAgent {
        fn new(id: &str, delay_ms: u64) -> Self {
            Self {
                config: AgentConfig {
                    id: id.to_string(),
                    name: format!("Orchestrator Test Agent {}", id),
                    lifecycle: Lifecycle::Permanent,
                    llm: LLMConfig {
                        provider: "Test".to_string(),
                        model: "test-model".to_string(),
                        temperature: 0.7,
                        max_tokens: 100,
                        is_reasoning: false,
                        context_window: None,
                    },
                    tools: vec![],
                    mcp_servers: vec![],
                    skills: vec![],
                    folders: vec![],
                    require_file_confirmation: true,
                    system_prompt: "Test prompt".to_string(),
                    max_tool_iterations: 50,
                    reasoning_effort: None,
                },
                delay_ms,
            }
        }
    }

    #[async_trait]
    impl Agent for OrchestratorTestAgent {
        async fn execute(&self, task: Task) -> anyhow::Result<Report> {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.delay_ms)).await;

            Ok(Report {
                status: ReportStatus::Success,
                content: format!("Report from agent {}: {}", self.config.id, task.description),
                response: format!("Report from agent {}: {}", self.config.id, task.description),
                metrics: ReportMetrics {
                    duration_ms: self.delay_ms,
                    tokens_input: 0,
                    tokens_output: 0,
                    context_tokens: 0,
                    cached_tokens: None,
                    cache_write_tokens: None,
                    thinking_tokens: None,
                    tools_used: vec![],
                    mcp_calls: vec![],
                    tool_executions: vec![],
                    reasoning_steps: vec![],
                    iteration_metrics: vec![],
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

    /// Test agent that always fails
    struct FailingTestAgent {
        config: AgentConfig,
    }

    impl FailingTestAgent {
        fn new(id: &str) -> Self {
            Self {
                config: AgentConfig {
                    id: id.to_string(),
                    name: format!("Failing Test Agent {}", id),
                    lifecycle: Lifecycle::Permanent,
                    llm: LLMConfig {
                        provider: "Test".to_string(),
                        model: "test-model".to_string(),
                        temperature: 0.7,
                        max_tokens: 100,
                        is_reasoning: false,
                        context_window: None,
                    },
                    tools: vec![],
                    mcp_servers: vec![],
                    skills: vec![],
                    folders: vec![],
                    require_file_confirmation: true,
                    system_prompt: "Test prompt".to_string(),
                    max_tool_iterations: 50,
                    reasoning_effort: None,
                },
            }
        }
    }

    #[async_trait]
    impl Agent for FailingTestAgent {
        async fn execute(&self, _task: Task) -> anyhow::Result<Report> {
            anyhow::bail!("Intentional test failure")
        }

        fn capabilities(&self) -> Vec<String> {
            vec!["fail".to_string()]
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
    async fn test_orchestrator_execute_single() {
        let registry = Arc::new(AgentRegistry::new());
        let agent = Arc::new(OrchestratorTestAgent::new("test_agent", 10));

        registry.register("test_agent".to_string(), agent).await;

        let orchestrator = AgentOrchestrator::new(registry);

        let task = Task {
            id: "task_1".to_string(),
            description: "Test task".to_string(),
            context: serde_json::json!({}),
        };

        let report = orchestrator
            .execute_with_mcp("test_agent", task, None, None)
            .await;
        assert!(report.is_ok());

        let report = report.unwrap();
        assert!(matches!(report.status, ReportStatus::Success));
        assert!(report.content.contains("test_agent"));
    }

    #[tokio::test]
    async fn test_orchestrator_execute_nonexistent_agent() {
        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = AgentOrchestrator::new(registry);

        let task = Task {
            id: "task_1".to_string(),
            description: "Test task".to_string(),
            context: serde_json::json!({}),
        };

        let result = orchestrator
            .execute_with_mcp("nonexistent", task, None, None)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Agent not found"));
    }

    #[tokio::test]
    async fn test_orchestrator_execute_failing_agent() {
        let registry = Arc::new(AgentRegistry::new());
        let agent = Arc::new(FailingTestAgent::new("failing_agent"));

        registry.register("failing_agent".to_string(), agent).await;

        let orchestrator = AgentOrchestrator::new(registry);

        let task = Task {
            id: "task_1".to_string(),
            description: "This should fail".to_string(),
            context: serde_json::json!({}),
        };

        let result = orchestrator
            .execute_with_mcp("failing_agent", task, None, None)
            .await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Intentional test failure"));
    }
}
