// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! LLM Agent - Agent that uses real LLM calls via ProviderManager

use crate::agents::core::agent::{Agent, Report, ReportMetrics, ReportStatus, Task};
use crate::llm::{LLMError, ProviderManager, ProviderType};
use crate::models::{AgentConfig, Lifecycle};
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{debug, error, info, instrument, warn};

/// Agent that uses real LLM calls via the ProviderManager
pub struct LLMAgent {
    /// Agent configuration
    config: AgentConfig,
    /// LLM provider manager
    provider_manager: Arc<ProviderManager>,
}

impl LLMAgent {
    /// Creates a new LLM agent
    ///
    /// # Arguments
    /// * `config` - Agent configuration including LLM settings
    /// * `provider_manager` - Shared provider manager for LLM calls
    pub fn new(config: AgentConfig, provider_manager: Arc<ProviderManager>) -> Self {
        Self {
            config,
            provider_manager,
        }
    }

    /// Gets the provider type from config
    fn get_provider_type(&self) -> Result<ProviderType, LLMError> {
        self.config.llm.provider.parse()
    }

    /// Builds the full prompt with system context
    fn build_prompt(&self, task: &Task) -> String {
        let context_str = if task.context.is_null() || task.context == serde_json::json!({}) {
            String::new()
        } else {
            format!(
                "\n\nContext:\n```json\n{}\n```",
                serde_json::to_string_pretty(&task.context).unwrap_or_default()
            )
        };

        format!("{}{}", task.description, context_str)
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
        let start = std::time::Instant::now();

        debug!(
            agent_name = %self.config.name,
            system_prompt_len = self.config.system_prompt.len(),
            tools_count = self.config.tools.len(),
            mcp_servers_count = self.config.mcp_servers.len(),
            "LLM Agent starting task execution"
        );

        // Build prompt
        let prompt = self.build_prompt(&task);

        // Get provider type from config
        let provider_type = match self.get_provider_type() {
            Ok(pt) => pt,
            Err(e) => {
                error!(error = %e, "Invalid provider type in config");
                return Ok(Report {
                    task_id: task.id.clone(),
                    status: ReportStatus::Failed,
                    content: format!(
                        "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\nInvalid provider configuration: {}",
                        self.config.id, task.description, e
                    ),
                    metrics: ReportMetrics {
                        duration_ms: start.elapsed().as_millis() as u64,
                        tokens_input: 0,
                        tokens_output: 0,
                        tools_used: vec![],
                        mcp_calls: vec![],
                    },
                });
            }
        };

        // Check if provider is configured
        if !self.provider_manager.is_provider_configured(provider_type) {
            warn!(
                ?provider_type,
                "Provider not configured, returning configuration error"
            );
            return Ok(Report {
                task_id: task.id.clone(),
                status: ReportStatus::Failed,
                content: format!(
                    "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\nLLM provider '{}' is not configured. Please configure it in Settings.",
                    self.config.id, task.description, provider_type
                ),
                metrics: ReportMetrics {
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_input: 0,
                    tokens_output: 0,
                    tools_used: vec![],
                    mcp_calls: vec![],
                },
            });
        }

        // Execute LLM call
        let llm_result = self
            .provider_manager
            .complete_with_provider(
                provider_type,
                &prompt,
                Some(&self.config.system_prompt),
                Some(&self.config.llm.model),
                self.config.llm.temperature,
                self.config.llm.max_tokens,
            )
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match llm_result {
            Ok(response) => {
                info!(
                    tokens_input = response.tokens_input,
                    tokens_output = response.tokens_output,
                    model = %response.model,
                    provider = ?response.provider,
                    duration_ms = duration_ms,
                    "LLM Agent task execution completed successfully"
                );

                let content = format!(
                    "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Response\n\n{}\n\n## Metrics\n- Provider: {}\n- Model: {}\n- Tokens (input/output): {}/{}\n- Duration: {}ms",
                    self.config.id,
                    task.description,
                    response.content,
                    response.provider,
                    response.model,
                    response.tokens_input,
                    response.tokens_output,
                    duration_ms
                );

                Ok(Report {
                    task_id: task.id,
                    status: ReportStatus::Success,
                    content,
                    metrics: ReportMetrics {
                        duration_ms,
                        tokens_input: response.tokens_input,
                        tokens_output: response.tokens_output,
                        tools_used: vec![],
                        mcp_calls: vec![],
                    },
                })
            }
            Err(e) => {
                error!(error = %e, "LLM call failed");

                let error_message = match &e {
                    LLMError::ConnectionError(msg) => {
                        format!("Connection error: {}\n\nMake sure the LLM service is running and accessible.", msg)
                    }
                    LLMError::ModelNotFound(msg) => {
                        format!("Model not found: {}", msg)
                    }
                    LLMError::MissingApiKey(provider) => {
                        format!(
                            "API key missing for {}. Please configure it in Settings.",
                            provider
                        )
                    }
                    LLMError::RequestFailed(msg) => {
                        format!("Request failed: {}", msg)
                    }
                    _ => e.to_string(),
                };

                let content = format!(
                    "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\n\n{}",
                    self.config.id, task.description, error_message
                );

                Ok(Report {
                    task_id: task.id,
                    status: ReportStatus::Failed,
                    content,
                    metrics: ReportMetrics {
                        duration_ms,
                        tokens_input: 0,
                        tokens_output: 0,
                        tools_used: vec![],
                        mcp_calls: vec![],
                    },
                })
            }
        }
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
    use crate::models::LLMConfig;

    fn create_test_config() -> AgentConfig {
        AgentConfig {
            id: "test_llm_agent".to_string(),
            name: "Test LLM Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Ollama".to_string(),
                model: "llama3.2".to_string(),
                temperature: 0.7,
                max_tokens: 2000,
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
            system_prompt: "You are a helpful assistant.".to_string(),
        }
    }

    #[test]
    fn test_llm_agent_new() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config.clone(), manager);

