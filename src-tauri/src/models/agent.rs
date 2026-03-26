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

use serde::{Deserialize, Serialize};

/// Reasoning effort level for thinking models.
///
/// Controls how much reasoning/thinking the model performs.
/// Only effective when the model supports reasoning (is_reasoning = true).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

impl ReasoningEffort {
    /// Returns the string representation matching serde serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            ReasoningEffort::Low => "low",
            ReasoningEffort::Medium => "medium",
            ReasoningEffort::High => "high",
        }
    }
}

/// Agent lifecycle type
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    #[default]
    Permanent,
    Temporary,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Unique identifier
    #[serde(default)]
    pub id: String,
    /// Agent name
    #[serde(default = "default_agent_name")]
    pub name: String,
    /// Lifecycle type
    #[serde(default)]
    pub lifecycle: Lifecycle,
    /// LLM configuration
    #[serde(default = "default_llm_config")]
    pub llm: LLMConfig,
    /// List of available tools
    ///
    /// Valid tool names include:
    /// - `MemoryTool` - Contextual memory with semantic search
    /// - `TodoTool` - Task management for workflow decomposition
    #[serde(default)]
    pub tools: Vec<String>,
    /// MCP server NAMES (not IDs) that the agent can use
    /// Example: ["Serena", "Context7"]
    #[serde(default)]
    pub mcp_servers: Vec<String>,
    /// Skill names assigned to this agent (read via ReadSkillTool)
    /// Example: ["coding-standards", "git-workflow"]
    #[serde(default)]
    pub skills: Vec<String>,
    /// Authorized directory paths for FileManagerTool
    #[serde(default)]
    pub folders: Vec<String>,
    /// Require user confirmation for destructive file operations (default: true)
    #[serde(default = "default_require_file_confirmation")]
    pub require_file_confirmation: bool,
    /// System prompt
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    /// Maximum number of tool execution iterations (1-200, default: 50)
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: usize,
    /// Reasoning effort for thinking models (None = disabled)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Default value for max_tool_iterations
fn default_max_tool_iterations() -> usize {
    50
}

/// Default value for require_file_confirmation
fn default_require_file_confirmation() -> bool {
    true
}

fn default_agent_name() -> String {
    "Unknown".to_string()
}

fn default_system_prompt() -> String {
    "You are a helpful assistant.".to_string()
}

fn default_llm_config() -> LLMConfig {
    LLMConfig {
        provider: default_llm_provider(),
        model: default_llm_model(),
        temperature: default_temperature(),
        max_tokens: default_max_tokens(),
        is_reasoning: false,
        context_window: None,
    }
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Provider name (e.g., "Mistral", "Ollama")
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    /// Model name
    #[serde(default = "default_llm_model")]
    pub model: String,
    /// Sampling temperature
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    /// Maximum tokens to generate
    #[serde(default = "default_max_tokens")]
    pub max_tokens: usize,
    /// Whether the model is a reasoning/thinking model (from DB)
    #[serde(default)]
    pub is_reasoning: bool,
    /// Context window size in tokens (from model config, passed to providers like Ollama as num_ctx)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<usize>,
}

fn default_llm_provider() -> String {
    "Mistral".to_string()
}

fn default_llm_model() -> String {
    "mistral-large-latest".to_string()
}

fn default_temperature() -> f64 {
    0.7
}

fn default_max_tokens() -> usize {
    4096
}

/// Agent configuration for creation (without ID, timestamps)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigCreate {
    /// Agent name (1-64 chars)
    pub name: String,
    /// Lifecycle type
    pub lifecycle: Lifecycle,
    /// LLM configuration
    pub llm: LLMConfig,
    /// List of available tools
    pub tools: Vec<String>,
    /// MCP server NAMES (not IDs) that the agent can use
    pub mcp_servers: Vec<String>,
    /// Skill names assigned to this agent
    #[serde(default)]
    pub skills: Vec<String>,
    /// Authorized directory paths for FileManagerTool
    #[serde(default)]
    pub folders: Vec<String>,
    /// Require user confirmation for destructive file operations (default: true)
    #[serde(default = "default_require_file_confirmation")]
    pub require_file_confirmation: bool,
    /// System prompt (1-10000 chars)
    pub system_prompt: String,
    /// Maximum number of tool execution iterations (1-200, default: 50)
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: usize,
    /// Reasoning effort for thinking models (None = disabled)
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

/// Agent configuration for updates (all fields optional except lifecycle which cannot change)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfigUpdate {
    /// Agent name (1-64 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// LLM configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llm: Option<LLMConfig>,
    /// List of available tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    /// MCP server NAMES (not IDs) that the agent can use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_servers: Option<Vec<String>>,
    /// Skill names assigned to this agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skills: Option<Vec<String>>,
    /// Authorized directory paths for FileManagerTool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folders: Option<Vec<String>>,
    /// Require user confirmation for destructive file operations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_file_confirmation: Option<bool>,
    /// System prompt (1-10000 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Maximum number of tool execution iterations (1-200)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_iterations: Option<usize>,
    /// Reasoning effort for thinking models
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<Option<ReasoningEffort>>,
}

/// Agent summary for listing (lightweight representation)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSummary {
    /// Unique identifier
    pub id: String,
    /// Agent name
    pub name: String,
    /// Lifecycle type
    pub lifecycle: Lifecycle,
    /// LLM provider name
    pub provider: String,
    /// LLM model name
    pub model: String,
    /// Number of enabled tools
    pub tools_count: usize,
    /// Number of configured MCP servers
    pub mcp_servers_count: usize,
    /// Number of assigned skills
    #[serde(default)]
    pub skills_count: usize,
    /// Number of authorized folders
    #[serde(default)]
    pub folders_count: usize,
}

