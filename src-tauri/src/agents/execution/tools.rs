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
use crate::models::agent::AgentKind;
use crate::models::function_calling::{FunctionCall, FunctionCallResult};
use crate::models::mcp::MCPTool;
use crate::models::AgentConfig;
use crate::tools::{
    context::AgentToolContext,
    validation_helper::{is_destructive_file_op, ValidationHelper},
    Tool, ToolDefinition, ToolError, ToolFactory, ToolResult,
};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Tool decorator that rejects a fixed set of (write) operations while
/// delegating everything else to the wrapped tool. Used to make the Prompt and
/// Skill managers read-only in DETACHED runs (K6.1): the Kanban analyze/compose
/// runs carry no agent context and inject an UNTRUSTED worker report, so a
/// prompt-injection payload must not be able to rewrite shared prompts/skills.
/// `list`/`read` operations stay available so the analyzer can still consult
/// them to inform its verdict.
pub(crate) struct ReadOnlyToolGuard {
    inner: Arc<dyn Tool>,
    forbidden_ops: &'static [&'static str],
}

impl ReadOnlyToolGuard {
    pub(crate) fn new(inner: Arc<dyn Tool>, forbidden_ops: &'static [&'static str]) -> Self {
        Self {
            inner,
            forbidden_ops,
        }
    }
}

#[async_trait]
impl Tool for ReadOnlyToolGuard {
    fn id(&self) -> &str {
        self.inner.id()
    }

    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        let op = input
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if self.forbidden_ops.contains(&op) {
            return Err(ToolError::PermissionDenied(format!(
                "Operation '{}' is disabled in a detached run (read-only). Prompt/skill \
                 edits happen via the card review chat with the user.",
                op
            )));
        }
        self.inner.execute(input).await
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        self.inner.validate_input(input)
    }
}

/// Write operations stripped from the Prompt/Skill managers in detached runs.
const PROMPT_MANAGER_WRITE_OPS: &[&str] = &["create_prompt", "update_prompt"];
const SKILL_MANAGER_WRITE_OPS: &[&str] = &[
    "create_skill",
    "update_skill",
    "restore_skill_version",
    "grant_skill_to_agent",
    "revoke_skill_from_agent",
];

