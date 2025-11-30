// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! LLM Agent - Agent that uses real LLM calls via ProviderManager
//!
//! This agent supports tool execution integration, allowing it to call both
//! local tools (MemoryTool, TodoTool) and MCP tools during workflow execution.
//!
//! # Tool Execution Flow
//!
//! 1. Agent receives task with configured tools
//! 2. System prompt is enhanced with tool definitions
//! 3. LLM generates response, potentially including tool calls
//! 4. Agent parses tool calls from response using XML markers
//! 5. Tools are executed via ToolFactory (local) or MCPManager (MCP)
//! 6. Results are fed back to LLM for continuation
//! 7. Loop continues until no more tool calls or max iterations reached

use crate::agents::core::agent::{
    Agent, Report, ReportMetrics, ReportStatus, Task, ToolExecutionData,
};
use crate::db::DBClient;
use crate::llm::{LLMError, ProviderManager, ProviderType};
use crate::mcp::MCPManager;
use crate::models::mcp::MCPTool;
use crate::models::streaming::{events, StreamChunk};
use crate::models::{AgentConfig, Lifecycle};
use crate::tools::{context::AgentToolContext, Tool, ToolFactory};
use async_trait::async_trait;
use chrono::Local;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::Emitter;
use tracing::{debug, error, info, instrument, warn};

/// Default maximum number of tool execution iterations to prevent infinite loops
/// Can be overridden per-agent via AgentConfig.max_tool_iterations
#[allow(dead_code)]
const DEFAULT_MAX_TOOL_ITERATIONS: usize = 50;

/// Parsed tool call extracted from LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedToolCall {
    /// Tool name (e.g., "MemoryTool" or "serena:find_symbol")
    pub tool_name: String,
    /// Arguments as JSON
    pub arguments: serde_json::Value,
    /// Whether this is an MCP tool (format: "server:tool")
    pub is_mcp: bool,
    /// MCP server name if is_mcp is true
    pub mcp_server: Option<String>,
    /// MCP tool name if is_mcp is true
    pub mcp_tool: Option<String>,
}

/// Result of tool execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionResult {
    /// Tool that was called
    pub tool_name: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Result JSON on success
    pub result: serde_json::Value,
    /// Error message on failure
    pub error: Option<String>,
}

