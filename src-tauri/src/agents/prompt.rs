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

//! Prompt building for LLM agents.
//!
//! This module handles:
//! - Building user prompts with conversation history and context
//! - Building system prompts with tool definitions
//! - Detecting generic completion messages for report enforcement

use crate::agents::core::agent::Task;
use crate::models::mcp::MCPTool;
use crate::models::AgentConfig;
use crate::tools::Tool;
use chrono::Local;
use std::sync::Arc;

/// Prompt sent to the LLM when a generic completion message is detected,
/// requesting a proper markdown report of what was accomplished.
pub(crate) const REPORT_ENFORCEMENT_PROMPT: &str = "You have completed your task using tools. However, you did not provide a summary of what you accomplished. Please provide a concise report in markdown format that describes:\n\n1. What actions you performed\n2. The key results or outcomes\n3. Any important details the user should know\n\nBe specific and reference the actual work done based on the tool calls you made. Respond in the same language as the original task.";

/// Summary of an MCP server for documentation in system prompt
#[derive(Debug, Clone)]
pub(crate) struct MCPServerSummary {
    /// Human-readable server name (used as identifier in mcp_servers parameter)
    pub name: String,
    /// Description of what the server does
    pub description: Option<String>,
    /// Number of tools available from this server
    pub tools_count: usize,
    /// Whether this agent has direct access to this server
    pub has_direct_access: bool,
}

/// Checks whether the given response content is a generic completion message
/// (i.e., the LLM did not provide a meaningful report).
///
/// Returns `true` if the content matches known generic fallback patterns,
/// indicating that a follow-up report request should be made.
pub(crate) fn is_generic_completion_message(content: &str) -> bool {
    let trimmed = content.trim();

    // Pattern 1: "Task completed after N iteration(s). Tool executions completed successfully."
    if trimmed.starts_with("Task completed after ")
        && trimmed.contains("iteration")
        && trimmed.contains("Tool executions completed successfully")
    {
        return true;
    }

    // Pattern 2: "Max tool iterations (N) reached, stopping execution"
    if trimmed.starts_with("Max tool iterations")
        && trimmed.contains("reached")
        && trimmed.contains("stopping execution")
    {
        return true;
    }

    // Pattern 3: Empty or whitespace-only
    if trimmed.is_empty() {
        return true;
    }

    false
}

