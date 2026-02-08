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

use crate::agents::core::agent::{Agent, Report, ReportMetrics, ReportStatus, Task};
use crate::models::{AgentConfig, Lifecycle};
use async_trait::async_trait;
use tracing::{debug, info, instrument};

/// Simple agent implementation for demonstration (base implementation)
///
/// Note: This agent is primarily used for testing purposes.
/// In production, agents are created via Settings UI and stored in SurrealDB.
#[allow(dead_code)]
pub struct SimpleAgent {
    config: AgentConfig,
}

#[allow(dead_code)]
impl SimpleAgent {
    /// Creates a new simple agent
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Agent for SimpleAgent {
    #[instrument(
        name = "simple_agent_execute",
        skip(self, task),
        fields(
            agent_id = %self.config.id,
            task_id = %task.id,
            task_description_len = task.description.len()
        )
    )]
    async fn execute(&self, task: Task) -> anyhow::Result<Report> {
        let start = std::time::Instant::now();

        debug!(
            agent_name = %self.config.name,
            system_prompt_len = self.config.system_prompt.len(),
            tools_count = self.config.tools.len(),
            mcp_servers_count = self.config.mcp_servers.len(),
            "Agent starting task execution"
        );

        // Basic task processing simulation (no LLM call yet)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let content = format!(
            "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Results\nTask completed successfully (base implementation).\n\n## Context\n```json\n{}\n```",
            self.config.id,
            task.description,
            serde_json::to_string_pretty(&task.context)?
        );

        let duration_ms = start.elapsed().as_millis() as u64;

        let report = Report {
            task_id: task.id.clone(),
            status: ReportStatus::Success,
            response: task.description.clone(),
            content,
            metrics: ReportMetrics {
                duration_ms,
                tokens_input: 0,
                tokens_output: 0,
                tools_used: vec![],
                mcp_calls: vec![],
                tool_executions: vec![],
                reasoning_steps: vec![],
            },
            system_prompt: None,
            tools_json: None,
        };

        info!(
            duration_ms = duration_ms,
            report_len = report.content.len(),
            "Agent task execution completed"
        );

        Ok(report)
    }

    fn capabilities(&self) -> Vec<String> {
        vec!["basic_execution".to_string()]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::agent::Agent;
    use crate::models::LLMConfig;

    fn create_test_config() -> AgentConfig {
        AgentConfig {
            id: "test_simple_agent".to_string(),
            name: "Test Simple Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Demo".to_string(),
                model: "simple".to_string(),
                temperature: 0.7,
                max_tokens: 2000,
            },
            tools: vec!["tool1".to_string(), "tool2".to_string()],
            mcp_servers: vec!["mcp_server1".to_string()],
            system_prompt: "You are a test agent.".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        }
    }

    #[test]
    fn test_simple_agent_new() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config.clone());

        assert_eq!(agent.config().id, "test_simple_agent");
        assert_eq!(agent.config().name, "Test Simple Agent");
    }

    #[test]
    fn test_simple_agent_capabilities() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let capabilities = agent.capabilities();
        assert_eq!(capabilities, vec!["basic_execution".to_string()]);
    }

    #[test]
    fn test_simple_agent_lifecycle() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        assert!(matches!(agent.lifecycle(), Lifecycle::Permanent));
    }

    #[test]
    fn test_simple_agent_lifecycle_temporary() {
        let mut config = create_test_config();
        config.lifecycle = Lifecycle::Temporary;
        let agent = SimpleAgent::new(config);

        assert!(matches!(agent.lifecycle(), Lifecycle::Temporary));
    }

    #[test]
    fn test_simple_agent_tools() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let tools = agent.tools();
        assert_eq!(tools, vec!["tool1".to_string(), "tool2".to_string()]);
    }

    #[test]
    fn test_simple_agent_mcp_servers() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let mcp_servers = agent.mcp_servers();
        assert_eq!(mcp_servers, vec!["mcp_server1".to_string()]);
    }

    #[test]
    fn test_simple_agent_system_prompt() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        assert_eq!(agent.system_prompt(), "You are a test agent.");
    }

    #[tokio::test]
    async fn test_simple_agent_execute() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let task = Task {
            id: "task_123".to_string(),
            description: "Test task description".to_string(),
            context: serde_json::json!({"key": "value"}),
        };

        let result = agent.execute(task).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(matches!(report.status, ReportStatus::Success));
        assert!(report.content.contains("test_simple_agent"));
        assert!(report.content.contains("Test task description"));
        assert!(report.content.contains("key"));
        assert!(report.content.contains("value"));
        assert!(report.metrics.duration_ms >= 100);
    }

    #[tokio::test]
    async fn test_simple_agent_execute_with_empty_context() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let task = Task {
            id: "task_empty".to_string(),
            description: "Empty context task".to_string(),
            context: serde_json::json!({}),
        };

        let result = agent.execute(task).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        assert!(matches!(report.status, ReportStatus::Success));
        assert!(report.content.contains("Empty context task"));
    }

    #[tokio::test]
    async fn test_simple_agent_report_format() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let task = Task {
            id: "task_format".to_string(),
            description: "Format test".to_string(),
            context: serde_json::json!({}),
        };

        let result = agent.execute(task).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        // Verify markdown format
        assert!(report.content.starts_with("# Agent Report:"));
        assert!(report.content.contains("**Task**:"));
        assert!(report.content.contains("**Status**: Success"));
        assert!(report.content.contains("## Results"));
        assert!(report.content.contains("## Context"));
    }

    #[tokio::test]
    async fn test_simple_agent_metrics() {
        let config = create_test_config();
        let agent = SimpleAgent::new(config);

        let task = Task {
            id: "task_metrics".to_string(),
            description: "Metrics test".to_string(),
            context: serde_json::json!({}),
        };

        let result = agent.execute(task).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        // Base implementation has no LLM calls
        assert_eq!(report.metrics.tokens_input, 0);
        assert_eq!(report.metrics.tokens_output, 0);
        assert!(report.metrics.tools_used.is_empty());
        assert!(report.metrics.mcp_calls.is_empty());
    }
}
