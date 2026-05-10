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

//! LLM Agent - Agent that uses real LLM calls via ProviderManager
//!
//! This agent supports tool execution integration, allowing it to call both
//! local tools (MemoryTool, TodoTool) and MCP tools during workflow execution.
//!
//! # Architecture
//!
//! The implementation is split across modules:
//! - [`super::prompt`] - Prompt building (user prompt, system prompt, report detection)
//! - [`super::execution::tools`] - Tool creation, collection, and execution
//! - [`super::execution::tool_loop`] - Main execution loops (simple and tool-augmented)

use crate::agents::core::agent::{Agent, Report, Task};
use crate::agents::execution::tool_loop::{self, ToolLoopContext};
use crate::llm::ProviderManager;
use crate::mcp::MCPManager;
use crate::models::{AgentConfig, Lifecycle};
use crate::tools::{context::AgentToolContext, ToolFactory};
use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::instrument;

/// Agent that uses real LLM calls via the ProviderManager
pub struct LLMAgent {
    /// Agent configuration
    config: AgentConfig,
    /// LLM provider manager
    provider_manager: Arc<ProviderManager>,
    /// Tool factory for creating local tool instances
    tool_factory: Option<Arc<ToolFactory>>,
    /// Agent tool context for sub-agent operations (only for primary agents)
    agent_context: Option<AgentToolContext>,
}

impl LLMAgent {
    /// Creates a new LLM agent with an AgentToolContext.
    ///
    /// The context carries shared dependencies (registry, orchestrator,
    /// llm_manager, mcp_manager, tool_factory, app_handle, cancellation_token)
    /// down to the tools created on each turn. `app_handle` in particular is
    /// what lets `UserQuestionTool::emit_question_event` reach the frontend.
    ///
    /// # Sub-Agent Tools Availability
    ///
    /// Sub-agent tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool)
    /// are only created when both:
    /// 1. The agent has an AgentToolContext (this constructor provides one)
    /// 2. The task context does NOT set `"is_sub_agent": true`
    ///
    /// Sub-agents spawned via SpawnAgentTool inject `is_sub_agent: true` in
    /// their task context, which forces `is_primary_agent && !is_sub_agent`
    /// to false in `tool_loop.rs`, so they only ever get basic tools — not
    /// other sub-agent tools (single-level hierarchy).
    pub fn with_context(
        config: AgentConfig,
        provider_manager: Arc<ProviderManager>,
        tool_factory: Arc<ToolFactory>,
        agent_context: AgentToolContext,
    ) -> Self {
        Self {
            config,
            provider_manager,
            tool_factory: Some(tool_factory),
            agent_context: Some(agent_context),
        }
    }
}

#[async_trait]
impl Agent for LLMAgent {
    #[instrument(
        name = "llm_agent_execute",
        skip(self, task),
        fields(
            agent_id = %self.config.id,
            task_id = %task.id,
            provider = %self.config.llm.provider,
            model = %self.config.llm.model,
            task_description_len = task.description.len()
        )
    )]
    async fn execute(&self, task: Task) -> anyhow::Result<Report> {
        // Trait `Agent::execute` does not surface a CancellationToken.
        // Cancellable execution flows through `execute_with_mcp`, which
        // threads the token down into `execute_simple` when no tools apply.
        tool_loop::execute_simple(
            &self.config,
            &self.provider_manager,
            self.agent_context.as_ref(),
            task,
            None,
        )
        .await
    }

    #[instrument(
        name = "llm_agent_execute_with_mcp",
        skip(self, task, mcp_manager, cancellation_token),
        fields(
            agent_id = %self.config.id,
            task_id = %task.id,
            provider = %self.config.llm.provider,
            model = %self.config.llm.model,
            has_mcp = mcp_manager.is_some(),
            local_tools_count = self.config.tools.len(),
            mcp_servers_count = self.config.mcp_servers.len()
        )
    )]
    async fn execute_with_mcp(
        &self,
        task: Task,
        mcp_manager: Option<Arc<MCPManager>>,
        cancellation_token: Option<CancellationToken>,
    ) -> anyhow::Result<Report> {
        tool_loop::execute_with_tools(
            ToolLoopContext {
                config: &self.config,
                provider_manager: &self.provider_manager,
                tool_factory: self.tool_factory.as_ref(),
                agent_context: self.agent_context.as_ref(),
            },
            task,
            mcp_manager,
            cancellation_token,
        )
        .await
    }

    fn capabilities(&self) -> Vec<String> {
        vec![
            "llm_completion".to_string(),
            format!("provider:{}", self.config.llm.provider),
            format!("model:{}", self.config.llm.model),
        ]
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

    fn config(&self) -> &AgentConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::agent::ReportStatus;
    use crate::models::LLMConfig;

    fn create_test_agent() -> LLMAgent {
        let config = AgentConfig {
            id: "test_llm_agent".to_string(),
            name: "Test LLM Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Ollama".to_string(),
                model: "llama3.2".to_string(),
                temperature: 0.7,
                max_tokens: 2000,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "You are a helpful assistant.".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
        };
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        LLMAgent {
            config,
            provider_manager: manager,
            tool_factory: None,
            agent_context: None,
        }
    }

    #[tokio::test]
    async fn test_execute_returns_failed_when_provider_not_configured() {
        let agent = create_test_agent();

        let task = Task {
            id: "task_test".to_string(),
            description: "Test prompt".to_string(),
            context: serde_json::json!({}),
        };

        let report = agent.execute(task).await.expect("execute should not error");
        assert!(matches!(report.status, ReportStatus::Failed));
        assert!(report.content.contains("not configured"));
    }
}
