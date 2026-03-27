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

//! Tool management for LLM agent execution.
//!
//! Handles tool creation, definition collection, and individual tool execution
//! for both local tools and MCP tools.

use crate::agents::prompt::MCPServerSummary;
use crate::mcp::MCPManager;
use crate::models::function_calling::{FunctionCall, FunctionCallResult};
use crate::models::mcp::MCPTool;
use crate::models::AgentConfig;
use crate::tools::{
    context::AgentToolContext,
    validation_helper::{is_destructive_file_op, ValidationHelper},
    Tool, ToolDefinition, ToolFactory,
};
use std::sync::Arc;
use tracing::{debug, info, warn};

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

/// Creates local tool instances for configured tools.
///
/// When `is_primary_agent` is true and `agent_context` is available,
/// this method will also create sub-agent tools (SpawnAgentTool,
/// DelegateTaskTool, ParallelTasksTool) in addition to basic tools.
pub(crate) async fn create_local_tools(
    config: &AgentConfig,
    tool_factory: Option<&Arc<ToolFactory>>,
    agent_context: Option<&AgentToolContext>,
    workflow_id: Option<String>,
    is_primary_agent: bool,
    context_override: Option<&AgentToolContext>,
) -> Vec<Arc<dyn Tool>> {
    let Some(factory) = tool_factory else {
        return Vec::new();
    };

    // Use override if provided, otherwise fall back to agent_context
    let effective_context = context_override.or(agent_context);

    // Extract app_handle from context if available
    let app_handle = effective_context.and_then(|ctx| ctx.app_handle.clone());

    // Auto-inject ReadSkillTool when agent has skills assigned
    let mut tool_names: Vec<String> = config.tools.clone();
    if !config.skills.is_empty() && !tool_names.iter().any(|t| t == "ReadSkillTool") {
        debug!(
            agent_id = %config.id,
            skills_count = config.skills.len(),
            "Auto-injecting ReadSkillTool for agent with skills"
        );
        tool_names.push("ReadSkillTool".to_string());
    }

    // If this is the primary agent and we have context, use create_tools_with_context
    if is_primary_agent {
        if let Some(context) = effective_context {
            debug!(
                agent_id = %config.id,
                "Creating tools with context for primary agent (sub-agent tools available)"
            );
            return factory
                .create_tools_with_context(
                    &tool_names,
                    workflow_id,
                    config.id.clone(),
                    Some(context.clone()),
                    true,
                )
                .await;
        }
    }

    // For sub-agents or agents without context, use basic tool creation
    debug!(
        agent_id = %config.id,
        is_primary_agent = is_primary_agent,
        has_context = effective_context.is_some(),
        "Creating basic tools (sub-agent tools NOT available)"
    );
    factory
        .create_tools(&tool_names, workflow_id, config.id.clone(), app_handle)
        .await
}

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
}

/// Executes a single function call (local or MCP tool).
pub(crate) async fn execute_function_call(
    call: &FunctionCall,
    ctx: &FunctionCallContext<'_>,
    tools_used: &mut Vec<String>,
    mcp_calls_made: &mut Vec<String>,
) -> FunctionCallResult {
    let start = std::time::Instant::now();

    // Check if MCP tool
    if let Some((server, tool)) = call.parse_mcp_name() {
        // Execute via MCP
        if let Some(mcp) = ctx.mcp_manager {
            mcp_calls_made.push(call.name.clone());

            // Request validation for MCP tool call
            if let Some(helper) = ctx.validation_helper {
                if let Err(e) = helper
                    .request_mcp_validation(ctx.workflow_id, server, tool, call.arguments.clone())
                    .await
                {
                    warn!(tool = %call.name, error = %e, "MCP validation rejected");
                    return FunctionCallResult::failure(&call.id, &call.name, e.to_string());
                }
            }

            match mcp.call_tool(server, tool, call.arguments.clone()).await {
                Ok(result) => {
                    if result.success {
                        info!(tool = %call.name, "MCP tool executed successfully");
                        FunctionCallResult::success(&call.id, &call.name, result.content)
                            .with_execution_time(start.elapsed().as_millis() as u64)
                    } else {
                        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                        warn!(tool = %call.name, error = %error_msg, "MCP tool returned error");
                        FunctionCallResult::failure(&call.id, &call.name, error_msg)
                    }
                }
                Err(e) => {
                    warn!(tool = %call.name, error = %e, "MCP tool call failed");
                    FunctionCallResult::failure(&call.id, &call.name, e.to_string())
                }
            }
        } else {
            FunctionCallResult::failure(&call.id, &call.name, "MCP manager not available")
        }
    } else {
        // Execute local tool
        let matching_tool = ctx
            .local_tools
            .iter()
            .find(|t| t.definition().id == call.name);

        if let Some(tool) = matching_tool {
            tools_used.push(call.name.clone());

            // Request validation for local tool
            // Skip validation for sub-agent tools (they have their own validation)
            let is_sub_agent_tool = call.name == "SpawnAgentTool"
                || call.name == "DelegateTaskTool"
                || call.name == "ParallelTasksTool";

            if !is_sub_agent_tool {
                // FileManagerTool: use file-specific validation if destructive + confirmation enabled
                if call.name == "FileManagerTool" && ctx.require_file_confirmation {
                    let operation = call
                        .arguments
                        .get("operation")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    if is_destructive_file_op(operation) {
                        if let Some(helper) = ctx.validation_helper {
                            let path = call
                                .arguments
                                .get("path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");

                            if let Err(e) = helper
                                .request_file_validation(
                                    ctx.workflow_id,
                                    operation,
                                    path,
                                    call.arguments.clone(),
                                )
                                .await
                            {
                                warn!(tool = %call.name, operation = %operation, error = %e, "File operation validation rejected");
                                return FunctionCallResult::failure(
                                    &call.id,
                                    &call.name,
                                    e.to_string(),
                                );
                            }
                        }
                    }
                } else if let Some(helper) = ctx.validation_helper {
                    // Standard tool validation for non-FileManagerTool
                    let operation = call
                        .arguments
                        .get("operation")
                        .and_then(|v| v.as_str())
                        .unwrap_or("execute");

                    if let Err(e) = helper
                        .request_tool_validation(
                            ctx.workflow_id,
                            &call.name,
                            operation,
                            call.arguments.clone(),
                        )
                        .await
                    {
                        warn!(tool = %call.name, error = %e, "Tool validation rejected");
                        return FunctionCallResult::failure(&call.id, &call.name, e.to_string());
                    }
                }
            }

            match tool.execute(call.arguments.clone()).await {
                Ok(result) => {
                    info!(tool = %call.name, "Local tool executed successfully");
                    FunctionCallResult::success(&call.id, &call.name, result)
                        .with_execution_time(start.elapsed().as_millis() as u64)
                }
                Err(e) => {
                    warn!(tool = %call.name, error = %e, "Local tool execution failed");
                    FunctionCallResult::failure(&call.id, &call.name, e.to_string())
                }
            }
        } else {
            let available_tools: Vec<String> = ctx
                .local_tools
                .iter()
                .map(|t| t.definition().id.clone())
                .collect();

            FunctionCallResult::failure(
                &call.id,
                &call.name,
                format!(
                    "Unknown tool '{}'. Available tools: {}",
                    call.name,
                    available_tools.join(", ")
                ),
            )
        }
    }
}
