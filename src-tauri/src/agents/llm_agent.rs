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
    Agent, ReasoningSource, ReasoningStepData, Report, ReportMetrics, ReportStatus, Task,
    ToolExecutionData,
};
use crate::db::DBClient;
use crate::llm::adapters::{MistralToolAdapter, OllamaToolAdapter, OpenAiToolAdapter};
use crate::llm::tool_adapter::ProviderToolAdapter;
use crate::llm::{LLMError, ProviderManager, ProviderType};
use crate::mcp::MCPManager;
use crate::models::function_calling::{FunctionCall, FunctionCallResult, ToolChoiceMode};
use crate::models::mcp::MCPTool;
use crate::models::streaming::{events, StreamChunk};
use crate::models::workflow::IterationMetrics;
use crate::models::{AgentConfig, Lifecycle};
use crate::tools::{
    context::AgentToolContext,
    validation_helper::{is_destructive_file_op, ValidationHelper},
    Tool, ToolDefinition, ToolFactory,
};
use async_trait::async_trait;
use chrono::Local;
use std::sync::Arc;
use tauri::Emitter;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn};

/// Default maximum number of tool execution iterations to prevent infinite loops
/// Can be overridden per-agent via AgentConfig.max_tool_iterations
#[allow(dead_code)]
const DEFAULT_MAX_TOOL_ITERATIONS: usize = 50;

/// Prompt sent to the LLM when a generic completion message is detected,
/// requesting a proper markdown report of what was accomplished.
const REPORT_ENFORCEMENT_PROMPT: &str = "You have completed your task using tools. However, you did not provide a summary of what you accomplished. Please provide a concise report in markdown format that describes:\n\n1. What actions you performed\n2. The key results or outcomes\n3. Any important details the user should know\n\nBe specific and reference the actual work done based on the tool calls you made. Respond in the same language as the original task.";

