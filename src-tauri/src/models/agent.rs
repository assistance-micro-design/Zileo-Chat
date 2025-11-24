// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
    pub tools: Vec<String>,
    /// List of MCP servers
    pub mcp_servers: Vec<String>,
    /// System prompt
    pub system_prompt: String,
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
}
