// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use async_trait::async_trait;
use crate::agents::core::agent::{Agent, Task, Report, ReportStatus, ReportMetrics};
use crate::models::{AgentConfig, Lifecycle};

/// Simple agent implementation for demonstration (base implementation)
pub struct SimpleAgent {
    config: AgentConfig,
}

impl SimpleAgent {
    /// Creates a new simple agent
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Agent for SimpleAgent {
    async fn execute(&self, task: Task) -> anyhow::Result<Report> {
        let start = std::time::Instant::now();

        // Basic task processing simulation (no LLM call yet)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let content = format!(
            "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Results\nTask completed successfully (base implementation).\n\n## Context\n```json\n{}\n```",
            self.config.id,
            task.description,
            serde_json::to_string_pretty(&task.context)?
        );

        let report = Report {
            task_id: task.id,
            status: ReportStatus::Success,
            content,
            metrics: ReportMetrics {
                duration_ms: start.elapsed().as_millis() as u64,
                tokens_input: 0,
                tokens_output: 0,
                tools_used: vec![],
                mcp_calls: vec![],
            },
        };

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