/// Checks whether the given response content is a generic completion message
/// (i.e., the LLM did not provide a meaningful report).
///
/// Returns `true` if the content matches known generic fallback patterns,
/// indicating that a follow-up report request should be made.
fn is_generic_completion_message(content: &str) -> bool {
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

/// Summary of an MCP server for documentation in system prompt
///
/// Used to provide high-level information about available MCP servers
/// so the agent can make informed decisions when spawning sub-agents.
#[derive(Debug, Clone)]
struct MCPServerSummary {
    /// Human-readable server name (used as identifier in mcp_servers parameter)
    name: String,
    /// Description of what the server does
    description: Option<String>,
    /// Number of tools available from this server
    tools_count: usize,
    /// Whether this agent has direct access to this server
    has_direct_access: bool,
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
        // Create a new empty embedding service reference (no embedding by default)
        let embedding_service = Arc::new(tokio::sync::RwLock::new(None));
        let tool_factory = Arc::new(ToolFactory::new(db, embedding_service));
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

        // Create a set of server names that this agent has direct access to
        let direct_access: std::collections::HashSet<&String> =
            self.config.mcp_servers.iter().collect();

        for server in all_servers {
            // Only include enabled servers that are running
            if server.config.enabled
                && server.status == crate::models::mcp::MCPServerStatus::Running
            {
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

    /// Creates local tool instances for configured tools
    ///
    /// When `is_primary_agent` is true and `agent_context` is available,
    /// this method will also create sub-agent tools (SpawnAgentTool,
    /// DelegateTaskTool, ParallelTasksTool) in addition to basic tools.
    ///
    /// # Arguments
    /// * `workflow_id` - Optional workflow ID for scoping tool operations
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    /// * `context_override` - Optional context to use instead of `self.agent_context`.
    ///   This allows injecting a modified context (e.g., with a cancellation token)
    ///   without mutating the agent.
    async fn create_local_tools(
        &self,
        workflow_id: Option<String>,
        is_primary_agent: bool,
        context_override: Option<&AgentToolContext>,
    ) -> Vec<Arc<dyn Tool>> {
        let Some(ref factory) = self.tool_factory else {
            return Vec::new();
        };

        // Use override if provided, otherwise fall back to self.agent_context
        let effective_context = context_override.or(self.agent_context.as_ref());

        // Extract app_handle from context if available
        let app_handle = effective_context.and_then(|ctx| ctx.app_handle.clone());

        // Auto-inject ReadSkillTool when agent has skills assigned
        let mut tool_names: Vec<String> = self.config.tools.clone();
        if !self.config.skills.is_empty() && !tool_names.iter().any(|t| t == "ReadSkillTool") {
            debug!(
                agent_id = %self.config.id,
                skills_count = self.config.skills.len(),
                "Auto-injecting ReadSkillTool for agent with skills"
            );
            tool_names.push("ReadSkillTool".to_string());
        }

        // If this is the primary agent and we have context, use create_tools_with_context
        // to include sub-agent tools
        if is_primary_agent {
            if let Some(context) = effective_context {
                debug!(
                    agent_id = %self.config.id,
                    "Creating tools with context for primary agent (sub-agent tools available)"
                );
                return factory
                    .create_tools_with_context(
                        &tool_names,
                        workflow_id,
                        self.config.id.clone(),
                        Some(context.clone()),
                        true, // is_primary_agent
                    )
                    .await;
            }
        }

        // For sub-agents or agents without context, use basic tool creation
        debug!(
            agent_id = %self.config.id,
            is_primary_agent = is_primary_agent,
            has_context = effective_context.is_some(),
            "Creating basic tools (sub-agent tools NOT available)"
        );
        factory
            .create_tools(&tool_names, workflow_id, self.config.id.clone(), app_handle)
            .await
    }

    /// Builds enhanced system prompt for JSON function calling
    ///
    /// With JSON function calling (OpenAI standard), tool definitions are passed
    /// via the API's `tools` parameter, NOT in the system prompt. This method
    /// builds a simplified prompt that includes:
    /// - The agent's base system prompt
    /// - Context about available tools (names only, schemas are in API)
    /// - Available MCP servers for sub-agent delegation
    /// - Current date/time and user's selected language
    ///
    /// NOTE: No XML instructions! The LLM uses native JSON function calling.
    fn build_system_prompt_with_tools(
        &self,
        local_tools: &[Arc<dyn Tool>],
        mcp_tools: &[(String, MCPTool)],
        mcp_server_summaries: &[MCPServerSummary],
        locale: Option<&str>,
    ) -> String {
        let mut sections = vec![self.config.system_prompt.clone()];

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

        // List local tools briefly
        if !local_tools.is_empty() {
            tools_context.push_str("\n### Local Tools\n");
            for tool in local_tools {
                let def = tool.definition();
                tools_context.push_str(&format!("- **{}**: {}\n", def.name, def.description));
            }
        }

        // List MCP tools briefly with naming convention
        if !mcp_tools.is_empty() {
            tools_context.push_str("\n### MCP Tools (Direct Access)\n");
            tools_context.push_str(
                "MCP tools use the naming format `mcp__server__tool`. Use them directly.\n",
            );
            for (server_name, tool) in mcp_tools {
                tools_context.push_str(&format!(
                    "- **mcp__{}__{}**: {}\n",
                    server_name, tool.name, tool.description
                ));
            }
        }

        sections.push(tools_context);

        // Add skills context if agent has skills assigned
        if !self.config.skills.is_empty() {
            let mut skills_section = String::from("## Available Skills\n\n");
            skills_section.push_str(
                "You have the following skills assigned. Use the ReadSkill tool to read their content before performing related tasks.\n\n",
            );
            for skill_name in &self.config.skills {
                skills_section.push_str(&format!("- `{}`\n", skill_name));
            }
            skills_section.push_str(
                "\nUse `ReadSkill` with `{\"operation\": \"list\"}` for descriptions, or `{\"name\": \"skill-name\"}` to read content.",
            );
            sections.push(skills_section);
        }

        // Add agent configuration context (provider, model, available resources)
        // This helps the LLM make informed decisions when spawning sub-agents
        let now = Local::now();

        // Convert locale code to full language name for clarity
        let language_display = match locale {
            Some("fr") => "French (Francais)",
            Some("en") => "English",
            Some(code) => code, // Fallback to code if unknown
            None => "English",  // Default
        };

        let mut config_section = format!(
            r#"## Your Configuration

**Current Date and Time**: {} (local timezone)
**User Language**: {} - Always respond in this language unless explicitly asked otherwise.

You are currently running with the following configuration:
- **Provider**: {}
- **Model**: {}"#,
            now.format("%A %d %B %Y, %H:%M:%S"),
            language_display,
            self.config.llm.provider,
            self.config.llm.model,
        );

        // Add detailed MCP server information with descriptions
        if mcp_server_summaries.is_empty() {
            config_section.push_str("\n- **Available MCP Servers**: None configured or running");
        } else {
            config_section.push_str("\n\n### Available MCP Servers for Delegation\n");
            config_section.push_str(
                "These servers can be assigned to sub-agents using the `mcp_servers` parameter (use server name).\n",
            );
            config_section.push_str(
                "**Note**: If you already have direct access to an MCP (listed in 'MCP Tools' above), use it directly instead of delegating.\n",
            );

            for server in mcp_server_summaries {
                let access_marker = if server.has_direct_access {
                    "[DIRECT]"
                } else {
                    "[DELEGATE]"
                };
                config_section.push_str(&format!(
                    "\n- **{}** {} - {} - {} tools\n",
                    server.name,
                    access_marker,
                    server.description.as_deref().unwrap_or("No description"),
                    server.tools_count
                ));
            }

            // Add usage example
            config_section.push_str("\n\n**Example**: To assign MCP servers to sub-agents:\n");
            config_section
                .push_str("```json\n{\"mcp_servers\": [\"Serena\", \"Context7\"]}\n```\n");
        }

        config_section.push_str(
            "\n\nWhen spawning sub-agents, you can specify provider/model/mcp_servers or let them inherit from your configuration.",
        );
        sections.push(config_section);

        sections.join("\n\n")
    }

    // =========================================================================
    // JSON Function Calling Helpers (replacing XML-based tool calling)
    // =========================================================================

    /// Collects all tool definitions from local tools and MCP tools.
    ///
    /// Creates ToolDefinition structs for all available tools so they can
    /// be formatted by the provider adapter for JSON function calling.
    fn collect_tool_definitions(
        &self,
        local_tools: &[Arc<dyn Tool>],
        mcp_tools: &[(String, MCPTool)],
    ) -> Vec<ToolDefinition> {
        let mut definitions = Vec::new();

        // Add local tool definitions
        for tool in local_tools {
            definitions.push(tool.definition());
        }

        // Add MCP tool definitions with mcp__server__tool naming
        for (server_name, mcp_tool) in mcp_tools {
            definitions.push(ToolDefinition {
                id: format!("mcp__{}__{}", server_name, mcp_tool.name),
                name: mcp_tool.name.clone(),
                description: mcp_tool.description.clone(),
                input_schema: mcp_tool.input_schema.clone(),
                output_schema: serde_json::json!({}),
                requires_confirmation: false,
            });
        }

        definitions
    }

    /// Executes a single function call (local or MCP tool).
    ///
    /// # Arguments
    /// * `call` - The function call to execute
    /// * `local_tools` - Available local tools
    /// * `mcp_manager` - Optional MCP manager for MCP tools
    /// * `tools_used` - Mutable vector to track local tool usage
    /// * `mcp_calls_made` - Mutable vector to track MCP tool calls
    /// * `workflow_id` - Workflow ID for validation tracking
    /// * `validation_helper` - Optional validation helper for human-in-the-loop
    #[allow(clippy::too_many_arguments)]
    async fn execute_function_call(
        &self,
        call: &FunctionCall,
        local_tools: &[Arc<dyn Tool>],
        mcp_manager: Option<&Arc<MCPManager>>,
        tools_used: &mut Vec<String>,
        mcp_calls_made: &mut Vec<String>,
        workflow_id: &str,
        validation_helper: Option<&ValidationHelper>,
    ) -> FunctionCallResult {
        let start = std::time::Instant::now();

        // Check if MCP tool
        if let Some((server, tool)) = call.parse_mcp_name() {
            // Execute via MCP
            if let Some(mcp) = mcp_manager {
                mcp_calls_made.push(call.name.clone());

                // Request validation for MCP tool call
                if let Some(helper) = validation_helper {
                    if let Err(e) = helper
                        .request_mcp_validation(workflow_id, server, tool, call.arguments.clone())
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
                            let error_msg =
                                result.error.unwrap_or_else(|| "Unknown error".to_string());
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
            let matching_tool = local_tools.iter().find(|t| t.definition().id == call.name);

            if let Some(tool) = matching_tool {
                tools_used.push(call.name.clone());

                // Request validation for local tool
                // Skip validation for sub-agent tools (they have their own validation)
                let is_sub_agent_tool = call.name == "SpawnAgentTool"
                    || call.name == "DelegateTaskTool"
                    || call.name == "ParallelTasksTool";

                if !is_sub_agent_tool {
                    // FileManagerTool: use file-specific validation if destructive + confirmation enabled
                    if call.name == "FileManagerTool" && self.config.require_file_confirmation {
                        let operation = call
                            .arguments
                            .get("operation")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        if is_destructive_file_op(operation) {
                            if let Some(helper) = validation_helper {
                                let path = call
                                    .arguments
                                    .get("path")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");

                                if let Err(e) = helper
                                    .request_file_validation(
                                        workflow_id,
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
                    } else if let Some(helper) = validation_helper {
                        // Standard tool validation for non-FileManagerTool
                        let operation = call
                            .arguments
                            .get("operation")
                            .and_then(|v| v.as_str())
                            .unwrap_or("execute");

                        if let Err(e) = helper
                            .request_tool_validation(
                                workflow_id,
                                &call.name,
                                operation,
                                call.arguments.clone(),
                            )
                            .await
                        {
                            warn!(tool = %call.name, error = %e, "Tool validation rejected");
                            return FunctionCallResult::failure(
                                &call.id,
                                &call.name,
                                e.to_string(),
                            );
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
                let available_tools: Vec<String> = local_tools
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
                let error_msg = format!("Invalid provider configuration: {}", e);
                return Ok(Report {
                    task_id: task.id.clone(),
                    status: ReportStatus::Failed,
                    content: format!(
                        "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\n{}",
                        self.config.id, task.description, error_msg
                    ),
                    response: error_msg,
                    metrics: ReportMetrics {
                        duration_ms: start.elapsed().as_millis() as u64,
                        tokens_input: 0,
                        tokens_output: 0,
                        context_tokens: 0,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        tools_used: vec![],
                        mcp_calls: vec![],
                        tool_executions: vec![],
                        reasoning_steps: vec![],
                        iteration_metrics: vec![],
                    },
                    system_prompt: None,
                    tools_json: None,
                });
            }
        };

        // Check if provider is configured
        if !self
            .provider_manager
            .is_provider_configured(provider_type.clone())
        {
            warn!(
                ?provider_type,
                "Provider not configured, returning configuration error"
            );
            let error_msg = format!(
                "LLM provider '{}' is not configured. Please configure it in Settings.",
                provider_type
            );
            return Ok(Report {
                task_id: task.id.clone(),
                status: ReportStatus::Failed,
                content: format!(
                    "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\n{}",
                    self.config.id, task.description, error_msg
                ),
                response: error_msg,
                metrics: ReportMetrics {
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_input: 0,
                    tokens_output: 0,
                    context_tokens: 0,
                    cached_tokens: None,
                    cache_write_tokens: None,
                    tools_used: vec![],
                    mcp_calls: vec![],
                    tool_executions: vec![],
                    reasoning_steps: vec![],
                    iteration_metrics: vec![],
                },
                system_prompt: None,
                tools_json: None,
            });
        }

        // Execute LLM call
        let llm_result = self
            .provider_manager
            .complete_with_provider(
                provider_type.clone(),
                &prompt,
                Some(&self.config.system_prompt),
                Some(&self.config.llm.model),
                self.config.llm.temperature,
                self.config.llm.max_tokens,
                self.config.llm.is_reasoning,
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
                    response: response.content.clone(),
                    metrics: ReportMetrics {
                        duration_ms,
                        tokens_input: response.tokens_input,
                        tokens_output: response.tokens_output,
                        context_tokens: response.tokens_input,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        tools_used: vec![],
                        mcp_calls: vec![],
                        tool_executions: vec![],
                        reasoning_steps: vec![],
                        iteration_metrics: vec![],
                    },
                    system_prompt: None,
                    tools_json: None,
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
                    response: error_message,
                    metrics: ReportMetrics {
                        duration_ms,
                        tokens_input: 0,
                        tokens_output: 0,
                        context_tokens: 0,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        tools_used: vec![],
                        mcp_calls: vec![],
                        tool_executions: vec![],
                        reasoning_steps: vec![],
                        iteration_metrics: vec![],
                    },
                    system_prompt: None,
                    tools_json: None,
                })
            }
        }
    }

    /// Executes a task with full tool support (local + MCP) using JSON function calling.
    ///
    /// This method implements a complete tool execution loop using native JSON function
    /// calling supported by Mistral and Ollama APIs (replacing the old XML-based approach):
    /// 1. Creates local tool instances via ToolFactory
    /// 2. Discovers MCP tools from configured servers
    /// 3. Formats tool definitions via provider adapter
    /// 4. Calls LLM with tools parameter
    /// 5. Parses tool_calls from JSON response
    /// 6. Executes tools and sends results back to LLM
    /// 7. Repeats until no tool calls or MAX_TOOL_ITERATIONS reached
    #[instrument(
        name = "llm_agent_execute_with_mcp",
        skip(self, task, mcp_manager, cancellation_token),
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
        cancellation_token: Option<CancellationToken>,
    ) -> anyhow::Result<Report> {
        let start = std::time::Instant::now();
        let mut tools_used: Vec<String> = Vec::new();
        let mut mcp_calls_made: Vec<String> = Vec::new();
        let mut total_tokens_input: usize = 0; // Cumulative input tokens (total billed)
        let mut total_tokens_output: usize = 0;
        let mut context_tokens: usize = 0; // Last call's input tokens (context window size)
        let mut total_cached_tokens: Option<usize> = None;
        let mut total_cache_write_tokens: Option<usize> = None;
        // Per-iteration values (saved from match arm for IterationMetrics)
        let mut iter_input_tokens: usize = 0;
        let mut iter_output_tokens: usize = 0;
        let mut iter_cached_tokens: Option<usize> = None;
        let mut iter_cache_write_tokens: Option<usize> = None;
        let mut iteration_metrics_data: Vec<IterationMetrics> = Vec::new();
        let mut tool_executions_data: Vec<ToolExecutionData> = Vec::new();
        let mut reasoning_steps_data: Vec<ReasoningStepData> = Vec::new();

        // Get provider type early to fail fast
        let provider_type = match self.get_provider_type() {
            Ok(pt) => pt,
            Err(e) => {
                error!(error = %e, "Invalid provider type in config");
                let error_msg = format!("Invalid provider configuration: {}", e);
                return Ok(Report {
                    task_id: task.id.clone(),
                    status: ReportStatus::Failed,
                    content: format!(
                        "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\n{}",
                        self.config.id, task.description, error_msg
                    ),
                    response: error_msg,
                    metrics: ReportMetrics {
                        duration_ms: start.elapsed().as_millis() as u64,
                        tokens_input: 0,
                        tokens_output: 0,
                        context_tokens: 0,
                        cached_tokens: None,
                        cache_write_tokens: None,
                        tools_used: vec![],
                        mcp_calls: vec![],
                        tool_executions: vec![],
                        reasoning_steps: vec![],
                        iteration_metrics: vec![],
                    },
                    system_prompt: None,
                    tools_json: None,
                });
            }
        };

        // Check if provider is configured
        if !self
            .provider_manager
            .is_provider_configured(provider_type.clone())
        {
            warn!(
                ?provider_type,
                "Provider not configured, returning configuration error"
            );
            let error_msg = format!(
                "LLM provider '{}' is not configured. Please configure it in Settings.",
                provider_type
            );
            return Ok(Report {
                task_id: task.id.clone(),
                status: ReportStatus::Failed,
                content: format!(
                    "# Agent Report: {}\n\n**Task**: {}\n\n**Status**: Failed\n\n## Error\n{}",
                    self.config.id, task.description, error_msg
                ),
                response: error_msg,
                metrics: ReportMetrics {
                    duration_ms: start.elapsed().as_millis() as u64,
                    tokens_input: 0,
                    tokens_output: 0,
                    context_tokens: 0,
                    cached_tokens: None,
                    cache_write_tokens: None,
                    tools_used: vec![],
                    mcp_calls: vec![],
                    tool_executions: vec![],
                    reasoning_steps: vec![],
                    iteration_metrics: vec![],
                },
                system_prompt: None,
                tools_json: None,
            });
        }

        // Get the adapter based on provider type for JSON function calling
        let adapter: Box<dyn ProviderToolAdapter> = match provider_type {
            ProviderType::Mistral => Box::new(MistralToolAdapter::new()),
            ProviderType::Ollama => Box::new(OllamaToolAdapter::new()),
            ProviderType::Custom(_) => Box::new(OpenAiToolAdapter::new()),
        };

        // Extract workflow_id early for event emission
        let workflow_id = task
            .context
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Clone workflow_id for use in progress events (use task_id as fallback)
        let event_workflow_id = workflow_id.clone().unwrap_or_else(|| task.id.clone());

        // Create validation helper for human-in-the-loop validation
        // Uses db from tool_factory, app_handle from agent_context (or factory as fallback)
        let validation_helper = if let Some(factory) = self.tool_factory.as_ref() {
            let db = factory.get_db();
            // Try agent_context first, then fallback to tool_factory's app_handle
            let app_handle = match self
                .agent_context
                .as_ref()
                .and_then(|ctx| ctx.app_handle.clone())
            {
                Some(handle) => Some(handle),
                None => factory.get_app_handle().await,
            };
            Some(ValidationHelper::new(db, app_handle))
        } else {
            None
        };

        // Check if this is the primary workflow agent
        let is_primary_agent = task
            .context
            .get("is_primary_agent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Extract user's selected locale for system prompt language instruction
        let locale = task
            .context
            .get("locale")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Inject cancellation token into context if both are available.
        // This ensures sub-agent tools receive the workflow's cancellation token
        // and can propagate it to SubAgentExecutor for graceful shutdown.
        let effective_context = match (&self.agent_context, &cancellation_token) {
            (Some(ctx), Some(token)) => Some(ctx.clone().with_cancellation_token(token.clone())),
            _ => None,
        };

        let local_tools = self
            .create_local_tools(workflow_id, is_primary_agent, effective_context.as_ref())
            .await;

        // Discover MCP tools and server summaries if manager is available
        let (mcp_tools, mcp_server_summaries) = if let Some(ref mcp) = mcp_manager {
            let tools = if !self.config.mcp_servers.is_empty() {
                self.get_mcp_tool_definitions(mcp).await
            } else {
                Vec::new()
            };
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
            provider = adapter.provider_name(),
            local_tools_count = local_tools.len(),
            mcp_tools_count = mcp_tools.len(),
            mcp_servers_count = mcp_server_summaries.len(),
            "LLM Agent starting task execution with JSON function calling"
        );

        // Collect tool definitions and format for API
        let tool_definitions = self.collect_tool_definitions(&local_tools, &mcp_tools);
        let tools_json = adapter.format_tools(&tool_definitions);

        // Check if we have existing conversation messages (continuation of workflow)
        let existing_messages = task
            .context
            .get("conversation_messages")
            .and_then(|v| v.as_array())
            .cloned();

        // Track if this is the first message (need to return system_prompt for persistence)
        let is_first_message = existing_messages.is_none();

        // Initialize messages array for JSON function calling
        let (mut messages, system_prompt_for_report): (Vec<serde_json::Value>, Option<String>) =
            if let Some(existing) = existing_messages {
                // Continuation: use existing messages (already contains system prompt)
                // Just add the new user message
                let mut msgs: Vec<serde_json::Value> = existing;
                msgs.push(serde_json::json!({
                    "role": "user",
                    "content": task.description
                }));
                debug!(
                    existing_messages_count = msgs.len() - 1,
                    "Continuing conversation with existing context"
                );
                (msgs, None)
            } else {
                // First message: build system prompt and initial messages
                let system_prompt = self.build_system_prompt_with_tools(
                    &local_tools,
                    &mcp_tools,
                    &mcp_server_summaries,
                    locale.as_deref(),
                );
                let base_prompt = self.build_prompt(&task);
                let msgs = vec![
                    serde_json::json!({"role": "system", "content": system_prompt}),
                    serde_json::json!({"role": "user", "content": base_prompt}),
                ];
                debug!("First message: building new system prompt with tools");
                (msgs, Some(system_prompt))
            };

        // Tool execution loop
        let mut final_response_content = String::new();
        let mut iteration = 0;
        let mut global_sequence: u32 = 0;

        // Use agent config max_tool_iterations, clamped to valid range [1, 200]
        let max_iterations = self.config.max_tool_iterations.clamp(1, 200);

        loop {
            iteration += 1;
            let iter_start = std::time::Instant::now();
            if iteration > max_iterations {
                warn!(
                    iterations = max_iterations,
                    "Max tool iterations reached, stopping execution"
                );
                let reasoning_content = format!(
                    "Max tool iterations ({}) reached, stopping execution",
                    max_iterations
                );
                global_sequence += 1;
                self.emit_progress(StreamChunk::reasoning(
                    event_workflow_id.clone(),
                    reasoning_content.clone(),
                ));
                reasoning_steps_data.push(ReasoningStepData {
                    content: reasoning_content,
                    duration_ms: start.elapsed().as_millis() as u64,
                    sequence: global_sequence,
                    source: ReasoningSource::AgentFlow,
                });
                break;
            }

            // Emit progress event for iteration start
            if iteration > 1 {
                let reasoning_content =
                    format!("Tool iteration {} - Processing tool results...", iteration);
                global_sequence += 1;
                self.emit_progress(StreamChunk::reasoning(
                    event_workflow_id.clone(),
                    reasoning_content.clone(),
                ));
                reasoning_steps_data.push(ReasoningStepData {
                    content: reasoning_content,
                    duration_ms: start.elapsed().as_millis() as u64,
                    sequence: global_sequence,
                    source: ReasoningSource::AgentFlow,
                });
            }

            debug!(
                iteration = iteration,
                messages_count = messages.len(),
                "Executing LLM call with JSON function calling"
            );

            // Execute LLM call with tools via JSON function calling API
            let response = match self
                .provider_manager
                .complete_with_tools(
                    provider_type.clone(),
                    &messages,
                    &tools_json,
                    Some(adapter.get_tool_choice(ToolChoiceMode::Auto)),
                    &self.config.llm.model,
                    self.config.llm.temperature,
                    self.config.llm.max_tokens,
                )
                .await
            {
                Ok(r) => {
                    // Track token usage from response using provider-specific adapter
                    // We track both cumulative (for billing) and last-call (for context size)
                    let usage = adapter.extract_usage(&r);
                    let input_tokens = usage.input_tokens;
                    let output_tokens = usage.output_tokens;
                    // Save per-iteration values before accumulation
                    iter_input_tokens = input_tokens;
                    iter_output_tokens = output_tokens;
                    iter_cached_tokens = usage.cached_tokens;
                    iter_cache_write_tokens = usage.cache_write_tokens;

                    total_tokens_input += input_tokens; // Cumulative (total billed across all API calls)
                    context_tokens = input_tokens; // Last call only (context window size)
                    total_tokens_output += output_tokens; // Cumulative (total generated)

                    // Accumulate cached tokens across iterations
                    if let Some(cached) = usage.cached_tokens {
                        total_cached_tokens = Some(total_cached_tokens.unwrap_or(0) + cached);
                    }
                    if let Some(cache_write) = usage.cache_write_tokens {
                        total_cache_write_tokens =
                            Some(total_cache_write_tokens.unwrap_or(0) + cache_write);
                    }

                    debug!(
                        iteration = iteration,
                        input_tokens = input_tokens,
                        output_tokens = output_tokens,
                        cached_tokens = ?usage.cached_tokens,
                        cache_write_tokens = ?usage.cache_write_tokens,
                        total_input = total_tokens_input,
                        total_output = total_tokens_output,
                        "Token usage - input_tokens is this call, total_input is cumulative"
                    );

                    r
                }
                Err(e) => {
                    error!(error = %e, iteration = iteration, "LLM call with tools failed");

                    let error_message = match &e {
                        LLMError::ConnectionError(msg) => {
                            format!(
                                "Connection error: {}\n\nMake sure the LLM service is running and accessible.",
                                msg
                            )
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
                        response: error_message,
                        metrics: ReportMetrics {
                            duration_ms: start.elapsed().as_millis() as u64,
                            tokens_input: total_tokens_input,
                            tokens_output: total_tokens_output,
                            context_tokens,
                            cached_tokens: total_cached_tokens,
                            cache_write_tokens: total_cache_write_tokens,
                            tools_used,
                            mcp_calls: mcp_calls_made,
                            tool_executions: tool_executions_data,
                            reasoning_steps: reasoning_steps_data,
                            iteration_metrics: iteration_metrics_data,
                        },
                        system_prompt: None,
                        tools_json: None,
                    });
                }
            };

            if let Some(thinking) = adapter.extract_thinking(&response) {
                if !thinking.trim().is_empty() {
                    global_sequence += 1;
                    self.emit_progress(StreamChunk::thinking_block(
                        event_workflow_id.clone(),
                        thinking.clone(),
                    ));
                    reasoning_steps_data.push(ReasoningStepData {
                        content: thinking,
                        duration_ms: start.elapsed().as_millis() as u64,
                        sequence: global_sequence,
                        source: ReasoningSource::ModelThinking,
                    });
                }
            }

            // Parse tool calls from response using the adapter (JSON function calling)
            // Use has_tool_calls() as a quick structural check before full parsing
            let function_calls = if adapter.has_tool_calls(&response) {
                adapter.parse_tool_calls(&response)
            } else {
                Vec::new()
            };

            // Record per-iteration metrics (per-call values, not cumulative)
            iteration_metrics_data.push(IterationMetrics {
                iteration: iteration as u32,
                tokens_input: iter_input_tokens,
                tokens_output: iter_output_tokens,
                cached_tokens: iter_cached_tokens,
                cache_write_tokens: iter_cache_write_tokens,
                messages_count: messages.len(),
                tool_calls_count: function_calls.len(),
                duration_ms: iter_start.elapsed().as_millis() as u64,
            });

            // Check if we're finished (no tool calls)
            if function_calls.is_empty() {
                // Extract final content from response
                if let Some(content) = adapter.extract_content(&response) {
                    if !content.trim().is_empty() {
                        final_response_content = content;
                    } else {
                        // Handle empty LLM response gracefully
                        warn!(
                            iteration = iteration,
                            "LLM returned empty content, treating as task completion"
                        );
                        final_response_content = format!(
                            "Task completed after {} iteration(s). Tool executions completed successfully.",
                            iteration
                        );
                    }
                } else {
                    final_response_content = format!(
                        "Task completed after {} iteration(s). Tool executions completed successfully.",
                        iteration
                    );
                }
                debug!(
                    iteration = iteration,
                    provider = adapter.provider_name(),
                    finished = adapter.is_finished(&response),
                    "No tool calls found, finishing"
                );
                break;
            }

            info!(
                iteration = iteration,
                tool_calls_count = function_calls.len(),
                "Found tool calls, executing"
            );

            // Emit progress event about found tool calls
            let tool_names: Vec<String> = function_calls.iter().map(|c| c.name.clone()).collect();
            let reasoning_content = format!(
                "Executing {} tool(s): {}",
                function_calls.len(),
                tool_names.join(", ")
            );
            global_sequence += 1;
            self.emit_progress(StreamChunk::reasoning(
                event_workflow_id.clone(),
                reasoning_content.clone(),
            ));
            reasoning_steps_data.push(ReasoningStepData {
                content: reasoning_content,
                duration_ms: start.elapsed().as_millis() as u64,
                sequence: global_sequence,
                source: ReasoningSource::AgentFlow,
            });

            // Add assistant message with tool calls to messages array
            // This preserves the conversation flow for the next iteration
            let assistant_message = adapter.build_assistant_message(&response);
            messages.push(assistant_message);

            // Execute each function call and collect results
            for call in &function_calls {
                let exec_start = std::time::Instant::now();

                // Emit tool_start event
                self.emit_progress(StreamChunk::tool_start(
                    event_workflow_id.clone(),
                    call.name.clone(),
                ));

                // Execute the function call using our helper
                let result = self
                    .execute_function_call(
                        call,
                        &local_tools,
                        mcp_manager.as_ref(),
                        &mut tools_used,
                        &mut mcp_calls_made,
                        &event_workflow_id,
                        validation_helper.as_ref(),
                    )
                    .await;

                // Capture detailed execution data
                let exec_duration = exec_start.elapsed().as_millis() as u64;
                let tool_type = if call.is_mcp_tool() { "mcp" } else { "local" };
                let (server_name, tool_name_for_data) =
                    if let Some((server, tool)) = call.parse_mcp_name() {
                        (Some(server.to_string()), tool.to_string())
                    } else {
                        (None, call.name.clone())
                    };

                global_sequence += 1;
                tool_executions_data.push(ToolExecutionData {
                    tool_type: tool_type.to_string(),
                    tool_name: tool_name_for_data.clone(),
                    server_name,
                    input_params: call.arguments.clone(),
                    output_result: result.result.clone(),
                    success: result.success,
                    error_message: result.error.clone(),
                    duration_ms: exec_duration,
                    iteration: iteration as u32,
                    sequence: global_sequence,
                });

                let input_json =
                    serde_json::to_string(&call.arguments).unwrap_or_else(|_| "{}".to_string());
                let output_json =
                    serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".to_string());
                self.emit_progress(StreamChunk::tool_call_complete(
                    event_workflow_id.clone(),
                    tool_name_for_data,
                    exec_duration,
                    input_json,
                    output_json,
                    result.success,
                ));

                // Format and add tool result to messages using adapter
                let tool_message = adapter.format_tool_result(&result);
                messages.push(tool_message);
            }
        }

        // Report enforcement: if the LLM finished without providing a meaningful report,
        // make one additional call asking for a proper summary.
        // Guard: iteration > 1 ensures the agent actually executed tools (iteration 1 = first LLM call,
        // iteration 2+ = LLM was called again after tool execution, meaning tools were used).
        if is_generic_completion_message(&final_response_content) && iteration > 1 {
            info!(
                original_response = %final_response_content,
                "Generic completion detected, requesting report from LLM"
            );

            // Check cancellation before the follow-up call
            let cancelled = cancellation_token
                .as_ref()
                .is_some_and(|t| t.is_cancelled());

            if !cancelled {
                // Emit reasoning step about the follow-up
                global_sequence += 1;
                let enforcement_reasoning =
                    "Agent completed tools without a report. Requesting summary...".to_string();
                self.emit_progress(StreamChunk::reasoning(
                    event_workflow_id.clone(),
                    enforcement_reasoning.clone(),
                ));
                reasoning_steps_data.push(ReasoningStepData {
                    content: enforcement_reasoning,
                    duration_ms: start.elapsed().as_millis() as u64,
                    sequence: global_sequence,
                    source: ReasoningSource::AgentFlow,
                });

                // Add the report enforcement prompt as a user message
                messages.push(serde_json::json!({
                    "role": "user",
                    "content": REPORT_ENFORCEMENT_PROMPT
                }));

                // Use an empty tools array to prevent tool calls in the follow-up.
                // ToolChoiceMode::None is ignored by some providers (e.g. Ollama),
                // so sending no tools is the most reliable way to force a text response.
                let empty_tools: Vec<serde_json::Value> = vec![];
                let report_iter_start = std::time::Instant::now();

                match self
                    .provider_manager
                    .complete_with_tools(
                        provider_type.clone(),
                        &messages,
                        &empty_tools,
                        None,
                        &self.config.llm.model,
                        self.config.llm.temperature,
                        self.config.llm.max_tokens,
                    )
                    .await
                {
                    Ok(response) => {
                        let usage = adapter.extract_usage(&response);
                        iter_input_tokens = usage.input_tokens;
                        iter_output_tokens = usage.output_tokens;
                        iter_cached_tokens = usage.cached_tokens;
                        iter_cache_write_tokens = usage.cache_write_tokens;

                        total_tokens_input += iter_input_tokens; // Accumulate
                        context_tokens = iter_input_tokens; // Update context size
                        total_tokens_output += iter_output_tokens;

                        // Accumulate cached tokens from report enforcement call
                        if let Some(cached) = iter_cached_tokens {
                            total_cached_tokens = Some(total_cached_tokens.unwrap_or(0) + cached);
                        }
                        if let Some(cache_write) = iter_cache_write_tokens {
                            total_cache_write_tokens =
                                Some(total_cache_write_tokens.unwrap_or(0) + cache_write);
                        }

                        if let Some(content) = adapter.extract_content(&response) {
                            if !content.trim().is_empty() {
                                info!(
                                    "Report enforcement successful, received meaningful response"
                                );
                                final_response_content = content;
                            } else {
                                warn!("Report enforcement returned empty content, keeping generic message");
                            }
                        }
                    }
                    Err(e) => {
                        warn!(error = %e, "Report enforcement LLM call failed, keeping generic message");
                    }
                }

                // Record report enforcement as an iteration (per-call values)
                iteration_metrics_data.push(IterationMetrics {
                    iteration: (iteration + 1) as u32,
                    tokens_input: iter_input_tokens,
                    tokens_output: iter_output_tokens,
                    cached_tokens: iter_cached_tokens,
                    cache_write_tokens: iter_cache_write_tokens,
                    messages_count: messages.len(),
                    tool_calls_count: 0,
                    duration_ms: report_iter_start.elapsed().as_millis() as u64,
                });
            } else {
                debug!("Skipping report enforcement: workflow cancelled");
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            iterations = iteration,
            provider = adapter.provider_name(),
            tools_used_count = tools_used.len(),
            mcp_calls_count = mcp_calls_made.len(),
            total_tokens_input = total_tokens_input,
            total_tokens_output = total_tokens_output,
            total_cached_tokens = ?total_cached_tokens,
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
            response: final_response_content,
            metrics: ReportMetrics {
                duration_ms,
                tokens_input: total_tokens_input,
                tokens_output: total_tokens_output,
                context_tokens,
                cached_tokens: total_cached_tokens,
                cache_write_tokens: total_cache_write_tokens,
                tools_used,
                mcp_calls: mcp_calls_made,
                tool_executions: tool_executions_data,
                reasoning_steps: reasoning_steps_data,
                iteration_metrics: iteration_metrics_data,
            },
            // Return system_prompt and tools_json only on first message for persistence
            system_prompt: system_prompt_for_report,
            tools_json: if is_first_message {
                Some(serde_json::Value::Array(tools_json))
            } else {
                None
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
                is_reasoning: false,
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "You are a helpful assistant.".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        }
    }

    #[test]
    fn test_llm_agent_new() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config.clone(), manager);

        assert_eq!(agent.config().id, "test_llm_agent");
        assert_eq!(agent.config().llm.provider, "Ollama");
    }

    #[test]
    fn test_llm_agent_capabilities() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config, manager);

        let capabilities = agent.capabilities();
        assert!(capabilities.contains(&"llm_completion".to_string()));
        assert!(capabilities.contains(&"provider:Ollama".to_string()));
        assert!(capabilities.contains(&"model:llama3.2".to_string()));
    }

    #[test]
    fn test_llm_agent_lifecycle() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config, manager);

        assert!(matches!(agent.lifecycle(), Lifecycle::Permanent));
    }

    #[test]
    fn test_llm_agent_get_provider_type() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config, manager);

        let provider = agent.get_provider_type().unwrap();
        assert_eq!(provider, ProviderType::Ollama);
    }

    #[test]
    fn test_llm_agent_get_provider_type_mistral() {
        let mut config = create_test_config();
        config.llm.provider = "Mistral".to_string();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config, manager);

        let provider = agent.get_provider_type().unwrap();
        assert_eq!(provider, ProviderType::Mistral);
    }

    #[test]
    fn test_llm_agent_build_prompt() {
        let config = create_test_config();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
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
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
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
        // Empty provider is the only invalid case now (others become Custom)
        let mut config = create_test_config();
        config.llm.provider = String::new();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config, manager);

        let result = agent.get_provider_type();
        assert!(result.is_err());
    }

    #[test]
    fn test_llm_agent_custom_provider() {
        let mut config = create_test_config();
        config.llm.provider = "routerlab".to_string();
        let manager = Arc::new(ProviderManager::new().expect("test provider manager"));
        let agent = LLMAgent::new(config, manager);

        let result = agent.get_provider_type();
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            ProviderType::Custom("routerlab".to_string())
        );
    }

    // Note: XML-based tool calling tests have been removed.
    // JSON function calling tests are in:
    // - src/llm/adapters/tests.rs (adapter parsing)
    // - src/models/function_calling.rs (type tests)
    // - Integration tests in tests/ directory

    // --- Report Enforcement Tests ---

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
        // Real markdown reports should NOT be detected as generic
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
        // Trimming should work
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
