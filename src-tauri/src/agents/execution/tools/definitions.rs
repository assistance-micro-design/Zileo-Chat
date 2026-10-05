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
//! Tool definition registry (`collect_tool_definitions`) + `FunctionCallContext`.
use crate::mcp::MCPManager;
use crate::models::mcp::MCPTool;
use crate::tools::{validation_helper::ValidationHelper, Tool, ToolDefinition};
use std::sync::Arc;

/// Collects all tool definitions from local tools and MCP tools.
///
/// Creates ToolDefinition structs for all available tools so they can
/// be formatted by the provider adapter for JSON function calling.
pub(crate) fn collect_tool_definitions(
    local_tools: &[Arc<dyn Tool>],
    mcp_tools: &[(String, MCPTool)],
) -> Vec<ToolDefinition> {
    let mut definitions = Vec::new();

    for tool in local_tools {
        definitions.push(tool.definition());
    }

    for (server_name, mcp_tool) in mcp_tools {
        let summary = mcp_tool
            .description
            .split_once('.')
            .map(|(first, _)| first.trim().to_string())
            .unwrap_or_else(|| mcp_tool.description.clone());
        definitions.push(ToolDefinition {
            id: format!("mcp__{}__{}", server_name, mcp_tool.name),
            name: mcp_tool.name.clone(),
            summary,
            description: mcp_tool.description.clone(),
            input_schema: mcp_tool.input_schema.clone(),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        });
    }

    definitions
}

/// Groups the immutable context needed to execute function calls.
///
/// Created once before the tool loop and reused for every call,
/// avoiding repeated parameter passing.
pub(crate) struct FunctionCallContext<'a> {
    pub local_tools: &'a [Arc<dyn Tool>],
    pub mcp_manager: Option<&'a Arc<MCPManager>>,
    pub workflow_id: &'a str,
    pub validation_helper: Option<&'a ValidationHelper>,
    pub require_file_confirmation: bool,
    /// True for an unattended (detached) run — enables the detached MCP gate.
    pub is_detached: bool,
    /// True when this detached run is a DELEGATED sub-agent (DelegateTask /
    /// ParallelTasks), as opposed to a direct detached run or a spawned
    /// sub-agent. In a delegated run the allowlist gate additionally requires
    /// the matching entry's `allow_in_delegated_runs` flag.
    pub is_delegated: bool,
    /// Per-(server_id, tool) allowlist consulted by the detached MCP gate.
    pub mcp_tool_allowlist: &'a [crate::models::agent::McpToolAllowlistEntry],
    /// The calling agent's skill-name allowlist (`config.skills`). Consulted by
    /// the *Manager write gate to decide ownership: a skill
    /// content update/restore is only "owned" when the skill's name is in this
    /// list. Empty for agents with no skills.
    pub agent_skills: &'a [String],
}