/// Builds the full user prompt with conversation history and context.
///
/// # Mistral API Compatibility
///
/// Mistral's API requires the last message to be a "user" or "tool" role.
/// To avoid role confusion, we format conversation history as quoted context
/// rather than using role markers like `[assistant]:` which might be
/// misinterpreted by the API.
pub(crate) fn build_prompt(task: &Task) -> String {
    // Check for conversation history in context
    let history_str = if let Some(history) = task.context.get("conversation_history") {
        if let Some(messages) = history.as_array() {
            if messages.is_empty() {
                String::new()
            } else {
                // Format messages in a way that won't confuse Mistral's API
                let formatted: Vec<String> = messages
                    .iter()
                    .filter_map(|msg| {
                        let role = msg.get("role")?.as_str()?;
                        let content = msg.get("content")?.as_str()?;
                        match role {
                            "user" => Some(format!("[Human]\n{}\n", content)),
                            "assistant" => Some(format!("[AI Response]\n{}\n", content)),
                            "system" => Some(format!("[System Note]\n{}\n", content)),
                            _ => Some(format!("[{}]\n{}\n", role, content)),
                        }
                    })
                    .collect();
                format!(
                    "\n\n--- Conversation Context ---\n{}\n--- End Context ---\n\nPlease respond to the current request:\n",
                    formatted.join("\n\n")
                )
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Build context string (excluding conversation_history which was handled above)
    let other_context: serde_json::Value = if let Some(obj) = task.context.as_object() {
        let filtered: serde_json::Map<String, serde_json::Value> = obj
            .iter()
            .filter(|(k, _)| *k != "conversation_history")
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if filtered.is_empty() {
            serde_json::json!({})
        } else {
            serde_json::Value::Object(filtered)
        }
    } else {
        serde_json::json!({})
    };

    let context_str = if other_context.is_null() || other_context == serde_json::json!({}) {
        String::new()
    } else {
        format!(
            "\n\nContext:\n```json\n{}\n```",
            serde_json::to_string_pretty(&other_context).unwrap_or_default()
        )
    };

    format!("{}{}{}", history_str, task.description, context_str)
}

/// Builds enhanced system prompt for JSON function calling.
///
/// With JSON function calling (OpenAI standard), tool definitions are passed
/// via the API's `tools` parameter, NOT in the system prompt. This method
/// builds a simplified prompt that includes:
/// - The agent's base system prompt
/// - Context about available tools (names only, schemas are in API)
/// - Available MCP servers for sub-agent delegation
/// - Current date/time and user's selected language
pub(crate) fn build_system_prompt_with_tools(
    config: &AgentConfig,
    local_tools: &[Arc<dyn Tool>],
    mcp_tools: &[(String, MCPTool)],
    mcp_server_summaries: &[MCPServerSummary],
    locale: Option<&str>,
    has_delegation_tools: bool,
) -> String {
    let mut sections = vec![config.system_prompt.clone()];

    // Only add tool context if there are tools available
    if local_tools.is_empty() && mcp_tools.is_empty() {
        return sections.join("\n\n");
    }

    // Brief tool context (full definitions are in the API tools parameter)
    let mut tools_context = String::from("## Available Tools\n\n");
    tools_context.push_str(
        "You have access to the following tools via function calling. \
         The API will provide the tool schemas; use function calls to invoke them.\n",
    );

    // Local tools: summary only (full description is in the API tools parameter)
    if !local_tools.is_empty() {
        tools_context.push_str("\n### Local Tools\n");
        for tool in local_tools {
            let def = tool.definition();
            tools_context.push_str(&format!("- **{}**: {}\n", def.name, def.summary));
        }
    }

    // MCP tools: full description (short descriptions, no duplication issue)
    if !mcp_tools.is_empty() {
        tools_context.push_str("\n### MCP Tools (Direct Access)\n");
        tools_context
            .push_str("MCP tools use the naming format `mcp__server__tool`. Use them directly.\n");
        for (server_name, tool) in mcp_tools {
            tools_context.push_str(&format!(
                "- **mcp__{}__{}**: {}\n",
                server_name, tool.name, tool.description
            ));
        }
    }

    sections.push(tools_context);

    // Skills: list only (usage instructions are in the agent's system_prompt)
    if !config.skills.is_empty() {
        let mut skills_section = String::from("## Available Skills\n");
        for skill_name in &config.skills {
            skills_section.push_str(&format!("- `{}`\n", skill_name));
        }
        sections.push(skills_section);
    }

    // Agent configuration context
    let now = Local::now();

    let language_display = match locale {
        Some("fr") => "French (Francais)",
        Some("en") => "English",
        Some(code) => code,
        None => "English",
    };

    let mut config_section = format!(
        r#"## Your Configuration

**Current Date and Time**: {} (local timezone)
**User Language**: {} - Always respond in this language unless explicitly asked otherwise."#,
        now.format("%A %d %B %Y, %H:%M:%S"),
        language_display,
    );

    // Provider/Model only if agent has delegation tools
    if has_delegation_tools {
        config_section.push_str(&format!(
            "\n\nYou are currently running with the following configuration:\n- **Provider**: {}\n- **Model**: {}",
            config.llm.provider,
            config.llm.model,
        ));
    }

    // MCP Delegation: only [DELEGATE] servers, only if has_delegation_tools
    if has_delegation_tools && !mcp_server_summaries.is_empty() {
        let delegate_servers: Vec<_> = mcp_server_summaries
            .iter()
            .filter(|s| !s.has_direct_access)
            .collect();

        if !delegate_servers.is_empty() {
            config_section.push_str("\n\n### Available MCP Servers for Delegation\n");
            for server in &delegate_servers {
                config_section.push_str(&format!(
                    "- **{}** - {} - {} tools\n",
                    server.name,
                    server.description.as_deref().unwrap_or("No description"),
                    server.tools_count
                ));
            }
            config_section.push_str(&format!(
                "\n**Example**: To assign MCP servers to sub-agents:\n```json\n{{\"mcp_servers\": [{}]}}\n```\n",
                delegate_servers
                    .iter()
                    .map(|s| format!("\"{}\"", s.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            config_section.push_str(
                "\nWhen spawning sub-agents, you can specify provider/model/mcp_servers or let them inherit from your configuration.",
            );
        } else {
            config_section.push_str(
                "\nWhen spawning sub-agents, you can specify provider/model or let them inherit from your configuration.",
            );
        }
    } else if has_delegation_tools {
        config_section.push_str(
            "\nWhen spawning sub-agents, you can specify provider/model or let them inherit from your configuration.",
        );
    }

    sections.push(config_section);

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_generic_completion_message_standard_pattern() {
        assert!(is_generic_completion_message(
            "Task completed after 2 iteration(s). Tool executions completed successfully."
        ));
        assert!(is_generic_completion_message(
            "Task completed after 1 iteration(s). Tool executions completed successfully."
        ));
        assert!(is_generic_completion_message(
            "Task completed after 15 iteration(s). Tool executions completed successfully."
        ));
    }

    #[test]
    fn test_is_generic_completion_message_max_iterations_pattern() {
        assert!(is_generic_completion_message(
            "Max tool iterations (50) reached, stopping execution"
        ));
        assert!(is_generic_completion_message(
            "Max tool iterations (200) reached, stopping execution"
        ));
    }

    #[test]
    fn test_is_generic_completion_message_empty() {
        assert!(is_generic_completion_message(""));
        assert!(is_generic_completion_message("   "));
        assert!(is_generic_completion_message("\n\t  "));
    }

    #[test]
    fn test_is_generic_completion_message_real_reports() {
        assert!(!is_generic_completion_message(
            "## Summary\n\nI analyzed the data and found 3 key insights."
        ));
        assert!(!is_generic_completion_message(
            "The task has been completed. Here are the results:\n- Item 1\n- Item 2"
        ));
        assert!(!is_generic_completion_message(
            "I successfully created the new component with the following structure..."
        ));
    }

    #[test]
    fn test_is_generic_completion_message_with_whitespace() {
        assert!(is_generic_completion_message(
            "  Task completed after 3 iteration(s). Tool executions completed successfully.  "
        ));
    }

    #[test]
    fn test_report_enforcement_prompt_is_valid() {
        assert!(!REPORT_ENFORCEMENT_PROMPT.is_empty());
        assert!(REPORT_ENFORCEMENT_PROMPT.contains("markdown"));
        assert!(REPORT_ENFORCEMENT_PROMPT.contains("report"));
    }
}
