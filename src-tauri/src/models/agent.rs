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

/// Agent lifecycle type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Permanent,
    Temporary,
}

/// Agent status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Available,
    Busy,
}

/// Agent entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Unique identifier
    pub id: String,
    /// Agent name
    pub name: String,
    /// Lifecycle type
    pub lifecycle: Lifecycle,
    /// Current status
    pub status: AgentStatus,
    /// List of capabilities
    pub capabilities: Vec<String>,
    /// List of available tools
    pub tools: Vec<String>,
    /// List of MCP servers used
    pub mcp_servers: Vec<String>,
}

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Unique identifier
    pub id: String,
    /// Agent name
    pub name: String,
    /// Lifecycle type
    pub lifecycle: Lifecycle,
    /// LLM configuration
    pub llm: LLMConfig,
    /// List of available tools
    ///
    /// Valid tool names include:
    /// - `MemoryTool` - Contextual memory with semantic search
    /// - `TodoTool` - Task management for workflow decomposition
    pub tools: Vec<String>,
    /// MCP server NAMES (not IDs) that the agent can use
    /// Example: ["Serena", "Context7"]
    pub mcp_servers: Vec<String>,
    /// System prompt
    pub system_prompt: String,
    /// Maximum number of tool execution iterations (1-200, default: 50)
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: usize,
    /// Enable thinking mode for supported models (default: true for thinking models)
    #[serde(default = "default_enable_thinking")]
    pub enable_thinking: bool,
}

/// Default value for max_tool_iterations
fn default_max_tool_iterations() -> usize {
    50
}

/// Default value for enable_thinking
fn default_enable_thinking() -> bool {
    true
}

// Allow dead code until Phase 6: Full Agent Integration
#[allow(dead_code)]
impl AgentConfig {
    /// Validates tool names against known tools.
    ///
    /// Returns a list of invalid tool names, or empty if all are valid.
    ///
    /// # Known Tools
    /// - `MemoryTool` - Contextual memory with semantic search
    /// - `TodoTool` - Task management for workflow decomposition
    /// - `CalculatorTool` - Scientific calculator for mathematical operations
    /// - `UserQuestionTool` - Ask questions to users via modal interface
    pub fn validate_tools(&self) -> Vec<String> {
        const KNOWN_TOOLS: [&str; 4] = ["MemoryTool", "TodoTool", "CalculatorTool", "UserQuestionTool"];

        self.tools
            .iter()
            .filter(|t| !KNOWN_TOOLS.contains(&t.as_str()))
            .cloned()
            .collect()
    }

    /// Returns true if all configured tools are known.
    pub fn has_valid_tools(&self) -> bool {
        self.validate_tools().is_empty()
    }
}

/// LLM provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfig {
    /// Provider name (e.g., "Mistral", "Ollama")
    pub provider: String,
    /// Model name
    pub model: String,
    /// Sampling temperature
    pub temperature: f32,
    /// Maximum tokens to generate
    pub max_tokens: usize,
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
    /// System prompt (1-10000 chars)
    pub system_prompt: String,
    /// Maximum number of tool execution iterations (1-200, default: 50)
    #[serde(default = "default_max_tool_iterations")]
    pub max_tool_iterations: usize,
    /// Enable thinking mode for supported models (default: true for thinking models)
    #[serde(default = "default_enable_thinking")]
    pub enable_thinking: bool,
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
    /// System prompt (1-10000 chars)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Maximum number of tool execution iterations (1-200)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tool_iterations: Option<usize>,
    /// Enable thinking mode for supported models
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_thinking: Option<bool>,
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
        }
    }
}

/// List of known tools that agents can use
pub const KNOWN_TOOLS: [&str; 4] = ["MemoryTool", "TodoTool", "CalculatorTool", "UserQuestionTool"];

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_agent_status_serialization() {
        let status = AgentStatus::Available;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"available\"");

        let busy = AgentStatus::Busy;
        let busy_json = serde_json::to_string(&busy).unwrap();
        assert_eq!(busy_json, "\"busy\"");
    }

    #[test]
    fn test_agent_serialization() {
        let agent = Agent {
            id: "agent_001".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            status: AgentStatus::Available,
            capabilities: vec!["capability1".to_string(), "capability2".to_string()],
            tools: vec!["tool1".to_string()],
            mcp_servers: vec!["server1".to_string()],
        };

        let json = serde_json::to_string(&agent).unwrap();
        let deserialized: Agent = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, agent.id);
        assert_eq!(deserialized.name, agent.name);
        assert!(matches!(deserialized.lifecycle, Lifecycle::Permanent));
        assert!(matches!(deserialized.status, AgentStatus::Available));
        assert_eq!(deserialized.capabilities.len(), 2);
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
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
            system_prompt: "You are a helpful assistant.".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AgentConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, config.id);
        assert_eq!(deserialized.llm.provider, "Mistral");
        assert_eq!(deserialized.llm.model, "mistral-large");
        assert!((deserialized.llm.temperature - 0.7).abs() < f32::EPSILON);
        assert_eq!(deserialized.llm.max_tokens, 4096);
    }

    #[test]
    fn test_llm_config_serialization() {
        let llm_config = LLMConfig {
            provider: "Ollama".to_string(),
            model: "llama3".to_string(),
            temperature: 0.5,
            max_tokens: 2000,
        };

        let json = serde_json::to_string(&llm_config).unwrap();
        let deserialized: LLMConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.provider, llm_config.provider);
        assert_eq!(deserialized.model, llm_config.model);
        assert!((deserialized.temperature - llm_config.temperature).abs() < f32::EPSILON);
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
            },
            tools: vec!["MemoryTool".to_string(), "TodoTool".to_string()],
            mcp_servers: vec![],
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
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
            },
            tools: vec![
                "MemoryTool".to_string(),
                "InvalidTool".to_string(),
                "AnotherBadTool".to_string(),
            ],
            mcp_servers: vec![],
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
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
            },
            tools: vec![
                "MemoryTool".to_string(),
                "TodoTool".to_string(),
                "CalculatorTool".to_string(),
            ],
            mcp_servers: vec![],
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        };

        assert!(config.has_valid_tools());
    }
}