/// Wraps the Prompt/Skill manager tools of a DETACHED run so their write
/// operations are rejected (K6.1). Other tools pass through untouched.
fn harden_detached_writes(tools: Vec<Arc<dyn Tool>>) -> Vec<Arc<dyn Tool>> {
    tools
        .into_iter()
        .map(|t| match t.id() {
            "PromptManagerTool" => {
                Arc::new(ReadOnlyToolGuard::new(t, PROMPT_MANAGER_WRITE_OPS)) as Arc<dyn Tool>
            }
            "SkillManagerTool" => {
                Arc::new(ReadOnlyToolGuard::new(t, SKILL_MANAGER_WRITE_OPS)) as Arc<dyn Tool>
            }
            _ => t,
        })
        .collect()
}

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

    // Kanban agents are confined: they orchestrate cards but must NEVER act as
    // a delegation caller (PAT_KANBAN_STRICT_SEPARATION — they are already
    // excluded as a callee in delegate_task_execution). Strip the three
    // sub-agent tools defensively in case one was persisted on the config, and
    // force the basic-tools branch below so `create_tools_with_context` (which
    // auto-injects Spawn/Delegate/Parallel for a primary agent) is never taken.
    // Streaming/attribution stay intact — only the sub-agent tools are omitted.
    //
    // UserQuestionTool is stripped too: the confined card review chat only runs
    // on `/kanban`, which does not mount the UserQuestionModal, so a question
    // would hang in the void until its 5-minute timeout. The Kanban supervisor
    // must phrase everything as a direct turn, never an interactive prompt.
    let is_kanban = config.kind == Some(AgentKind::Kanban);
    if is_kanban {
        tool_names.retain(|t| {
            t != "SpawnAgentTool"
                && t != "DelegateTaskTool"
                && t != "ParallelTasksTool"
                && t != "UserQuestionTool"
        });
        // Auto-inject the per-card chat tools only when a context is present,
        // i.e. the streaming card review chat (the only Kanban streaming path).
        // The detached analyze/compose runs pass `agent_context: None`, so they
        // never receive these tools. The tools self-gate anyway (they resolve
        // the card via `review_chat_workflow_id`), so this is scoping for
        // prompt clarity, not security.
        if effective_context.is_some() {
            for card_tool in ["RerunWorkerTool", "MoveCardTool", "ScheduleCardTool"] {
                if !tool_names.iter().any(|t| t == card_tool) {
                    tool_names.push(card_tool.to_string());
                }
            }
        }
    }

    // If this is the primary agent and we have context, use create_tools_with_context
    // (skipped for Kanban agents so the delegation tools are never auto-added).
    let tools = match (is_primary_agent && !is_kanban, effective_context) {
        (true, Some(context)) => {
            debug!(
                agent_id = %config.id,
                "Creating tools with context for primary agent (sub-agent tools available)"
            );
            factory
                .create_tools_with_context(
                    &tool_names,
                    workflow_id,
                    config.id.clone(),
                    Some(context.clone()),
                    true,
                )
                .await
        }
        _ => {
            // Sub-agents or agents without context use basic tool creation.
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
    };

    // K6.1: a DETACHED run carries no agent context (the Kanban analyze/compose
    // runs pass `agent_context: None`). With no human in the loop and an
    // UNTRUSTED worker report injected into analyze, the Prompt/Skill managers
    // must not be able to WRITE shared prompts/skills — wrap them read-only.
    // Runs WITH a context (the /agent page, the card review chat) keep full
    // write access.
    if effective_context.is_none() {
        harden_detached_writes(tools)
    } else {
        tools
    }
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
        let matching_tool = ctx.local_tools.iter().find(|t| t.id() == call.name);

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
            let available_tools: Vec<String> =
                ctx.local_tools.iter().map(|t| t.id().to_string()).collect();

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;
    use crate::tools::context::AgentToolContext;

    fn agent_config_from(value: serde_json::Value) -> AgentConfig {
        serde_json::from_value(value).expect("valid AgentConfig fixture")
    }

    fn tool_ids(tools: &[Arc<dyn Tool>]) -> Vec<String> {
        tools.iter().map(|t| t.id().to_string()).collect()
    }

    /// A standard (kind = None) primary agent with a context gets the three
    /// sub-agent tools auto-injected — this is the baseline the Kanban gating
    /// must NOT reproduce.
    #[tokio::test]
    async fn standard_primary_agent_gets_sub_agent_tools() {
        let (state, _g) = setup_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Std",
            "tools": ["MemoryTool"],
        }));

        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            Some(&context),
            Some("wf-1".to_string()),
            true,
            None,
        )
        .await;
        let ids = tool_ids(&tools);
        assert!(
            ids.contains(&"SpawnAgentTool".to_string()),
            "standard primary agent must receive SpawnAgentTool, got {ids:?}"
        );
    }

    /// A Kanban-kind primary agent must NEVER receive Spawn/Delegate/Parallel,
    /// even with a full context present (PAT_KANBAN_STRICT_SEPARATION). The
    /// explicitly-configured SpawnAgentTool is stripped defensively.
    #[tokio::test]
    async fn kanban_primary_agent_never_gets_sub_agent_tools() {
        let (state, _g) = setup_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Kanban",
            "kind": "kanban",
            // SpawnAgentTool persisted on the config must be stripped.
            "tools": ["MemoryTool", "SpawnAgentTool"],
        }));

        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            Some(&context),
            Some("wf-1".to_string()),
            true,
            None,
        )
        .await;
        let ids = tool_ids(&tools);
        for forbidden in ["SpawnAgentTool", "DelegateTaskTool", "ParallelTasksTool"] {
            assert!(
                !ids.contains(&forbidden.to_string()),
                "Kanban agent must NOT receive {forbidden}, got {ids:?}"
            );
        }
        assert!(
            ids.contains(&"MemoryTool".to_string()),
            "Kanban agent must still receive its non-delegation tools, got {ids:?}"
        );
    }

    fn find_tool<'a>(tools: &'a [Arc<dyn Tool>], id: &str) -> &'a Arc<dyn Tool> {
        tools
            .iter()
            .find(|t| t.id() == id)
            .unwrap_or_else(|| panic!("{id} not found in {:?}", tool_ids(tools)))
    }

    /// K6.1: a DETACHED run (no agent context — the Kanban analyze/compose path)
    /// must have PromptManager write operations blocked, while read operations
    /// stay available.
    #[tokio::test]
    async fn detached_run_blocks_prompt_manager_writes() {
        let (state, _g) = setup_test_state().await;
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Detached",
            "tools": ["PromptManagerTool"],
        }));

        // agent_context None + context_override None => detached.
        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            None,
            Some("wf-1".to_string()),
            false,
            None,
        )
        .await;

        let pm = find_tool(&tools, "PromptManagerTool");
        let write = pm
            .execute(serde_json::json!({
                "operation": "create_prompt",
                "name": "x",
                "content": "y"
            }))
            .await;
        assert!(
            matches!(write, Err(crate::tools::ToolError::PermissionDenied(_))),
            "detached create_prompt must be denied, got {write:?}"
        );

        // Read stays available (not denied).
        let read = pm
            .execute(serde_json::json!({"operation": "list_prompts"}))
            .await;
        assert!(
            !matches!(read, Err(crate::tools::ToolError::PermissionDenied(_))),
            "detached list_prompts must NOT be denied, got {read:?}"
        );
    }

    /// K6.1: a run WITH an agent context (the /agent page, the card review chat)
    /// keeps full PromptManager write access — the guard is detached-only.
    #[tokio::test]
    async fn contextful_run_allows_prompt_manager_writes() {
        let (state, _g) = setup_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Contextful",
            "tools": ["PromptManagerTool"],
        }));

        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            Some(&context),
            Some("wf-1".to_string()),
            true,
            None,
        )
        .await;

        let pm = find_tool(&tools, "PromptManagerTool");
        let write = pm
            .execute(serde_json::json!({
                "operation": "create_prompt",
                "name": "x",
                "content": "y"
            }))
            .await;
        assert!(
            !matches!(write, Err(crate::tools::ToolError::PermissionDenied(_))),
            "contextful create_prompt must NOT be denied by the read-only guard, got {write:?}"
        );
    }

    /// K6.1: a DETACHED run must have SkillManager write operations blocked by
    /// the read-only guard. We assert the GUARD denied it (its distinctive
    /// message) — not the unrelated `ensure_kanban` gate — to confirm the
    /// wrapping is what intercepts the write before it reaches the inner tool.
    #[tokio::test]
    async fn detached_run_wraps_skill_manager_writes() {
        let (state, _g) = setup_test_state().await;
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "KanbanDetached",
            "kind": "kanban",
            "tools": ["SkillManagerTool"],
        }));

        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            None,
            Some("wf-1".to_string()),
            false,
            None,
        )
        .await;

        let sm = find_tool(&tools, "SkillManagerTool");
        let write = sm
            .execute(serde_json::json!({
                "operation": "create_skill",
                "name": "x",
                "content": "y",
                "description": "d",
                "target_agent_id": uuid::Uuid::new_v4().to_string()
            }))
            .await;
        match write {
            Err(crate::tools::ToolError::PermissionDenied(msg)) => assert!(
                msg.contains("read-only"),
                "create_skill must be denied by the read-only GUARD, got: {msg}"
            ),
            other => panic!("expected PermissionDenied from the guard, got {other:?}"),
        }
    }

    /// K6.1: the `ReadOnlyToolGuard` itself — forbidden ops are denied with the
    /// read-only message, every other op delegates to the wrapped tool. Covers
    /// the SkillManager forbidden list (read ops pass through) without the
    /// `ensure_kanban` confound of a non-persisted agent.
    #[tokio::test]
    async fn read_only_guard_blocks_forbidden_and_passes_through() {
        // Minimal fake tool that echoes the input back on execute.
        struct EchoTool;
        #[async_trait]
        impl Tool for EchoTool {
            fn id(&self) -> &str {
                "SkillManagerTool"
            }
            fn definition(&self) -> ToolDefinition {
                ToolDefinition {
                    id: "SkillManagerTool".to_string(),
                    name: "SkillManager".to_string(),
                    summary: String::new(),
                    description: String::new(),
                    input_schema: serde_json::json!({}),
                    output_schema: serde_json::json!({}),
                    requires_confirmation: false,
                }
            }
            async fn execute(&self, input: Value) -> ToolResult<Value> {
                Ok(input)
            }
            fn validate_input(&self, _input: &Value) -> ToolResult<()> {
                Ok(())
            }
        }

        let guard = ReadOnlyToolGuard::new(Arc::new(EchoTool), SKILL_MANAGER_WRITE_OPS);
        assert_eq!(guard.id(), "SkillManagerTool", "id delegates to inner");

        for write_op in SKILL_MANAGER_WRITE_OPS {
            let r = guard
                .execute(serde_json::json!({"operation": write_op}))
                .await;
            assert!(
                matches!(r, Err(crate::tools::ToolError::PermissionDenied(_))),
                "write op {write_op} must be denied, got {r:?}"
            );
        }

        // A read op delegates to the inner tool (echoes the input back).
        let read = guard
            .execute(serde_json::json!({"operation": "list_skills"}))
            .await
            .expect("read op must delegate");
        assert_eq!(read["operation"], "list_skills");
    }

    /// A standard (kind = None) agent that lists UserQuestionTool keeps it:
    /// the `/agent` page mounts the UserQuestionModal that answers the prompt.
    /// This is the baseline the Kanban gating must NOT reproduce.
    #[tokio::test]
    async fn standard_agent_keeps_user_question_tool() {
        let (state, _g) = setup_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Std",
            "tools": ["UserQuestionTool"],
        }));

        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            Some(&context),
            Some("wf-1".to_string()),
            true,
            None,
        )
        .await;
        let ids = tool_ids(&tools);
        assert!(
            ids.contains(&"UserQuestionTool".to_string()),
            "standard agent must receive UserQuestionTool, got {ids:?}"
        );
    }

    /// A Kanban-kind agent must NEVER receive UserQuestionTool, even when it is
    /// explicitly persisted on the config: the confined card review chat only
    /// runs on `/kanban`, which does not mount the UserQuestionModal, so a
    /// question would hang for the full 5-minute timeout in the void. The tool
    /// is stripped just like the sub-agent delegation tools.
    #[tokio::test]
    async fn kanban_agent_never_gets_user_question_tool() {
        let (state, _g) = setup_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Kanban",
            "kind": "kanban",
            "tools": ["MemoryTool", "UserQuestionTool"],
        }));

        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            Some(&context),
            Some("wf-1".to_string()),
            true,
            None,
        )
        .await;
        let ids = tool_ids(&tools);
        assert!(
            !ids.contains(&"UserQuestionTool".to_string()),
            "Kanban agent must NOT receive UserQuestionTool, got {ids:?}"
        );
        assert!(
            ids.contains(&"MemoryTool".to_string()),
            "Kanban agent must still receive its non-delegation tools, got {ids:?}"
        );
    }
}
