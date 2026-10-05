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
//! MCP tool definition and server-summary collection.
use crate::agents::prompt::MCPServerSummary;
use crate::mcp::MCPManager;
use crate::models::mcp::MCPTool;
use crate::models::AgentConfig;
use tracing::warn;

/// Collects MCP tool definitions with full metadata from configured servers.
pub(crate) async fn get_mcp_tool_definitions(
    config: &AgentConfig,
    mcp_manager: &MCPManager,
) -> Vec<(String, MCPTool)> {
    let mut all_tools = Vec::new();

    for server_name in &config.mcp_servers {
        let tools = mcp_manager.list_server_tools(server_name).await;
        for tool in tools {
            all_tools.push((server_name.clone(), tool));
        }
    }

    all_tools
}

/// Collects summaries of ALL available MCP servers (enabled and running only).
///
/// This provides high-level information about each MCP server so the agent
/// can make informed decisions when spawning sub-agents with specific MCP servers.
pub(crate) async fn get_mcp_server_summaries(
    config: &AgentConfig,
    mcp_manager: &MCPManager,
) -> Vec<MCPServerSummary> {
    let mut summaries = Vec::new();

    let all_servers = match mcp_manager.list_servers().await {
        Ok(servers) => servers,
        Err(e) => {
            warn!(error = %e, "Failed to list MCP servers for documentation");
            return summaries;
        }
    };

    let direct_access: std::collections::HashSet<&String> = config.mcp_servers.iter().collect();

    for server in all_servers {
        if server.config.enabled && server.status == crate::models::mcp::MCPServerStatus::Running {
            let name = server.config.name.clone();
            let has_direct_access = direct_access.contains(&name);

            summaries.push(MCPServerSummary {
                name,
                description: server.config.description.clone(),
                tools_count: server.tools.len(),
                has_direct_access,
            });
        }
    }

    summaries
}