/// Summary of an MCP server for documentation in system prompt
///
/// Used to provide high-level information about available MCP servers
/// so the agent can make informed decisions when spawning sub-agents.
#[derive(Debug, Clone)]
struct MCPServerSummary {
    /// Server ID (used in mcp_servers parameter)
    id: String,
    /// Human-readable server name
    name: String,
    /// Description of what the server does
    description: Option<String>,
    /// Current status (running, stopped, etc.)
    status: String,
    /// Number of tools available from this server
    tools_count: usize,
}

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
    /// Creates a new LLM agent without tool support
    ///
    /// # Arguments
    /// * `config` - Agent configuration including LLM settings
    /// * `provider_manager` - Shared provider manager for LLM calls
    #[allow(dead_code)]
    pub fn new(config: AgentConfig, provider_manager: Arc<ProviderManager>) -> Self {
        Self {
            config,
            provider_manager,
            tool_factory: None,
            agent_context: None,
        }
    }

    /// Creates a new LLM agent with tool execution support
    ///
    /// # Arguments
    /// * `config` - Agent configuration including LLM settings
    /// * `provider_manager` - Shared provider manager for LLM calls
    /// * `db` - Database client for tool persistence
    ///
    /// # Example
    /// ```ignore
    /// let agent = LLMAgent::with_tools(config, provider_manager, db);
    /// ```
    #[allow(dead_code)]
    pub fn with_tools(
        config: AgentConfig,
        provider_manager: Arc<ProviderManager>,
        db: Arc<DBClient>,
    ) -> Self {
        let tool_factory = Arc::new(ToolFactory::new(db, None));
        Self {
            config,
            provider_manager,
            tool_factory: Some(tool_factory),
            agent_context: None,
        }
    }

    /// Creates a new LLM agent with a custom tool factory
    ///
    /// Use this when you need to provide embedding service for MemoryTool.
    /// This constructor does NOT provide AgentToolContext, so sub-agent tools
    /// will not be available.
    #[allow(dead_code)]
    pub fn with_factory(
        config: AgentConfig,
        provider_manager: Arc<ProviderManager>,
        tool_factory: Arc<ToolFactory>,
    ) -> Self {
        Self {
            config,
            provider_manager,
            tool_factory: Some(tool_factory),
            agent_context: None,
        }
    }

    /// Creates a new LLM agent with AgentToolContext for sub-agent operations.
    ///
    /// This constructor provides the agent with access to sub-agent tools
    /// (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool) when used as
    /// the primary workflow agent.
    ///
    /// # Arguments
    /// * `config` - Agent configuration including LLM settings
    /// * `provider_manager` - Shared provider manager for LLM calls
    /// * `tool_factory` - Factory for creating tool instances
    /// * `agent_context` - Context providing access to agent system dependencies
    ///
    /// # Sub-Agent Tools Availability
    ///
    /// Sub-agent tools are only available when:
    /// 1. The agent has an AgentToolContext (this constructor provides one)
    /// 2. The task context includes `"is_primary_agent": true`
    ///
    /// Sub-agents created via SpawnAgentTool use `with_factory()` instead,
    /// ensuring they cannot spawn other sub-agents (single level constraint).
    ///
    /// # Example
    /// ```ignore
    /// let context = AgentToolContext::from_app_state_full(&state);
    /// let agent = LLMAgent::with_context(config, provider_manager, tool_factory, context);
    /// ```
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

    /// Gets the provider type from config
    fn get_provider_type(&self) -> Result<ProviderType, LLMError> {
        self.config.llm.provider.parse()
    }

    /// Builds the full prompt with conversation history and context
    ///
    /// # Mistral API Compatibility
    ///
    /// Mistral's API requires the last message to be a "user" or "tool" role.
    /// To avoid role confusion, we format conversation history as quoted context
    /// rather than using role markers like `[assistant]:` which might be
    /// misinterpreted by the API.
    fn build_prompt(&self, task: &Task) -> String {
        // Check for conversation history in context
        let history_str = if let Some(history) = task.context.get("conversation_history") {
            if let Some(messages) = history.as_array() {
                if messages.is_empty() {
                    String::new()
                } else {
                    // Format messages in a way that won't confuse Mistral's API
                    // Avoid role markers that might be interpreted as actual roles
                    let formatted: Vec<String> = messages
                        .iter()
                        .filter_map(|msg| {
                            let role = msg.get("role")?.as_str()?;
                            let content = msg.get("content")?.as_str()?;
                            // Use format that won't be confused with API role markers
                            // Mistral interprets "USER:", "ASSISTANT:" etc. as actual roles
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

    /// Builds prompt with available MCP tools information
    #[allow(dead_code)]
    fn build_prompt_with_tools(&self, task: &Task, available_tools: &[String]) -> String {
        let base_prompt = self.build_prompt(task);

        if available_tools.is_empty() {
            return base_prompt;
        }

        let tools_info = format!(
            "\n\nAvailable MCP Tools:\n{}",
            available_tools
                .iter()
                .map(|t| format!("- {}", t))
                .collect::<Vec<_>>()
                .join("\n")
        );

        format!("{}{}", base_prompt, tools_info)
    }

    /// Executes an MCP tool call and returns the result
    ///
    /// This method is prepared for future phases where the agent will
    /// parse LLM responses to extract tool calls and execute them.
    #[allow(dead_code)]
    async fn call_mcp_tool(
        &self,
        mcp_manager: &MCPManager,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, String> {
        debug!(
            server_name = %server_name,
            tool_name = %tool_name,
            "Calling MCP tool"
        );

        match mcp_manager
            .call_tool(server_name, tool_name, arguments)
            .await
        {
            Ok(result) => {
                if result.success {
                    Ok(serde_json::to_string_pretty(&result.content)
                        .unwrap_or_else(|_| result.content.to_string()))
                } else {
                    Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
                }
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Collects available tools from configured MCP servers
    #[allow(dead_code)]
    async fn get_available_mcp_tools(&self, mcp_manager: &MCPManager) -> Vec<String> {
        let mut all_tools = Vec::new();

        for server_name in &self.config.mcp_servers {
            let tools = mcp_manager.list_server_tools(server_name).await;
            for tool in tools {
                all_tools.push(format!("{}:{}", server_name, tool.name));
            }
        }

        all_tools
    }

    /// Collects MCP tool definitions with full metadata from configured servers
    async fn get_mcp_tool_definitions(&self, mcp_manager: &MCPManager) -> Vec<(String, MCPTool)> {
        let mut all_tools = Vec::new();

        for server_name in &self.config.mcp_servers {
            let tools = mcp_manager.list_server_tools(server_name).await;
            for tool in tools {
                all_tools.push((server_name.clone(), tool));
            }
        }

        all_tools
    }

    /// Collects summaries of ALL available MCP servers (enabled and running only)
    ///
    /// This provides high-level information about each MCP server so the agent
    /// can make informed decisions when spawning sub-agents with specific MCP servers.
    /// Unlike `self.config.mcp_servers` which only lists servers assigned to this agent,
    /// this method returns ALL available servers that the agent can assign to sub-agents.
    async fn get_mcp_server_summaries(&self, mcp_manager: &MCPManager) -> Vec<MCPServerSummary> {
        let mut summaries = Vec::new();

        // Get ALL servers from the manager, not just those assigned to this agent
        let all_servers = match mcp_manager.list_servers().await {
            Ok(servers) => servers,
            Err(e) => {
                warn!(error = %e, "Failed to list MCP servers for documentation");
                return summaries;
            }
        };

        for server in all_servers {
            // Only include enabled servers that are running
            if server.config.enabled
                && server.status == crate::models::mcp::MCPServerStatus::Running
            {
                summaries.push(MCPServerSummary {
                    id: server.config.id.clone(),
                    name: server.config.name.clone(),
                    description: server.config.description.clone(),
                    status: server.status.to_string(),
                    tools_count: server.tools.len(),
                });
            }
        }

        summaries
    }

    /// Creates local tool instances for configured tools
    ///
    /// When `is_primary_agent` is true and `agent_context` is available,
    /// this method will also create sub-agent tools (SpawnAgentTool,
    /// DelegateTaskTool, ParallelTasksTool) in addition to basic tools.
    ///
    /// # Arguments
    /// * `workflow_id` - Optional workflow ID for scoping tool operations
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    fn create_local_tools(
        &self,
        workflow_id: Option<String>,
        is_primary_agent: bool,
    ) -> Vec<Arc<dyn Tool>> {
        let Some(ref factory) = self.tool_factory else {
            return Vec::new();
        };

        // If this is the primary agent and we have context, use create_tools_with_context
        // to include sub-agent tools
        if is_primary_agent {
            if let Some(ref context) = self.agent_context {
                debug!(
                    agent_id = %self.config.id,
                    "Creating tools with context for primary agent (sub-agent tools available)"
                );
                return factory.create_tools_with_context(
                    &self.config.tools,
                    workflow_id,
                    self.config.id.clone(),
                    Some(context.clone()),
                    true, // is_primary_agent
                );
            }
        }

        // For sub-agents or agents without context, use basic tool creation
        debug!(
            agent_id = %self.config.id,
            is_primary_agent = is_primary_agent,
            has_context = self.agent_context.is_some(),
            "Creating basic tools (sub-agent tools NOT available)"
        );
        factory.create_tools(&self.config.tools, workflow_id, self.config.id.clone())
    }

    /// Builds enhanced system prompt with tool definitions for LLM
    ///
    /// This method constructs a system prompt that includes:
    /// - The agent's base system prompt
    /// - Instructions on how to call tools
    /// - Definitions of all available local tools
    /// - Definitions of all available MCP tools
    /// - Available MCP servers with descriptions (for sub-agent spawning)
    fn build_system_prompt_with_tools(
        &self,
        local_tools: &[Arc<dyn Tool>],
        mcp_tools: &[(String, MCPTool)],
        mcp_server_summaries: &[MCPServerSummary],
    ) -> String {
        let mut sections = vec![self.config.system_prompt.clone()];

        // Only add tool instructions if there are tools available
        if local_tools.is_empty() && mcp_tools.is_empty() {
            return sections.join("\n\n");
        }

        // Tool calling instructions
        sections.push(
            r#"## Tool Usage Instructions

You have access to tools that can help you complete tasks. To call a tool, use this exact format:

<tool_call name="ToolName">
{"operation": "...", "param": "value"}
</tool_call>

After calling a tool, wait for the result before continuing. Tool results will be provided in this format:

<tool_result name="ToolName" success="true">
{...result JSON...}
</tool_result>

You can call multiple tools in sequence. Always analyze tool results before proceeding."#
                .to_string(),
        );

        // Local tools section
        if !local_tools.is_empty() {
            let mut local_section = String::from("## Local Tools\n");

            for tool in local_tools {
                let def = tool.definition();
                local_section.push_str(&format!(
                    "\n### {}\n**Description**: {}\n\n**Input Schema**:\n```json\n{}\n```\n",
                    def.name,
                    def.description,
                    serde_json::to_string_pretty(&def.input_schema).unwrap_or_default()
                ));
            }

            sections.push(local_section);
        }

        // MCP tools section - these are tools the agent can use DIRECTLY
        if !mcp_tools.is_empty() {
            let mut mcp_section = String::from(
                "## MCP Tools (Direct Access)\n\n**IMPORTANT**: These are YOUR tools. Use them directly - do NOT delegate to a sub-agent.\nTo call MCP tools, use the format: `server_id:tool_name`\n",
            );

            for (server_id, tool) in mcp_tools {
                mcp_section.push_str(&format!(
                    "\n### {}:{}\n**Description**: {}\n\n**Input Schema**:\n```json\n{}\n```\n",
                    server_id,
                    tool.name,
                    tool.description,
                    serde_json::to_string_pretty(&tool.input_schema).unwrap_or_default()
                ));
            }

            sections.push(mcp_section);
        }

        // Add agent configuration context (provider, model, available resources)
        // This helps the LLM make informed decisions when spawning sub-agents
        let now = Local::now();
        let mut config_section = format!(
            r#"## Your Configuration

**Current Date and Time**: {} (local timezone)

You are currently running with the following configuration:
- **Provider**: {}
- **Model**: {}"#,
            now.format("%A %d %B %Y, %H:%M:%S"),
            self.config.llm.provider,
            self.config.llm.model,
        );

        // Add detailed MCP server information with descriptions
        // Determine which MCP servers this agent has direct access to
        let direct_mcp_ids: std::collections::HashSet<&String> =
            self.config.mcp_servers.iter().collect();

        if mcp_server_summaries.is_empty() {
            config_section.push_str("\n- **Available MCP Servers**: None configured or running");
        } else {
            config_section.push_str("\n\n### Available MCP Servers for Delegation\n");
            config_section.push_str(
                "These servers can be assigned to sub-agents using the `mcp_servers` parameter.\n",
            );
            config_section.push_str(
                "**Note**: If you already have direct access to an MCP (listed in 'MCP Tools' above), use it directly instead of delegating.\n",
            );

            for server in mcp_server_summaries {
                let access_note = if direct_mcp_ids.contains(&server.id) {
                    " [YOU HAVE DIRECT ACCESS - use it directly!]"
                } else {
                    " [Delegate only]"
                };
                config_section.push_str(&format!(
                    "\n- **{}** (ID: `{}`){}\n  - Description: {}\n  - Status: {} | Tools: {}\n",
                    server.name,
                    server.id,
                    access_note,
                    server.description.as_deref().unwrap_or("No description"),
                    server.status,
                    server.tools_count
                ));
            }
        }

        config_section.push_str(
            "\n\nWhen spawning sub-agents, you can specify provider/model/mcp_servers or let them inherit from your configuration.",
        );
        sections.push(config_section);

        sections.join("\n\n")
    }

    /// Parses tool calls from LLM response text
    ///
    /// Extracts tool calls using XML-style markers:
    /// ```text
    /// <tool_call name="ToolName">
    /// {"operation": "...", "param": "value"}
    /// </tool_call>
    /// ```
    fn parse_tool_calls(response: &str) -> Vec<ParsedToolCall> {
        let mut calls = Vec::new();

        // Pattern: <tool_call name="...">...</tool_call>
        let pattern = Regex::new(r#"<tool_call\s+name="([^"]+)">\s*([\s\S]*?)\s*</tool_call>"#)
            .expect("Invalid regex");

        for cap in pattern.captures_iter(response) {
            let tool_name = cap[1].to_string();
            let json_str = cap[2].trim();

            // Try to parse the JSON arguments
            let arguments = match serde_json::from_str(json_str) {
                Ok(args) => args,
                Err(e) => {
                    warn!(tool = %tool_name, error = %e, "Failed to parse tool arguments");
                    continue;
                }
            };

            // Check if this is an MCP tool (format: server:tool)
            let (is_mcp, mcp_server, mcp_tool) = if tool_name.contains(':') {
                let parts: Vec<&str> = tool_name.splitn(2, ':').collect();
                if parts.len() == 2 {
                    (true, Some(parts[0].to_string()), Some(parts[1].to_string()))
                } else {
                    (false, None, None)
                }
            } else {
                (false, None, None)
            };

            calls.push(ParsedToolCall {
                tool_name,
                arguments,
                is_mcp,
                mcp_server,
                mcp_tool,
            });
        }

        debug!(count = calls.len(), "Parsed tool calls from response");
        calls
    }

    /// Executes a local tool and returns the result
    async fn execute_local_tool(
        tool: &Arc<dyn Tool>,
        arguments: serde_json::Value,
    ) -> ToolExecutionResult {
        let tool_name = tool.definition().id.clone();

        match tool.execute(arguments.clone()).await {
            Ok(result) => {
                info!(tool = %tool_name, "Local tool executed successfully");
                ToolExecutionResult {
                    tool_name,
                    success: true,
                    result,
                    error: None,
                }
            }
            Err(e) => {
                warn!(tool = %tool_name, error = %e, "Local tool execution failed");
                ToolExecutionResult {
                    tool_name,
                    success: false,
                    result: serde_json::json!({}),
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Executes an MCP tool and returns the result
    async fn execute_mcp_tool(
        mcp_manager: &MCPManager,
        server_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> ToolExecutionResult {
        let full_name = format!("{}:{}", server_name, tool_name);

        match mcp_manager
            .call_tool(server_name, tool_name, arguments)
            .await
        {
            Ok(result) => {
                if result.success {
                    info!(tool = %full_name, "MCP tool executed successfully");
                    ToolExecutionResult {
                        tool_name: full_name,
                        success: true,
                        result: result.content,
                        error: None,
                    }
                } else {
                    let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                    warn!(tool = %full_name, error = %error_msg, "MCP tool returned error");
                    ToolExecutionResult {
                        tool_name: full_name,
                        success: false,
                        result: serde_json::json!({}),
                        error: Some(error_msg),
                    }
                }
            }
            Err(e) => {
                warn!(tool = %full_name, error = %e, "MCP tool call failed");
                ToolExecutionResult {
                    tool_name: full_name,
                    success: false,
                    result: serde_json::json!({}),
                    error: Some(e.to_string()),
                }
            }
        }
    }

    /// Formats tool execution results for injection back to LLM
    fn format_tool_results(results: &[ToolExecutionResult]) -> String {
        results
            .iter()
            .map(|r| {
                if r.success {
                    format!(
                        "<tool_result name=\"{}\" success=\"true\">\n{}\n</tool_result>",
                        r.tool_name,
                        serde_json::to_string_pretty(&r.result).unwrap_or_default()
                    )
                } else {
                    format!(
                        "<tool_result name=\"{}\" success=\"false\">\nError: {}\n</tool_result>",
                        r.tool_name,
                        r.error.as_deref().unwrap_or("Unknown error")
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Strips tool calls from response text, leaving only the regular content
    fn strip_tool_calls(response: &str) -> String {
        let pattern =
            Regex::new(r#"<tool_call\s+name="[^"]+">[\s\S]*?</tool_call>"#).expect("Invalid regex");

        pattern.replace_all(response, "").trim().to_string()
    }

    /// Emits a streaming event to the frontend via Tauri.
    ///
    /// This is used to provide real-time progress updates during tool execution.
    /// If no app_handle is available in the agent context, the event is silently skipped.
    ///
    /// # Arguments
    /// * `workflow_id` - The workflow ID to associate with the event
    /// * `chunk` - The StreamChunk to emit
    fn emit_progress(&self, chunk: StreamChunk) {
        if let Some(ref context) = self.agent_context {
            if let Some(ref handle) = context.app_handle {
                if let Err(e) = handle.emit(events::WORKFLOW_STREAM, &chunk) {
                    warn!(
                        error = %e,
                        "Failed to emit LLM agent progress event"
                    );
                }
            }
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
                        tool_executions: vec![],
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
                    tool_executions: vec![],
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
                        tool_executions: vec![],
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
                        tool_executions: vec![],
                    },
                })
            }
        }
    }

    /// Executes a task with full tool support (local + MCP)
    ///
    /// This method implements a complete tool execution loop:
    /// 1. Creates local tool instances via ToolFactory
    /// 2. Discovers MCP tools from configured servers
    /// 3. Builds enhanced system prompt with tool definitions
    /// 4. Calls LLM with tool-aware prompt
    /// 5. Parses tool calls from response
    /// 6. Executes tools and feeds results back to LLM
    /// 7. Repeats until no tool calls or MAX_TOOL_ITERATIONS reached
    #[instrument(
        name = "llm_agent_execute_with_mcp",
        skip(self, task, mcp_manager),
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
    ) -> anyhow::Result<Report> {
        let start = std::time::Instant::now();
        let mut tools_used: Vec<String> = Vec::new();
        let mut mcp_calls_made: Vec<String> = Vec::new();
        let mut total_tokens_input: usize = 0;
        let mut total_tokens_output: usize = 0;
        let mut tool_executions_data: Vec<ToolExecutionData> = Vec::new();

        // Get provider type early to fail fast
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
                        tool_executions: vec![],
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
                    tool_executions: vec![],
                },
            });
        }

        // Extract workflow_id early for event emission
        let workflow_id = task
            .context
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Clone workflow_id for use in progress events (use task_id as fallback)
        let event_workflow_id = workflow_id.clone().unwrap_or_else(|| task.id.clone());

        // Check if this is the primary workflow agent
        // Sub-agents have "is_sub_agent": true in their context
        // Primary agents have "is_primary_agent": true in their context
        let is_primary_agent = task
            .context
            .get("is_primary_agent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let local_tools = self.create_local_tools(workflow_id, is_primary_agent);

        // Discover MCP tools and server summaries if manager is available
        // Note: get_mcp_tool_definitions uses self.config.mcp_servers (tools assigned to this agent)
        // but get_mcp_server_summaries returns ALL available servers (for sub-agent spawning)
        let (mcp_tools, mcp_server_summaries) = if let Some(ref mcp) = mcp_manager {
            // Get tools only from servers assigned to this agent
            let tools = if !self.config.mcp_servers.is_empty() {
                self.get_mcp_tool_definitions(mcp).await
            } else {
                Vec::new()
            };
            // Get ALL available servers (for documentation when spawning sub-agents)
            let summaries = self.get_mcp_server_summaries(mcp).await;
            (tools, summaries)
        } else {
            (Vec::new(), Vec::new())
        };

        // If no tools available at all, fall back to basic execute
        if local_tools.is_empty() && mcp_tools.is_empty() {
            debug!("No tools available, using basic execute");
            return self.execute(task).await;
        }

        debug!(
            agent_name = %self.config.name,
            local_tools_count = local_tools.len(),
            mcp_tools_count = mcp_tools.len(),
            mcp_servers_count = mcp_server_summaries.len(),
            "LLM Agent starting task execution with tool support"
        );

        // Build enhanced system prompt with tool definitions and MCP server info
        let system_prompt =
            self.build_system_prompt_with_tools(&local_tools, &mcp_tools, &mcp_server_summaries);

        // Build initial user prompt
        let base_prompt = self.build_prompt(&task);

        // Tool execution loop
        let mut conversation_history = vec![base_prompt];
        let mut final_response_content = String::new();
        let mut iteration = 0;

        // Use agent config max_tool_iterations, clamped to valid range [1, 200]
        let max_iterations = self.config.max_tool_iterations.clamp(1, 200);

        loop {
            iteration += 1;
            if iteration > max_iterations {
                warn!(
                    iterations = max_iterations,
                    "Max tool iterations reached, stopping execution"
                );
                // Emit progress event about max iterations
                self.emit_progress(StreamChunk::reasoning(
                    event_workflow_id.clone(),
                    format!("Max tool iterations ({}) reached, stopping execution", max_iterations),
                ));
                break;
            }

            // Emit progress event for iteration start
            if iteration > 1 {
                self.emit_progress(StreamChunk::reasoning(
                    event_workflow_id.clone(),
                    format!("Tool iteration {} - Processing tool results...", iteration),
                ));
            }

            // Build the full prompt from conversation history
            let full_prompt = conversation_history.join("\n\n");

            debug!(
                iteration = iteration,
                prompt_len = full_prompt.len(),
                "Executing LLM call"
            );

            // Execute LLM call
            let llm_result = self
                .provider_manager
                .complete_with_provider(
                    provider_type,
                    &full_prompt,
                    Some(&system_prompt),
                    Some(&self.config.llm.model),
                    self.config.llm.temperature,
                    self.config.llm.max_tokens,
                )
                .await;

            let response = match llm_result {
                Ok(r) => {
                    total_tokens_input += r.tokens_input;
                    total_tokens_output += r.tokens_output;
                    r
                }
                Err(e) => {
                    error!(error = %e, iteration = iteration, "LLM call failed");

                    let error_message = match &e {
                        LLMError::ConnectionError(msg) => {
                            format!("Connection error: {}\n\nMake sure the LLM service is running and accessible.", msg)
                        }
                        LLMError::ModelNotFound(msg) => format!("Model not found: {}", msg),
                        LLMError::MissingApiKey(provider) => {
                            format!(
                                "API key missing for {}. Please configure it in Settings.",
                                provider
                            )
                        }
                        LLMError::RequestFailed(msg) => format!("Request failed: {}", msg),
                        _ => e.to_string(),
                    };

                    return Ok(Report {
                        task_id: task.id,
                        status: ReportStatus::Failed,
                        content: format!(
                            "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\n\n{}",
                            self.config.id, task.description, error_message
                        ),
                        metrics: ReportMetrics {
                            duration_ms: start.elapsed().as_millis() as u64,
                            tokens_input: total_tokens_input,
                            tokens_output: total_tokens_output,
                            tools_used,
                            mcp_calls: mcp_calls_made,
                            tool_executions: tool_executions_data,
                        },
                    });
                }
            };

            // Parse tool calls from response
            let tool_calls = Self::parse_tool_calls(&response.content);

            if tool_calls.is_empty() {
                // No more tool calls, we're done
                final_response_content = Self::strip_tool_calls(&response.content);
                debug!(iteration = iteration, "No tool calls found, finishing");
                break;
            }

            info!(
                iteration = iteration,
                tool_calls_count = tool_calls.len(),
                "Found tool calls, executing"
            );

            // Emit progress event about found tool calls
            let tool_names: Vec<String> = tool_calls.iter().map(|c| c.tool_name.clone()).collect();
            self.emit_progress(StreamChunk::reasoning(
                event_workflow_id.clone(),
                format!(
                    "Executing {} tool(s): {}",
                    tool_calls.len(),
                    tool_names.join(", ")
                ),
            ));

            // Execute each tool call with detailed tracking
            let mut execution_results = Vec::new();

            for call in tool_calls {
                let exec_start = std::time::Instant::now();
                let input_params = call.arguments.clone();

                // Emit tool_start event
                self.emit_progress(StreamChunk::tool_start(
                    event_workflow_id.clone(),
                    call.tool_name.clone(),
                ));

                let result = if call.is_mcp {
                    // MCP tool execution
                    if let (Some(server), Some(tool)) = (&call.mcp_server, &call.mcp_tool) {
                        if let Some(ref mcp) = mcp_manager {
                            mcp_calls_made.push(call.tool_name.clone());
                            let exec_result =
                                Self::execute_mcp_tool(mcp, server, tool, call.arguments).await;

                            // Capture detailed execution data for MCP tool
                            let exec_duration = exec_start.elapsed().as_millis() as u64;
                            tool_executions_data.push(ToolExecutionData {
                                tool_type: "mcp".to_string(),
                                tool_name: tool.clone(),
                                server_name: Some(server.clone()),
                                input_params: input_params.clone(),
                                output_result: exec_result.result.clone(),
                                success: exec_result.success,
                                error_message: exec_result.error.clone(),
                                duration_ms: exec_duration,
                                iteration: iteration as u32,
                            });

                            exec_result
                        } else {
                            let exec_result = ToolExecutionResult {
                                tool_name: call.tool_name.clone(),
                                success: false,
                                result: serde_json::json!({}),
                                error: Some("MCP manager not available".to_string()),
                            };

                            // Capture failed execution data
                            let exec_duration = exec_start.elapsed().as_millis() as u64;
                            tool_executions_data.push(ToolExecutionData {
                                tool_type: "mcp".to_string(),
                                tool_name: call.tool_name.clone(),
                                server_name: call.mcp_server.clone(),
                                input_params: input_params.clone(),
                                output_result: exec_result.result.clone(),
                                success: false,
                                error_message: exec_result.error.clone(),
                                duration_ms: exec_duration,
                                iteration: iteration as u32,
                            });

                            exec_result
                        }
                    } else {
                        let exec_result = ToolExecutionResult {
                            tool_name: call.tool_name.clone(),
                            success: false,
                            result: serde_json::json!({}),
                            error: Some("Invalid MCP tool format".to_string()),
                        };

                        // Capture failed execution data
                        let exec_duration = exec_start.elapsed().as_millis() as u64;
                        tool_executions_data.push(ToolExecutionData {
                            tool_type: "mcp".to_string(),
                            tool_name: call.tool_name.clone(),
                            server_name: None,
                            input_params: input_params.clone(),
                            output_result: exec_result.result.clone(),
                            success: false,
                            error_message: exec_result.error.clone(),
                            duration_ms: exec_duration,
                            iteration: iteration as u32,
                        });

                        exec_result
                    }
                } else {
                    // Local tool execution
                    let matching_tool = local_tools
                        .iter()
                        .find(|t| t.definition().id == call.tool_name);

                    if let Some(tool) = matching_tool {
                        tools_used.push(call.tool_name.clone());
                        let exec_result = Self::execute_local_tool(tool, call.arguments).await;

                        // Capture detailed execution data for local tool
                        let exec_duration = exec_start.elapsed().as_millis() as u64;
                        tool_executions_data.push(ToolExecutionData {
                            tool_type: "local".to_string(),
                            tool_name: call.tool_name.clone(),
                            server_name: None,
                            input_params: input_params.clone(),
                            output_result: exec_result.result.clone(),
                            success: exec_result.success,
                            error_message: exec_result.error.clone(),
                            duration_ms: exec_duration,
                            iteration: iteration as u32,
                        });

                        exec_result
                    } else {
                        let exec_result = ToolExecutionResult {
                            tool_name: call.tool_name.clone(),
                            success: false,
                            result: serde_json::json!({}),
                            error: Some(format!(
                                "Unknown tool '{}'. Available tools: {}",
                                call.tool_name,
                                local_tools
                                    .iter()
                                    .map(|t| t.definition().id)
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )),
                        };

                        // Capture failed execution data
                        let exec_duration = exec_start.elapsed().as_millis() as u64;
                        tool_executions_data.push(ToolExecutionData {
                            tool_type: "local".to_string(),
                            tool_name: call.tool_name.clone(),
                            server_name: None,
                            input_params: input_params.clone(),
                            output_result: exec_result.result.clone(),
                            success: false,
                            error_message: exec_result.error.clone(),
                            duration_ms: exec_duration,
                            iteration: iteration as u32,
                        });

                        exec_result
                    }
                };

                // Calculate duration and emit tool_end event
                let tool_duration = exec_start.elapsed().as_millis() as u64;
                self.emit_progress(StreamChunk::tool_end(
                    event_workflow_id.clone(),
                    call.tool_name.clone(),
                    tool_duration,
                ));

                execution_results.push(result);
            }

            // Format results and add to conversation
            let results_text = Self::format_tool_results(&execution_results);
            let clean_response = Self::strip_tool_calls(&response.content);

            // Build continuation context for next iteration
            // Mistral API requires the last message to be from "user" or "tool" role.
            // We structure the prompt as a user request with embedded context to satisfy this constraint.
            // Pattern from CrewAI/AutoGen: append a user continuation message after tool results.
            let continuation = format!(
                "---\n\
                 Context from previous step:\n\
                 {}\n\n\
                 Tool execution results:\n\
                 {}\n\
                 ---\n\n\
                 Based on the tool results above, please continue with the task.",
                if clean_response.is_empty() {
                    "(No additional text)".to_string()
                } else {
                    clean_response
                },
                results_text
            );
            conversation_history.push(continuation);
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            iterations = iteration,
            tools_used_count = tools_used.len(),
            mcp_calls_count = mcp_calls_made.len(),
            total_tokens_input = total_tokens_input,
            total_tokens_output = total_tokens_output,
            duration_ms = duration_ms,
            "LLM Agent task execution with tools completed"
        );

        // Build tools section for report
        let tools_section = if !tools_used.is_empty() || !mcp_calls_made.is_empty() {
            let local_used = if !tools_used.is_empty() {
                format!(
                    "\n### Local Tools Used\n{}",
                    tools_used
                        .iter()
                        .map(|t| format!("- {}", t))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            } else {
                String::new()
            };

            let mcp_used = if !mcp_calls_made.is_empty() {
                format!(
                    "\n### MCP Tools Called\n{}",
                    mcp_calls_made
                        .iter()
                        .map(|t| format!("- {}", t))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            } else {
                String::new()
            };

            format!("\n\n## Tool Usage{}{}", local_used, mcp_used)
        } else {
            String::new()
        };

        let content = format!(
            "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Success\n\n## Response\n\n{}\n\n## Metrics\n- Provider: {}\n- Model: {}\n- Tokens (input/output): {}/{}\n- Duration: {}ms\n- Tool iterations: {}{}",
            self.config.id,
            task.description,
            final_response_content,
            provider_type,
            self.config.llm.model,
            total_tokens_input,
            total_tokens_output,
            duration_ms,
            iteration,
            tools_section
        );

        Ok(Report {
            task_id: task.id,
            status: ReportStatus::Success,
            content,
            metrics: ReportMetrics {
                duration_ms,
                tokens_input: total_tokens_input,
                tokens_output: total_tokens_output,
                tools_used,
                mcp_calls: mcp_calls_made,
                tool_executions: tool_executions_data,
            },
        })
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
            max_tool_iterations: 50,
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

        // Test with conversation history
        let task_with_history = Task {
            id: "task3".to_string(),
            description: "What did we discuss?".to_string(),
            context: serde_json::json!({
                "conversation_history": [
                    {"role": "user", "content": "Hello"},
                    {"role": "assistant", "content": "Hi there!"}
                ]
            }),
        };
        let prompt_with_history = agent.build_prompt(&task_with_history);
        assert!(prompt_with_history.contains("Conversation Context"));
        assert!(prompt_with_history.contains("[Human]"));
        assert!(prompt_with_history.contains("Hello"));
        assert!(prompt_with_history.contains("[AI Response]"));
        assert!(prompt_with_history.contains("Hi there!"));
        assert!(prompt_with_history.contains("What did we discuss?"));
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

    #[test]
    fn test_parse_tool_calls_single_local() {
        let response = r#"I will use the MemoryTool to store this.

<tool_call name="MemoryTool">
{"operation": "add", "type": "knowledge", "content": "Important fact"}
</tool_call>

Let me know if you need anything else."#;

        let calls = LLMAgent::parse_tool_calls(response);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "MemoryTool");
        assert!(!calls[0].is_mcp);
        assert_eq!(calls[0].arguments["operation"], "add");
        assert_eq!(calls[0].arguments["type"], "knowledge");
    }

    #[test]
    fn test_parse_tool_calls_mcp() {
        let response = r#"I will search for the symbol.

<tool_call name="serena:find_symbol">
{"name_path_pattern": "MyClass", "include_body": true}
</tool_call>"#;

        let calls = LLMAgent::parse_tool_calls(response);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].tool_name, "serena:find_symbol");
        assert!(calls[0].is_mcp);
        assert_eq!(calls[0].mcp_server, Some("serena".to_string()));
        assert_eq!(calls[0].mcp_tool, Some("find_symbol".to_string()));
    }

    #[test]
    fn test_parse_tool_calls_multiple() {
        let response = r#"Let me create tasks and store memory.

<tool_call name="TodoTool">
{"operation": "create", "name": "Analyze code", "priority": 1}
</tool_call>

Now I'll store that for later.

<tool_call name="MemoryTool">
{"operation": "add", "type": "context", "content": "Task created"}
</tool_call>"#;

        let calls = LLMAgent::parse_tool_calls(response);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].tool_name, "TodoTool");
        assert_eq!(calls[1].tool_name, "MemoryTool");
    }

    #[test]
    fn test_parse_tool_calls_none() {
        let response = "I don't need to use any tools for this response.";

        let calls = LLMAgent::parse_tool_calls(response);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_parse_tool_calls_invalid_json() {
        let response = r#"<tool_call name="MemoryTool">
{invalid json here}
</tool_call>"#;

        let calls = LLMAgent::parse_tool_calls(response);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_format_tool_results_success() {
        let results = vec![ToolExecutionResult {
            tool_name: "MemoryTool".to_string(),
            success: true,
            result: serde_json::json!({"success": true, "memory_id": "abc123"}),
            error: None,
        }];

        let formatted = LLMAgent::format_tool_results(&results);
        assert!(formatted.contains("MemoryTool"));
        assert!(formatted.contains("success=\"true\""));
        assert!(formatted.contains("abc123"));
    }

    #[test]
    fn test_format_tool_results_failure() {
        let results = vec![ToolExecutionResult {
            tool_name: "TodoTool".to_string(),
            success: false,
            result: serde_json::json!({}),
            error: Some("Task not found".to_string()),
        }];

        let formatted = LLMAgent::format_tool_results(&results);
        assert!(formatted.contains("TodoTool"));
        assert!(formatted.contains("success=\"false\""));
        assert!(formatted.contains("Task not found"));
    }

    #[test]
    fn test_strip_tool_calls() {
        let response = r#"I will help you with that.

<tool_call name="MemoryTool">
{"operation": "add"}
</tool_call>

Here is some more text after the tool call."#;

        let stripped = LLMAgent::strip_tool_calls(response);
        assert!(!stripped.contains("<tool_call"));
        assert!(!stripped.contains("</tool_call>"));
        assert!(stripped.contains("I will help you with that"));
        assert!(stripped.contains("Here is some more text"));
    }

    #[test]
    fn test_strip_tool_calls_multiple() {
        let response = r#"First part.

<tool_call name="Tool1">
{"a": 1}
</tool_call>

Middle.

<tool_call name="Tool2">
{"b": 2}
</tool_call>

End."#;

        let stripped = LLMAgent::strip_tool_calls(response);
        assert!(stripped.contains("First part"));
        assert!(stripped.contains("Middle"));
        assert!(stripped.contains("End"));
        assert!(!stripped.contains("tool_call"));
    }
}
