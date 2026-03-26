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

//! Agent configuration validation and serialization helpers.
//!
//! Pure functions for validating agent fields before database persistence.

use crate::constants::commands as cmd_const;
use crate::llm::ProviderType;
use crate::models::{AgentConfig, AgentConfigCreate, AgentConfigUpdate, LLMConfig};
use crate::tools::registry::TOOL_REGISTRY;
use crate::tools::validation_helper::validate_trimmed_name;

// ---------------------------------------------------------------------------
// Field validators
// ---------------------------------------------------------------------------

/// Delegates to centralized validate_trimmed_name
pub fn validate_agent_name(name: &str) -> Result<String, String> {
    validate_trimmed_name(name, "Agent name", cmd_const::MAX_AGENT_NAME_LEN)
}

/// Validates system prompt
pub fn validate_system_prompt(prompt: &str) -> Result<String, String> {
    let trimmed = prompt.trim();

    if trimmed.is_empty() {
        return Err("System prompt cannot be empty".to_string());
    }

    if trimmed.len() > cmd_const::MAX_SYSTEM_PROMPT_LEN {
        return Err(format!(
            "System prompt exceeds maximum length of {} characters",
            cmd_const::MAX_SYSTEM_PROMPT_LEN
        ));
    }

    Ok(trimmed.to_string())
}

/// Validates LLM configuration
pub fn validate_llm_config(llm: &LLMConfig) -> Result<LLMConfig, String> {
    // Validate provider (supports builtin + custom providers)
    llm.provider
        .parse::<ProviderType>()
        .map_err(|_| format!("Invalid provider '{}'", llm.provider))?;

    // Validate model name
    let model = llm.model.trim();
    if model.is_empty() {
        return Err("Model name cannot be empty".to_string());
    }
    if model.len() > 128 {
        return Err("Model name exceeds maximum length of 128 characters".to_string());
    }

    // Validate temperature
    if llm.temperature < cmd_const::MIN_TEMPERATURE || llm.temperature > cmd_const::MAX_TEMPERATURE
    {
        return Err(format!(
            "Temperature must be between {} and {}",
            cmd_const::MIN_TEMPERATURE,
            cmd_const::MAX_TEMPERATURE
        ));
    }

    // Validate max_tokens
    if llm.max_tokens < cmd_const::MIN_MAX_TOKENS || llm.max_tokens > cmd_const::MAX_MAX_TOKENS {
        return Err(format!(
            "max_tokens must be between {} and {}",
            cmd_const::MIN_MAX_TOKENS,
            cmd_const::MAX_MAX_TOKENS
        ));
    }

    Ok(LLMConfig {
        provider: llm.provider.clone(),
        model: model.to_string(),
        temperature: llm.temperature,
        max_tokens: llm.max_tokens,
        is_reasoning: llm.is_reasoning,
        context_window: llm.context_window,
    })
}

/// Validates tools list against the tool registry
pub fn validate_tools(tools: &[String]) -> Result<Vec<String>, String> {
    let mut validated = Vec::new();

    for tool in tools {
        let trimmed = tool.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !TOOL_REGISTRY.has_tool(trimmed) {
            return Err(format!(
                "Unknown tool '{}'. Available tools: {:?}",
                trimmed,
                TOOL_REGISTRY.available_tools()
            ));
        }

        validated.push(trimmed.to_string());
    }

    Ok(validated)
}

/// Validates a list of alphanumeric identifiers (skills, MCP servers).
///
/// Each entry is trimmed, empty entries are skipped.
/// Valid characters: alphanumeric, underscore, hyphen.
fn validate_identifier_list(
    items: &[String],
    label: &str,
    max_len: usize,
) -> Result<Vec<String>, String> {
    let mut validated = Vec::new();

    for item in items {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!(
                "Invalid {} name '{}'. Only alphanumeric, underscore, and hyphen allowed",
                label, trimmed
            ));
        }

        if trimmed.len() > max_len {
            return Err(format!(
                "{} name '{}' exceeds maximum length of {} characters",
                label, trimmed, max_len
            ));
        }

        validated.push(trimmed.to_string());
    }

    Ok(validated)
}

/// Validates skill names list
pub fn validate_skills(skills: &[String]) -> Result<Vec<String>, String> {
    validate_identifier_list(skills, "Skill", 128)
}

/// Validates MCP servers list
pub fn validate_mcp_servers(servers: &[String]) -> Result<Vec<String>, String> {
    validate_identifier_list(servers, "MCP server", 128)
}

// ---------------------------------------------------------------------------
// Composite validators
// ---------------------------------------------------------------------------

/// Validates full agent creation config
pub fn validate_agent_create(config: &AgentConfigCreate) -> Result<AgentConfigCreate, String> {
    Ok(AgentConfigCreate {
        name: validate_agent_name(&config.name)?,
        lifecycle: config.lifecycle.clone(),
        llm: validate_llm_config(&config.llm)?,
        tools: validate_tools(&config.tools)?,
        mcp_servers: validate_mcp_servers(&config.mcp_servers)?,
        skills: validate_skills(&config.skills)?,
        folders: config.folders.clone(),
        require_file_confirmation: config.require_file_confirmation,
        system_prompt: validate_system_prompt(&config.system_prompt)?,
        max_tool_iterations: config.max_tool_iterations.clamp(1, 200),
        reasoning_effort: config.reasoning_effort.clone(),
    })
}

/// Merges partial update fields with existing agent config, validating each field.
pub fn merge_agent_config(
    update: &AgentConfigUpdate,
    existing: &AgentConfig,
) -> Result<AgentConfig, String> {
    let name = match &update.name {
        Some(n) => validate_agent_name(n)?,
        None => existing.name.clone(),
    };
    let llm = match &update.llm {
        Some(l) => validate_llm_config(l)?,
        None => existing.llm.clone(),
    };
    let tools = match &update.tools {
        Some(t) => validate_tools(t)?,
        None => existing.tools.clone(),
    };
    let mcp_servers = match &update.mcp_servers {
        Some(m) => validate_mcp_servers(m)?,
        None => existing.mcp_servers.clone(),
    };
    let skills = match &update.skills {
        Some(s) => validate_skills(s)?,
        None => existing.skills.clone(),
    };
    let folders = match &update.folders {
        Some(f) => f.clone(),
        None => existing.folders.clone(),
    };
    let require_file_confirmation = update
        .require_file_confirmation
        .unwrap_or(existing.require_file_confirmation);
    let system_prompt = match &update.system_prompt {
        Some(p) => validate_system_prompt(p)?,
        None => existing.system_prompt.clone(),
    };
    let max_tool_iterations = update
        .max_tool_iterations
        .map_or(existing.max_tool_iterations, |m| m.clamp(1, 200));
    let reasoning_effort = match &update.reasoning_effort {
        Some(effort) => effort.clone(),
        None => existing.reasoning_effort.clone(),
    };

    Ok(AgentConfig {
        id: existing.id.clone(),
        name,
        lifecycle: existing.lifecycle.clone(),
        llm,
        tools,
        mcp_servers,
        skills,
        folders,
        require_file_confirmation,
        system_prompt,
        max_tool_iterations,
        reasoning_effort,
    })
}

// ---------------------------------------------------------------------------
// Database serialization
// ---------------------------------------------------------------------------

/// Formats reasoning_effort for SurrealDB storage
pub fn format_reasoning_effort(config: &AgentConfig) -> String {
    config
        .reasoning_effort
        .as_ref()
        .map_or("NONE".to_string(), |e| format!("'{}'", e.as_str()))
}
