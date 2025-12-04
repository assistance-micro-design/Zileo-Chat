// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use super::{
    agent::{Report, Task},
    registry::AgentRegistry,
};
use crate::mcp::MCPManager;
use std::sync::Arc;
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

    /// Executes a task via a specific agent (legacy, without MCP)
    ///
    /// This method is maintained for backward compatibility with tests.
    /// Production code should use `execute_with_mcp`.
    #[allow(dead_code)]
    #[instrument(
        name = "orchestrator_execute",
        skip(self, task),
        fields(
            task_id = %task.id,
            agent_id = %agent_id,
            task_description_len = task.description.len()
        )
    )]
    pub async fn execute(&self, agent_id: &str, task: Task) -> anyhow::Result<Report> {
        self.execute_with_mcp(agent_id, task, None).await
    }

    /// Executes a task via a specific agent with MCP tool support
    ///
    /// # Arguments
    /// * `agent_id` - ID of the agent to execute the task
    /// * `task` - The task to execute
    /// * `mcp_manager` - Optional MCP manager for tool invocation
    #[instrument(
        name = "orchestrator_execute_with_mcp",
        skip(self, task, mcp_manager),
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
            .execute_with_mcp(task, mcp_manager)
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

    /// Executes multiple tasks in parallel (if independent).
    ///
    /// Used by ParallelTasksTool to run multiple agent tasks concurrently.
    /// All tasks execute using `futures::join_all`, making total time
    /// approximately equal to the slowest individual task.
    ///
    /// # Arguments
    /// * `tasks` - Vector of (agent_id, task) pairs to execute in parallel
    ///
    /// # Returns
    /// Vector of results in the same order as input tasks
    #[instrument(
        name = "orchestrator_execute_parallel",
        skip(self, tasks),
        fields(task_count = tasks.len())
    )]
    pub async fn execute_parallel(
        &self,
        tasks: Vec<(String, Task)>, // Vec<(agent_id, task)>
    ) -> Vec<anyhow::Result<Report>> {
        use futures::future::join_all;

        info!(task_count = tasks.len(), "Starting parallel execution");

        let futures = tasks.into_iter().map(|(agent_id, task)| {
            let registry = self.registry.clone();
            let task_id = task.id.clone();
            async move {
                debug!(task_id = %task_id, agent_id = %agent_id, "Executing parallel task");

                let agent = registry.get(&agent_id).await.ok_or_else(|| {
                    warn!(agent_id = %agent_id, "Agent not found for parallel task");
                    anyhow::anyhow!("Agent not found: {}", agent_id)
                })?;

                agent.execute(task).await
            }
        });

        let results = join_all(futures).await;

        let success_count = results.iter().filter(|r| r.is_ok()).count();
        let failure_count = results.iter().filter(|r| r.is_err()).count();

        info!(
            success_count = success_count,
            failure_count = failure_count,
            "Parallel execution completed"
        );

        results
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
                    },
                    tools: vec![],
                    mcp_servers: vec![],
                    system_prompt: "Test prompt".to_string(),
                    max_tool_iterations: 50,
                    enable_thinking: true,
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
                task_id: task.id.clone(),
                status: ReportStatus::Success,
                content: format!("Report from agent {}: {}", self.config.id, task.description),
                metrics: ReportMetrics {
                    duration_ms: self.delay_ms,
                    tokens_input: 0,
                    tokens_output: 0,
                    tools_used: vec![],
                    mcp_calls: vec![],
                    tool_executions: vec![],
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
                    },
                    tools: vec![],
                    mcp_servers: vec![],
                    system_prompt: "Test prompt".to_string(),
                    max_tool_iterations: 50,
                    enable_thinking: true,
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

        let report = orchestrator.execute("test_agent", task).await;
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

        let result = orchestrator.execute("nonexistent", task).await;
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

        let result = orchestrator.execute("failing_agent", task).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Intentional test failure"));
    }

    #[tokio::test]
    async fn test_orchestrator_execute_parallel() {
        let registry = Arc::new(AgentRegistry::new());

        let agent1 = Arc::new(OrchestratorTestAgent::new("agent_1", 50));
        let agent2 = Arc::new(OrchestratorTestAgent::new("agent_2", 50));

        registry.register("agent_1".to_string(), agent1).await;
        registry.register("agent_2".to_string(), agent2).await;

        let orchestrator = AgentOrchestrator::new(registry);

        let tasks = vec![
            (
                "agent_1".to_string(),
                Task {
                    id: "task_1".to_string(),
                    description: "Task for agent 1".to_string(),
                    context: serde_json::json!({}),
                },
            ),
            (
                "agent_2".to_string(),
                Task {
                    id: "task_2".to_string(),
                    description: "Task for agent 2".to_string(),
                    context: serde_json::json!({}),
                },
            ),
        ];

        let start = std::time::Instant::now();
        let results = orchestrator.execute_parallel(tasks).await;
        let duration = start.elapsed().as_millis();

        // Parallel execution should be faster than sequential (50+50=100ms)
        // Allow some margin for test overhead
        assert!(
            duration < 150,
            "Parallel execution took too long: {}ms",
            duration
        );

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }

    #[tokio::test]
    async fn test_orchestrator_execute_parallel_with_failure() {
        let registry = Arc::new(AgentRegistry::new());

        let good_agent = Arc::new(OrchestratorTestAgent::new("good_agent", 10));
        let bad_agent = Arc::new(FailingTestAgent::new("bad_agent"));

        registry
            .register("good_agent".to_string(), good_agent)
            .await;
        registry.register("bad_agent".to_string(), bad_agent).await;

        let orchestrator = AgentOrchestrator::new(registry);

        let tasks = vec![
            (
                "good_agent".to_string(),
                Task {
                    id: "task_1".to_string(),
                    description: "Good task".to_string(),
                    context: serde_json::json!({}),
                },
            ),
            (
                "bad_agent".to_string(),
                Task {
                    id: "task_2".to_string(),
                    description: "Bad task".to_string(),
                    context: serde_json::json!({}),
                },
            ),
        ];

        let results = orchestrator.execute_parallel(tasks).await;

        assert_eq!(results.len(), 2);
        // One should succeed, one should fail
        let successes = results.iter().filter(|r| r.is_ok()).count();
        let failures = results.iter().filter(|r| r.is_err()).count();

        assert_eq!(successes, 1);
        assert_eq!(failures, 1);
    }

    #[tokio::test]
    async fn test_orchestrator_execute_parallel_empty() {
        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = AgentOrchestrator::new(registry);

        let results = orchestrator.execute_parallel(vec![]).await;
        assert!(results.is_empty());
    }
}