        assert_eq!(agent.config().id, "test_llm_agent");
        assert_eq!(agent.config().llm.provider, "Ollama");
    }

    #[test]
    fn test_llm_agent_capabilities() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        let capabilities = agent.capabilities();
        assert!(capabilities.contains(&"llm_completion".to_string()));
        assert!(capabilities.contains(&"provider:Ollama".to_string()));
        assert!(capabilities.contains(&"model:llama3.2".to_string()));
    }

    #[test]
    fn test_llm_agent_lifecycle() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        assert!(matches!(agent.lifecycle(), Lifecycle::Permanent));
    }

    #[test]
    fn test_llm_agent_get_provider_type() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        let provider = agent.get_provider_type().unwrap();
        assert_eq!(provider, ProviderType::Ollama);
    }

    #[test]
    fn test_llm_agent_get_provider_type_mistral() {
        let mut config = create_test_config();
        config.llm.provider = "Mistral".to_string();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        let provider = agent.get_provider_type().unwrap();
        assert_eq!(provider, ProviderType::Mistral);
    }

    #[test]
    fn test_llm_agent_build_prompt() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        // Test with empty context
        let task = Task {
            id: "task1".to_string(),
            description: "Test task".to_string(),
            context: serde_json::json!({}),
        };
        let prompt = agent.build_prompt(&task);
        assert_eq!(prompt, "Test task");

        // Test with context
        let task_with_context = Task {
            id: "task2".to_string(),
            description: "Analyze this".to_string(),
            context: serde_json::json!({"key": "value"}),
        };
        let prompt_with_context = agent.build_prompt(&task_with_context);
        assert!(prompt_with_context.contains("Analyze this"));
        assert!(prompt_with_context.contains("Context:"));
        assert!(prompt_with_context.contains("key"));
    }

    #[tokio::test]
    async fn test_llm_agent_execute_not_configured() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        let task = Task {
            id: "task_test".to_string(),
            description: "Test prompt".to_string(),
            context: serde_json::json!({}),
        };

        let result = agent.execute(task).await;
        assert!(result.is_ok());

        let report = result.unwrap();
        // Should fail because provider not configured
        assert!(matches!(report.status, ReportStatus::Failed));
        assert!(report.content.contains("not configured"));
    }

    #[test]
    fn test_llm_agent_invalid_provider() {
        let mut config = create_test_config();
        config.llm.provider = "InvalidProvider".to_string();
        let manager = Arc::new(ProviderManager::new());
        let agent = LLMAgent::new(config, manager);

        let result = agent.get_provider_type();
        assert!(result.is_err());
    }
}