impl From<&AgentConfig> for AgentSummary {
    fn from(config: &AgentConfig) -> Self {
        Self {
            id: config.id.clone(),
            name: config.name.clone(),
            lifecycle: config.lifecycle.clone(),
            provider: config.llm.provider.clone(),
            model: config.llm.model.clone(),
            tools_count: config.tools.len(),
            mcp_servers_count: config.mcp_servers.len(),
            skills_count: config.skills.len(),
            folders_count: config.folders.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::registry::TOOL_REGISTRY;

    impl AgentConfig {
        /// Validates tool names against known tools in the registry.
        pub(crate) fn validate_tools(&self) -> Vec<String> {
            self.tools
                .iter()
                .filter(|t| !TOOL_REGISTRY.has_tool(t))
                .cloned()
                .collect()
        }

        /// Returns true if all configured tools are known.
        pub(crate) fn has_valid_tools(&self) -> bool {
            self.validate_tools().is_empty()
        }
    }

    #[test]
    fn test_reasoning_effort_serialization() {
        let low = ReasoningEffort::Low;
        let json = serde_json::to_string(&low).unwrap();
        assert_eq!(json, "\"low\"");

        let medium: ReasoningEffort = serde_json::from_str("\"medium\"").unwrap();
        assert_eq!(medium, ReasoningEffort::Medium);

        let high: ReasoningEffort = serde_json::from_str("\"high\"").unwrap();
        assert_eq!(high, ReasoningEffort::High);
    }

    #[test]
    fn test_agent_config_with_reasoning_effort() {
        let config = AgentConfig {
            id: "test".to_string(),
            name: "Test".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
                is_reasoning: true,
                context_window: None,
            },
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: Some(ReasoningEffort::Medium),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"reasoning_effort\":\"medium\""));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.reasoning_effort, Some(ReasoningEffort::Medium));
    }

    #[test]
    fn test_agent_config_without_reasoning_effort() {
        let config = AgentConfig {
            id: "test".to_string(),
            name: "Test".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(!json.contains("reasoning_effort"));

        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.reasoning_effort, None);
    }

    #[test]
    fn test_lifecycle_serialization() {
        let lifecycle = Lifecycle::Permanent;
        let json = serde_json::to_string(&lifecycle).unwrap();
        assert_eq!(json, "\"permanent\"");

        let deserialized: Lifecycle = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, Lifecycle::Permanent));
    }

    #[test]
    fn test_lifecycle_temporary() {
        let lifecycle = Lifecycle::Temporary;
        let json = serde_json::to_string(&lifecycle).unwrap();
        assert_eq!(json, "\"temporary\"");

        let deserialized: Lifecycle = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, Lifecycle::Temporary));
    }

    #[test]
    fn test_agent_config_serialization() {
        let config = AgentConfig {
            id: "agent_config_001".to_string(),
            name: "Config Test Agent".to_string(),
            lifecycle: Lifecycle::Temporary,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
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

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, config.id);
        assert_eq!(deserialized.llm.provider, "Mistral");
        assert_eq!(deserialized.llm.model, "mistral-large");
        assert!((deserialized.llm.temperature - 0.7).abs() < f64::EPSILON);
        assert_eq!(deserialized.llm.max_tokens, 4096);
    }

    #[test]
    fn test_llm_config_serialization() {
        let llm_config = LLMConfig {
            provider: "Ollama".to_string(),
            model: "llama3".to_string(),
            temperature: 0.5,
            max_tokens: 2000,
            is_reasoning: false,
            context_window: None,
        };

        let json = serde_json::to_string(&llm_config).unwrap();
        let deserialized: LLMConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.provider, llm_config.provider);
        assert_eq!(deserialized.model, llm_config.model);
        assert!((deserialized.temperature - llm_config.temperature).abs() < f64::EPSILON);
        assert_eq!(deserialized.max_tokens, llm_config.max_tokens);
    }

    #[test]
    fn test_agent_config_validate_tools_valid() {
        let config = AgentConfig {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec!["MemoryTool".to_string(), "TodoTool".to_string()],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
        };

        assert!(config.has_valid_tools());
        assert!(config.validate_tools().is_empty());
    }

    #[test]
    fn test_agent_config_validate_tools_invalid() {
        let config = AgentConfig {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec![
                "MemoryTool".to_string(),
                "InvalidTool".to_string(),
                "AnotherBadTool".to_string(),
            ],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
        };

        assert!(!config.has_valid_tools());
        let invalid = config.validate_tools();
        assert_eq!(invalid.len(), 2);
        assert!(invalid.contains(&"InvalidTool".to_string()));
        assert!(invalid.contains(&"AnotherBadTool".to_string()));
    }

    #[test]
    fn test_agent_config_all_known_tools() {
        let config = AgentConfig {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec![
                "MemoryTool".to_string(),
                "TodoTool".to_string(),
                "CalculatorTool".to_string(),
                "UserQuestionTool".to_string(),
                "SpawnAgentTool".to_string(),
                "DelegateTaskTool".to_string(),
                "ParallelTasksTool".to_string(),
            ],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
        };

        assert!(config.has_valid_tools());
        assert_eq!(config.tools.len(), 7);
    }
}
