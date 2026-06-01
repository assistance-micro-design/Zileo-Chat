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
use crate::constants::mcp::{
    MCP_MAX_CALLS_PER_RUN, MCP_MAX_RESULT_BYTES_PER_RUN, MCP_MAX_SINGLE_RESULT_BYTES,
};
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
    /// True for an unattended (detached) run — enables the R-SEC-4 MCP gate.
    pub is_detached: bool,
    /// True when this detached run is a DELEGATED sub-agent (DelegateTask /
    /// ParallelTasks), as opposed to a direct detached run or a spawned
    /// sub-agent. In a delegated run the allowlist gate additionally requires
    /// the matching entry's `allow_in_delegated_runs` flag (R1).
    pub is_delegated: bool,
    /// Per-(server_id, tool) allowlist consulted by the detached MCP gate.
    pub mcp_tool_allowlist: &'a [crate::models::agent::McpToolAllowlistEntry],
}

/// Pure R-SEC-4 decision: is `(server_id, tool)` armed in the agent's detached
/// allowlist? Keyed by the immutable `server_id` (never the display name).
/// `server_id == None` (the server name could not be resolved to an id) is
/// fail-closed → `false`. An empty allowlist arms nothing.
///
/// R1 — `is_delegated`: when this detached run is a DELEGATED sub-agent
/// (DelegateTask / ParallelTasks), the matching entry must ALSO set
/// `allow_in_delegated_runs`. A DIRECT detached run (rerun-primary / analyze /
/// compose) or a SPAWNED sub-agent (`is_delegated == false`) ignores the flag,
/// preserving the pre-R1 behavior. This closes the UNION confused-deputy where
/// a detached worker delegates to a standard agent whose own allowlist arms
/// tools the worker should not be able to trigger.
fn is_mcp_tool_armed(
    allowlist: &[crate::models::agent::McpToolAllowlistEntry],
    server_id: Option<&str>,
    tool: &str,
    is_delegated: bool,
) -> bool {
    server_id.is_some_and(|sid| {
        allowlist.iter().any(|e| {
            e.server_id == sid
                && e.tools.iter().any(|t| t == tool)
                && (!is_delegated || e.allow_in_delegated_runs)
        })
    })
}

/// R-SEC-10 pure decision: may another MCP call proceed in this run?
///
/// `calls_so_far` is the number of MCP calls already attempted in the run
/// (EXCLUDING the one about to be made), and `bytes_so_far` is the cumulative
/// sink byte size (serialized result + error) of all prior MCP results — success
/// AND error. Returns `Err` with a human-readable refusal once either the call cap
/// ([`MCP_MAX_CALLS_PER_RUN`]) or the cumulative byte budget
/// ([`MCP_MAX_RESULT_BYTES_PER_RUN`]) is reached.
///
/// CHECK-BEFORE-REFUSE: consulted *before* a call is dispatched, so exactly
/// `MCP_MAX_CALLS_PER_RUN` calls are allowed and an in-flight result is never
/// truncated (truncating would corrupt the JSON and hide information).
///
/// # Errors
/// Returns a refusal string when the per-run call cap or byte budget is reached.
fn mcp_run_budget_check(calls_so_far: usize, bytes_so_far: usize) -> Result<(), String> {
    if calls_so_far >= MCP_MAX_CALLS_PER_RUN {
        return Err(format!(
            "Per-run MCP call limit reached ({MCP_MAX_CALLS_PER_RUN} calls): this agent run has \
             made too many MCP tool calls. No further MCP calls are permitted in this run."
        ));
    }
    if bytes_so_far >= MCP_MAX_RESULT_BYTES_PER_RUN {
        return Err(format!(
            "Per-run MCP result budget reached ({MCP_MAX_RESULT_BYTES_PER_RUN} bytes): this agent \
             run has accumulated too much MCP output. No further MCP calls are permitted in this run."
        ));
    }
    Ok(())
}

/// R-SEC-10.1: total bytes a tool result actually pushes to the run's sinks —
/// the serialized `result.result` PLUS the `result.error` string.
///
/// BOTH fields reach the LLM tool message (`result_to_string` emits
/// `{"error": ...}` for a failure), the persisted row (`output_result` +
/// `error_message`), and the live stream chunk. Measuring only the serialized
/// result misses a giant ERROR payload: `FunctionCallResult::failure` sets
/// `result` to `Null` (serializes to `"null"`, ~4 bytes) and routes the
/// server's (possibly giant) message into `error`. Summing both is the size the
/// per-result cap and the cumulative budget must use. `serialized_result` is the
/// caller's already-computed `to_string(result.result)` (no re-serialization).
pub(crate) fn result_sink_byte_len(serialized_result: &str, error: Option<&str>) -> usize {
    serialized_result.len() + error.map_or(0, str::len)
}

/// R-SEC-10.1 pure decision: must a just-returned MCP result be replaced for
/// exceeding the per-result size cap?
///
/// Returns `Some(refusal_message)` when an MCP result's sink byte size
/// (`result_byte_len` — serialized result + error, via [`result_sink_byte_len`],
/// measured post image-strip) exceeds [`MCP_MAX_SINGLE_RESULT_BYTES`] — the
/// caller then swaps the giant payload for that error before it reaches the LLM
/// tool message, the persisted `tool_execution` row, or the live stream chunk.
/// Returns `None` (pass through unchanged) for a local-tool result (user-trusted,
/// out of scope) or a result within the cap.
///
/// SUCCESS-AGNOSTIC by design: under the R-SEC-10 threat model a compromised MCP
/// server controls its JSON-RPC response size on BOTH paths, so a giant ERROR
/// payload (`success == false`, carried in `result.error`) is just as dangerous
/// as a giant success payload and is capped identically. Replacing a giant error
/// with a small capped error is acceptable (the call already failed).
///
/// Complements the cumulative [`mcp_run_budget_check`]: this is a POST-call cap
/// on the CURRENT result; the cumulative budget is the PRE-call gate for the
/// NEXT call. The two coexist — neither truncates a payload.
pub(crate) fn mcp_oversized_result_refusal(
    is_mcp_tool: bool,
    result_byte_len: usize,
) -> Option<String> {
    if is_mcp_tool && result_byte_len > MCP_MAX_SINGLE_RESULT_BYTES {
        Some(format!(
            "MCP result exceeded the per-result size limit of {MCP_MAX_SINGLE_RESULT_BYTES} bytes; \
             refused to protect the run context."
        ))
    } else {
        None
    }
}

/// R-SEC-10 pure accounting: bytes a just-returned tool result contributes to the
/// cumulative per-run MCP byte budget.
///
/// Counts EVERY MCP result — success AND error — because a compromised server
/// controls error size too; gating on success would let a flood of medium-sized
/// error payloads (each under the per-result cap) bypass the cumulative budget.
/// Local-tool results contribute nothing (out of scope). Call with the POST
/// per-result-cap sink byte length (`result_byte_len` from [`result_sink_byte_len`])
/// so an oversized result charges only its (small) replacement, never the giant
/// original — and so a giant error (carried in `result.error`) is charged in full.
pub(crate) fn mcp_result_budget_charge(is_mcp_tool: bool, result_byte_len: usize) -> usize {
    if is_mcp_tool {
        result_byte_len
    } else {
        0
    }
}

/// Executes a single function call (local or MCP tool).
///
/// `mcp_result_bytes` is the cumulative sink byte size (serialized result +
/// error) of all prior MCP results in this run — success AND error — consulted by
/// the R-SEC-10 per-run budget gate (the caller accumulates it in `iteration.rs`
/// after each MCP call via [`result_sink_byte_len`] + [`mcp_result_budget_charge`]).
pub(crate) async fn execute_function_call(
    call: &FunctionCall,
    ctx: &FunctionCallContext<'_>,
    tools_used: &mut Vec<String>,
    mcp_calls_made: &mut Vec<String>,
    mcp_result_bytes: usize,
) -> FunctionCallResult {
    let start = std::time::Instant::now();

    // Check if MCP tool
    if let Some((server, tool)) = call.parse_mcp_name() {
        // Execute via MCP
        if let Some(mcp) = ctx.mcp_manager {
            // R-SEC-10: snapshot the pre-call count BEFORE recording this
            // attempt, so the per-run cap (CHECK-BEFORE-REFUSE) counts only
            // prior calls and allows EXACTLY MCP_MAX_CALLS_PER_RUN.
            let calls_before = mcp_calls_made.len();
            mcp_calls_made.push(call.name.clone());

            // R-SEC-4: in a DETACHED run there is no human to answer the
            // validation modal, so the gate is the per-agent allowlist —
            // evaluated UNCONDITIONALLY (independent of ValidationMode, which
            // would otherwise let `Auto` short-circuit to fail-open). Keyed by
            // the immutable `server_id` (resolved from the tool's server name),
            // never the display name. Empty allowlist = nothing armed = refuse.
            // Evaluated FIRST so an unarmed detached call is refused and audited
            // as a security refusal even when the run is also over budget.
            if ctx.is_detached {
                let server_id = mcp.get_server_id_by_name(server).await;
                let armed = is_mcp_tool_armed(
                    ctx.mcp_tool_allowlist,
                    server_id.as_deref(),
                    tool,
                    ctx.is_delegated,
                );
                if !armed {
                    // R-SEC-11: structured refusal audit (no secret). `delegated`
                    // distinguishes the R1 confused-deputy refusal (armed for the
                    // agent but not flagged `allow_in_delegated_runs`) from a
                    // plain unarmed refusal.
                    warn!(
                        server = %server,
                        server_id = server_id.as_deref().unwrap_or("<unknown>"),
                        tool = %tool,
                        delegated = ctx.is_delegated,
                        "MCP tool refused: not armed for this agent in a detached run"
                    );
                    // R2: persist the refusal into the EXISTING audit log
                    // (Settings > Audit Log) so a no-human-in-the-loop block is
                    // visible, not only traced. Unconditional + best-effort
                    // (never blocks or fails the refusal itself).
                    if let Some(helper) = ctx.validation_helper {
                        helper
                            .record_security_refusal(
                                &call.name,
                                server_id.as_deref(),
                                "not armed for this agent in a detached run",
                                ctx.is_delegated,
                                ctx.workflow_id,
                            )
                            .await;
                    }
                    let scope = if ctx.is_delegated {
                        "unattended (detached) delegated runs"
                    } else {
                        "unattended (detached) runs"
                    };
                    return FunctionCallResult::failure(
                        &call.id,
                        &call.name,
                        format!(
                            "MCP tool '{}' is not enabled for this agent in {}",
                            call.name, scope
                        ),
                    );
                }
                // Armed: proceed without the interactive modal (no human present).
            }

            // R-SEC-10: per-run cap + cumulative byte budget. Checked AFTER the
            // detached gate (a budget refusal can never short-circuit / fail-open
            // that gate) and BEFORE the attended modal (don't prompt the human for
            // a call we will refuse anyway). Applies to BOTH run types. An
            // in-flight result is never truncated — the NEXT call is refused.
            if let Err(reason) = mcp_run_budget_check(calls_before, mcp_result_bytes) {
                warn!(tool = %call.name, reason = %reason, "MCP call refused: per-run budget/cap reached");
                return FunctionCallResult::failure(&call.id, &call.name, reason);
            }

            if !ctx.is_detached {
                if let Some(helper) = ctx.validation_helper {
                    // Attended run: existing interactive validation modal.
                    if let Err(e) = helper
                        .request_mcp_validation(
                            ctx.workflow_id,
                            server,
                            tool,
                            call.arguments.clone(),
                        )
                        .await
                    {
                        warn!(tool = %call.name, error = %e, "MCP validation rejected");
                        return FunctionCallResult::failure(&call.id, &call.name, e.to_string());
                    }
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

    // ----------------------------------------------------------------------
    // R-SEC-4: detached MCP tool allowlist gate
    // ----------------------------------------------------------------------

    use crate::models::agent::McpToolAllowlistEntry;
    use crate::models::function_calling::FunctionCall;

    fn allow(server_id: &str, tools: &[&str]) -> McpToolAllowlistEntry {
        McpToolAllowlistEntry {
            server_id: server_id.to_string(),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            allow_in_delegated_runs: false,
        }
    }

    /// Like [`allow`] but the entry is also armed for DELEGATED detached runs
    /// (`allow_in_delegated_runs = true`, R1).
    fn allow_delegated(server_id: &str, tools: &[&str]) -> McpToolAllowlistEntry {
        McpToolAllowlistEntry {
            server_id: server_id.to_string(),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            allow_in_delegated_runs: true,
        }
    }

    /// R1 matrix on the pure decision: the per-entry `allow_in_delegated_runs`
    /// flag ONLY gates DELEGATED runs (Delegate/Parallel). A DIRECT detached run
    /// (rerun-primary / analyze / compose / Spawn-clone, `is_delegated = false`)
    /// ignores the flag — its behavior is exactly the pre-R1 armed check.
    #[test]
    fn armed_decision_respects_delegated_flag() {
        let strict = vec![allow("srv-id-1", &["read"])]; // flag = false (default/strict)
        let delegable = vec![allow_delegated("srv-id-1", &["read"])]; // flag = true

        // DIRECT detached (is_delegated = false): flag irrelevant, armed by match.
        assert!(is_mcp_tool_armed(&strict, Some("srv-id-1"), "read", false));
        assert!(is_mcp_tool_armed(
            &delegable,
            Some("srv-id-1"),
            "read",
            false
        ));

        // DELEGATED detached (is_delegated = true): armed ONLY if flagged.
        assert!(
            !is_mcp_tool_armed(&strict, Some("srv-id-1"), "read", true),
            "a strict entry must be refused in a delegated run (confused-deputy)"
        );
        assert!(
            is_mcp_tool_armed(&delegable, Some("srv-id-1"), "read", true),
            "an explicitly delegation-armed entry must pass in a delegated run"
        );

        // The flag never resurrects an unarmed tool nor a wrong server.
        assert!(!is_mcp_tool_armed(
            &delegable,
            Some("srv-id-1"),
            "write",
            true
        ));
        assert!(!is_mcp_tool_armed(&delegable, Some("other"), "read", true));
        assert!(!is_mcp_tool_armed(&delegable, None, "read", true));
    }

    #[test]
    fn armed_decision_matches_server_id_and_tool() {
        let allowlist = vec![allow("srv-id-1", &["read", "list"])];
        // Armed pair (direct detached run: is_delegated = false).
        assert!(is_mcp_tool_armed(
            &allowlist,
            Some("srv-id-1"),
            "read",
            false
        ));
        // Tool not in the armed set.
        assert!(!is_mcp_tool_armed(
            &allowlist,
            Some("srv-id-1"),
            "write",
            false
        ));
        // Right tool, wrong server id.
        assert!(!is_mcp_tool_armed(
            &allowlist,
            Some("other-id"),
            "read",
            false
        ));
    }

    #[test]
    fn budget_check_allows_under_both_limits() {
        // R-SEC-10: a run well under both ceilings proceeds.
        assert!(mcp_run_budget_check(0, 0).is_ok());
        // One below each limit is still allowed (the cap allows EXACTLY
        // MCP_MAX_CALLS_PER_RUN calls, so calls_so_far == cap-1 passes).
        assert!(
            mcp_run_budget_check(MCP_MAX_CALLS_PER_RUN - 1, MCP_MAX_RESULT_BYTES_PER_RUN - 1)
                .is_ok()
        );
    }

    #[test]
    fn budget_check_refuses_at_call_cap() {
        // At the cap, the NEXT call is refused (check-before-refuse).
        let err = mcp_run_budget_check(MCP_MAX_CALLS_PER_RUN, 0)
            .expect_err("reaching the per-run call cap must refuse");
        assert!(
            err.contains("call limit reached") && err.contains(&MCP_MAX_CALLS_PER_RUN.to_string()),
            "the refusal must name the per-run MCP call cap, got: {err:?}"
        );
    }

    #[test]
    fn budget_check_refuses_at_byte_budget() {
        // At the byte budget (with calls under the cap), the next call is refused.
        let err = mcp_run_budget_check(0, MCP_MAX_RESULT_BYTES_PER_RUN)
            .expect_err("reaching the per-run byte budget must refuse");
        assert!(
            err.contains("result budget reached")
                && err.contains(&MCP_MAX_RESULT_BYTES_PER_RUN.to_string()),
            "the refusal must name the per-run MCP byte budget, got: {err:?}"
        );
    }

    #[test]
    fn budget_check_cap_takes_precedence_when_both_exceeded() {
        // When both limits are blown, the message is deterministic (cap first)
        // so the refusal is stable and testable.
        let err = mcp_run_budget_check(MCP_MAX_CALLS_PER_RUN, MCP_MAX_RESULT_BYTES_PER_RUN)
            .expect_err("both limits exceeded must refuse");
        assert!(
            err.contains("call limit reached"),
            "the call cap must take precedence in the refusal message, got: {err:?}"
        );
    }

    #[test]
    fn oversized_mcp_result_is_refused_success_or_error() {
        // R-SEC-10.1: an MCP result past the per-result cap is replaced by an
        // actionable error (closes the cumulative soft-ceiling). SUCCESS-AGNOSTIC:
        // the wiring serializes the result the same way whether the call
        // succeeded or errored, so a compromised server's giant ERROR payload is
        // capped identically to a giant success payload.
        let refusal = mcp_oversized_result_refusal(true, MCP_MAX_SINGLE_RESULT_BYTES + 1)
            .expect("an oversized MCP result must be refused");
        assert!(
            refusal.contains("per-result size limit")
                && refusal.contains(&MCP_MAX_SINGLE_RESULT_BYTES.to_string()),
            "the refusal must name the per-result cap, got: {refusal:?}"
        );
    }

    #[test]
    fn mcp_result_at_or_under_cap_passes_whole() {
        // Boundary: exactly the cap passes (strict `>`), as does anything smaller.
        assert!(mcp_oversized_result_refusal(true, MCP_MAX_SINGLE_RESULT_BYTES).is_none());
        assert!(mcp_oversized_result_refusal(true, 0).is_none());
    }

    #[test]
    fn oversized_local_tool_result_is_out_of_scope() {
        // Local tools are user-trusted; the per-result MCP cap must not touch them
        // (the R-SEC-10 threat model is a compromised MCP server, not local tools).
        assert!(mcp_oversized_result_refusal(false, MCP_MAX_SINGLE_RESULT_BYTES * 4).is_none());
    }

    #[test]
    fn mcp_result_budget_charge_counts_every_mcp_result() {
        // R-SEC-10 twin fix: the cumulative budget charges EVERY MCP result, not
        // just successful ones — a compromised server can flood medium-sized ERROR
        // payloads (each under the per-result cap) that would otherwise bypass the
        // cumulative gate. Success-agnostic (no success parameter).
        assert_eq!(mcp_result_budget_charge(true, 1234), 1234);
        assert_eq!(mcp_result_budget_charge(true, 0), 0);
    }

    #[test]
    fn local_tool_result_charges_nothing_to_mcp_budget() {
        // A giant LOCAL result is out of scope: it charges nothing to the MCP
        // cumulative budget (and is not per-result capped either).
        assert_eq!(
            mcp_result_budget_charge(false, MCP_MAX_SINGLE_RESULT_BYTES * 4),
            0
        );
    }

    #[test]
    fn result_sink_byte_len_includes_the_error_payload_not_just_the_result() {
        // The inert-twin bug: a giant MCP error routes its payload through
        // `.error` while `.result` is Null, so measuring only the serialized
        // result ("null", ~4 bytes) misses it. result_sink_byte_len must sum BOTH
        // fields — this is the size that actually reaches all three sinks.
        let giant = "x".repeat(MCP_MAX_SINGLE_RESULT_BYTES + 1);
        let failure = FunctionCallResult::failure("id", "tool", giant);
        let serialized_result =
            serde_json::to_string(&failure.result).unwrap_or_else(|_| "{}".to_string());
        // What the OLD (inert) measure saw — the serialized Null result is tiny.
        assert!(
            serialized_result.len() < 16,
            "a failure's result serializes to ~`null`, got {}",
            serialized_result.len()
        );
        // The real sink size includes the giant `.error` → over the cap, so the
        // per-result refusal now fires for a giant error (it did not before).
        let sink_len = result_sink_byte_len(&serialized_result, failure.error.as_deref());
        assert!(
            sink_len > MCP_MAX_SINGLE_RESULT_BYTES,
            "the per-result measure must include the error payload, got {sink_len}"
        );
        assert!(
            mcp_oversized_result_refusal(true, sink_len).is_some(),
            "a giant MCP error must be refused once measured correctly"
        );
    }

    #[test]
    fn result_sink_byte_len_of_a_success_is_just_the_result() {
        // A success carries no error; the sink size is the serialized result.
        assert_eq!(result_sink_byte_len("hello", None), 5);
    }

    #[test]
    fn small_mcp_error_passes_the_cap_but_is_still_charged() {
        // A normal-size MCP error is within the per-result cap (passes) but its
        // bytes still count toward the cumulative budget (success-agnostic).
        let len = result_sink_byte_len("null", Some("boom")); // 4 + 4
        assert_eq!(len, 8);
        assert!(mcp_oversized_result_refusal(true, len).is_none());
        assert_eq!(mcp_result_budget_charge(true, len), 8);
    }

    #[test]
    fn armed_decision_is_fail_closed_on_unresolved_server() {
        let allowlist = vec![allow("srv-id-1", &["read"])];
        // Server name could not be resolved to an id -> refused.
        assert!(!is_mcp_tool_armed(&allowlist, None, "read", false));
        // Empty allowlist -> nothing armed.
        assert!(!is_mcp_tool_armed(&[], Some("srv-id-1"), "read", false));
    }

    #[test]
    fn armed_decision_is_keyed_by_id_so_it_survives_rename() {
        // The allowlist stores the immutable id; the lookup takes the id
        // resolved from the (possibly renamed) display name. As long as the id
        // is stable, the rename is transparent.
        let allowlist = vec![allow("immutable-id", &["read"])];
        assert!(is_mcp_tool_armed(
            &allowlist,
            Some("immutable-id"),
            "read",
            false
        ));
    }

    /// Builds a `FunctionCallContext` for the gate tests.
    fn call_ctx<'a>(
        mcp: &'a Arc<MCPManager>,
        is_detached: bool,
        is_delegated: bool,
        allowlist: &'a [McpToolAllowlistEntry],
    ) -> FunctionCallContext<'a> {
        FunctionCallContext {
            local_tools: &[],
            mcp_manager: Some(mcp),
            workflow_id: "wf-test",
            validation_helper: None, // helper ABSENT on purpose
            require_file_confirmation: false,
            is_detached,
            is_delegated,
            mcp_tool_allowlist: allowlist,
        }
    }

    #[tokio::test]
    async fn detached_unarmed_mcp_call_refused_even_without_helper() {
        // Detached + empty allowlist + no helper -> immediate refusal (fail-closed).
        let (state, _g) = setup_test_state().await;
        let call = FunctionCall {
            id: "c1".to_string(),
            name: "mcp__some-server__dangerous_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        let ctx = call_ctx(&state.mcp_manager, true, false, &[]);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;
        assert!(!res.success, "detached unarmed MCP call must fail");
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("not enabled for this agent"),
            "refusal must come from the allowlist gate, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn attended_mcp_call_is_not_blocked_by_allowlist() {
        // Interactive (is_detached=false) + empty allowlist: the allowlist gate
        // must NOT fire. The call still fails (no real server) but for a
        // different reason — proving the attended path is not allowlist-gated.
        let (state, _g) = setup_test_state().await;
        let call = FunctionCall {
            id: "c2".to_string(),
            name: "mcp__some-server__some_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        let ctx = call_ctx(&state.mcp_manager, false, false, &[]);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;
        assert!(
            !res.error
                .as_deref()
                .unwrap_or("")
                .contains("not enabled for this agent"),
            "attended path must not be refused by the detached allowlist"
        );
    }

    #[test]
    fn agent_config_allowlist_round_trips_through_serde() {
        // R-SEC-4 persistence (Rust side): the nested array<object> survives a
        // serialize -> deserialize round-trip without dropping sub-keys.
        let config = agent_config_from(serde_json::json!({
            "id": "a1",
            "name": "A",
            "mcp_tool_allowlist": [
                { "server_id": "srv-1", "tools": ["read", "list"] }
            ],
        }));
        let json = serde_json::to_string(&config).unwrap();
        let back: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back.mcp_tool_allowlist.len(), 1);
        assert_eq!(back.mcp_tool_allowlist[0].server_id, "srv-1");
        assert_eq!(back.mcp_tool_allowlist[0].tools, vec!["read", "list"]);
    }

    #[test]
    fn armed_decision_isolates_each_server_in_a_multi_entry_allowlist() {
        // Edge: two distinct servers in the allowlist, a tool armed on only one.
        // The decision must not bleed across entries — the armed tool of `srv-a`
        // must NOT become armed for `srv-b`, and vice-versa.
        let allowlist = vec![allow("srv-a", &["read"]), allow("srv-b", &["exec"])];
        assert!(is_mcp_tool_armed(&allowlist, Some("srv-a"), "read", false));
        assert!(is_mcp_tool_armed(&allowlist, Some("srv-b"), "exec", false));
        // Cross-server leakage must not happen.
        assert!(
            !is_mcp_tool_armed(&allowlist, Some("srv-b"), "read", false),
            "srv-a's tool must not be armed for srv-b"
        );
        assert!(
            !is_mcp_tool_armed(&allowlist, Some("srv-a"), "exec", false),
            "srv-b's tool must not be armed for srv-a"
        );
    }

    #[tokio::test]
    async fn detached_refusal_is_audit_grade_and_still_counts_the_call() {
        // R-SEC-11: a detached refusal must (1) name the unattended/detached
        // context in the returned failure so the audit trail is unambiguous,
        // and (2) the refused call must still be recorded in `mcp_calls_made`
        // (the counter is bumped before the gate — relevant for the per-run MCP
        // budget/audit, R-SEC-10). The allowlist here references a SERVER ID
        // that no live server resolves to (e.g. a deleted server): the call's
        // server name cannot be resolved -> fail-closed, no panic.
        let (state, _g) = setup_test_state().await;
        let allowlist = vec![allow("ghost-server-id", &["read"])];
        let call = FunctionCall {
            id: "c3".to_string(),
            name: "mcp__some-unresolvable-server__read".to_string(),
            arguments: serde_json::json!({}),
        };
        let ctx = call_ctx(&state.mcp_manager, true, false, &allowlist);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;

        assert!(
            !res.success,
            "unresolved server in a detached run must be refused"
        );
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("unattended") && err.contains("detached"),
            "refusal must name the unattended/detached context for the audit trail, got: {err:?}"
        );
        assert!(
            mc.contains(&call.name),
            "the refused MCP call must still be recorded in mcp_calls_made for the per-run audit/budget, got: {mc:?}"
        );
    }

    #[tokio::test]
    async fn mcp_call_refused_when_run_byte_budget_exhausted_even_attended() {
        // R-SEC-10: the per-run byte budget is NOT coupled to detached mode — it
        // gates ATTENDED runs too, and trips BEFORE the tool is dispatched. Here
        // the run is attended (is_detached=false), no allowlist, no helper; the
        // budget is already at the ceiling, so the call is refused for budget
        // (not for the allowlist, which doesn't apply to attended runs).
        let (state, _g) = setup_test_state().await;
        let call = FunctionCall {
            id: "b1".to_string(),
            name: "mcp__some-server__read".to_string(),
            arguments: serde_json::json!({}),
        };
        let ctx = call_ctx(&state.mcp_manager, false, false, &[]);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res =
            execute_function_call(&call, &ctx, &mut tu, &mut mc, MCP_MAX_RESULT_BYTES_PER_RUN)
                .await;
        assert!(!res.success, "an over-budget MCP call must be refused");
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("result budget reached"),
            "the refusal must come from the per-run byte budget, got: {err:?}"
        );
        assert!(
            mc.contains(&call.name),
            "the refused attempt must still be recorded in mcp_calls_made, got: {mc:?}"
        );
    }

    #[tokio::test]
    async fn mcp_call_refused_when_run_call_cap_reached() {
        // R-SEC-10: once MCP_MAX_CALLS_PER_RUN calls have been made, the next is
        // refused (CHECK-BEFORE-REFUSE on the pre-call count). Attended run, no
        // helper, so the cap is the only thing that can refuse here.
        let (state, _g) = setup_test_state().await;
        let call = FunctionCall {
            id: "b2".to_string(),
            name: "mcp__some-server__read".to_string(),
            arguments: serde_json::json!({}),
        };
        let ctx = call_ctx(&state.mcp_manager, false, false, &[]);
        let mut tu = Vec::new();
        // Pre-fill the per-run counter to exactly the cap.
        let mut mc: Vec<String> = (0..MCP_MAX_CALLS_PER_RUN)
            .map(|i| format!("mcp__s__t{i}"))
            .collect();
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;
        assert!(
            !res.success,
            "the call past the per-run cap must be refused"
        );
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("call limit reached"),
            "the refusal must come from the per-run call cap, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn mcp_budget_does_not_fail_open_detached_gate() {
        // R-SEC-10 must NOT fail-open R-SEC-4: a detached UNARMED call that is
        // also over budget stays refused, and the detached gate runs FIRST so the
        // refusal is the audited allowlist refusal (not the budget one). The key
        // invariant is that an over-budget state never lets an unarmed detached
        // call through.
        let (state, _g) = setup_test_state().await;
        let call = FunctionCall {
            id: "b3".to_string(),
            name: "mcp__some-server__dangerous_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        let ctx = call_ctx(&state.mcp_manager, true, false, &[]);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res =
            execute_function_call(&call, &ctx, &mut tu, &mut mc, MCP_MAX_RESULT_BYTES_PER_RUN)
                .await;
        assert!(
            !res.success,
            "an unarmed detached call must stay refused even when over budget"
        );
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("not enabled for this agent"),
            "the detached allowlist gate must run first (audited refusal), got: {err:?}"
        );
    }

    #[tokio::test]
    async fn mcp_budget_gates_armed_detached_call_after_the_gate_passes() {
        // R-SEC-10 applies on the detached path too: an ARMED tool clears the
        // R-SEC-4 gate but is still refused once the run is over budget (the
        // budget check sits AFTER the gate, BEFORE execution).
        let (state, _g) = setup_test_state().await;
        state
            .mcp_manager
            .id_to_name
            .write()
            .await
            .insert("srv-budget-id".to_string(), "srv-budget".to_string());
        let call = FunctionCall {
            id: "b4".to_string(),
            name: "mcp__srv-budget__read".to_string(),
            arguments: serde_json::json!({}),
        };
        let allowlist = vec![allow("srv-budget-id", &["read"])];
        let ctx = call_ctx(&state.mcp_manager, true, false, &allowlist);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res =
            execute_function_call(&call, &ctx, &mut tu, &mut mc, MCP_MAX_RESULT_BYTES_PER_RUN)
                .await;
        assert!(!res.success, "an over-budget armed call must be refused");
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("result budget reached"),
            "an armed tool clears the gate but the budget still refuses it, got: {err:?}"
        );
    }

    /// R-SEC-4 TRANSITIVE gate — the hole the direct gate tests above do NOT
    /// catch. A sub-agent task built by a DETACHED parent (spawn / delegate /
    /// parallel call `stamp_detached(.., true)` on the task they hand the
    /// orchestrator) must resolve detached, so an MCP call to an UNARMED tool
    /// is refused IMMEDIATELY by the allowlist — never routed to the
    /// interactive modal (no human present in a detached run) and never
    /// executed. Before the fix the sub-agent path hard-coded
    /// `is_detached: false`, so this refusal never fired and the call either
    /// stalled on the modal or fell through fail-open.
    #[tokio::test]
    async fn detached_parent_subagent_unarmed_mcp_call_refused_immediately() {
        use crate::agents::core::agent::{stamp_detached, Task};

        let (state, _g) = setup_test_state().await;

        // The exact task shape a detached spawn/delegate produces.
        let mut context = serde_json::json!({
            "workflow_id": "wf-detached",
            "is_sub_agent": true,
        });
        stamp_detached(&mut context, true);
        let sub_task = Task {
            id: "sub".to_string(),
            description: "do work".to_string(),
            context,
        };
        assert!(
            sub_task.is_detached(),
            "a sub-agent task stamped by a detached parent must resolve detached"
        );

        // The sub-agent's tool loop derives is_detached from its task exactly as
        // LLMAgent::execute_with_mcp now does, then feeds the gate.
        let ctx = call_ctx(
            &state.mcp_manager,
            sub_task.is_detached(),
            sub_task.is_delegated(),
            &[],
        );
        let call = FunctionCall {
            id: "sc1".to_string(),
            name: "mcp__some-server__dangerous_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;

        assert!(
            !res.success,
            "a detached sub-agent's unarmed MCP call must be refused"
        );
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("not enabled for this agent") && err.contains("detached"),
            "refusal must come from the allowlist gate (not the modal, not execution), got: {err:?}"
        );
    }

    /// Converse: a sub-agent of an ATTENDED parent leaves `is_detached` unset,
    /// so the allowlist gate does NOT fire (its MCP calls follow the normal
    /// interactive path). Guards against over-refusing legitimate attended
    /// sub-agent MCP usage.
    #[tokio::test]
    async fn attended_parent_subagent_mcp_not_allowlist_gated() {
        use crate::agents::core::agent::{stamp_detached, Task};

        let (state, _g) = setup_test_state().await;
        let mut context = serde_json::json!({ "is_sub_agent": true });
        stamp_detached(&mut context, false);
        let sub_task = Task {
            id: "sub2".to_string(),
            description: "do work".to_string(),
            context,
        };
        assert!(!sub_task.is_detached());

        let ctx = call_ctx(
            &state.mcp_manager,
            sub_task.is_detached(),
            sub_task.is_delegated(),
            &[],
        );
        let call = FunctionCall {
            id: "sc2".to_string(),
            name: "mcp__some-server__some_tool".to_string(),
            arguments: serde_json::json!({}),
        };
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;
        assert!(
            !res.error
                .as_deref()
                .unwrap_or("")
                .contains("not enabled for this agent"),
            "an attended sub-agent must not be refused by the detached allowlist gate"
        );
    }

    /// R1 + 3.2 end-to-end at the gate, with a RESOLVABLE server (so the
    /// decision exercises the flag, not the fail-closed-on-unresolved path).
    /// `validation_helper` is None → proves the gate is UNCONDITIONAL (above
    /// ValidationMode): a delegated detached run cannot open MCP by letting the
    /// sub-agent validation skip/timeout. The armed-but-unflagged tool is
    /// refused for a DELEGATE, allowed for a DIRECT run, and allowed for the
    /// DELEGATE once the entry is flagged `allow_in_delegated_runs`.
    #[tokio::test]
    async fn delegated_run_requires_allow_in_delegated_runs_flag() {
        let (state, _g) = setup_test_state().await;
        // Make `srv-x` resolvable to an immutable id so the gate evaluates the
        // allowlist entry instead of fail-closing on an unresolved server.
        state
            .mcp_manager
            .id_to_name
            .write()
            .await
            .insert("srv-x-id".to_string(), "srv-x".to_string());

        let call = FunctionCall {
            id: "d1".to_string(),
            name: "mcp__srv-x__danger".to_string(),
            arguments: serde_json::json!({}),
        };

        // Armed for the agent, but NOT flagged for delegation (strict default).
        let strict = vec![allow("srv-x-id", &["danger"])];

        // DELEGATED + detached + strict entry → REFUSED (confused-deputy),
        // even though validation_helper is None (gate is unconditional → 3.2).
        let ctx = call_ctx(&state.mcp_manager, true, true, &strict);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0).await;
        assert!(!res.success, "delegated + strict entry must be refused");
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("not enabled for this agent") && err.contains("delegated"),
            "refusal must name the delegated detached scope for the audit trail, got: {err:?}"
        );

        // DIRECT detached run (is_delegated = false) with the SAME strict entry
        // → NOT refused by the allowlist (flag is irrelevant for direct runs).
        let ctx_direct = call_ctx(&state.mcp_manager, true, false, &strict);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx_direct, &mut tu, &mut mc, 0).await;
        assert!(
            !res.error
                .as_deref()
                .unwrap_or("")
                .contains("not enabled for this agent"),
            "a direct detached run must NOT be allowlist-refused for an armed tool (flag is delegation-only)"
        );

        // DELEGATED with the entry explicitly flagged → NOT refused by the gate
        // (it then proceeds to call_tool, which fails for an unrelated reason
        // since no live client backs the seeded name — but NOT the gate).
        let delegable = vec![allow_delegated("srv-x-id", &["danger"])];
        let ctx_ok = call_ctx(&state.mcp_manager, true, true, &delegable);
        let (mut tu, mut mc) = (Vec::new(), Vec::new());
        let res = execute_function_call(&call, &ctx_ok, &mut tu, &mut mc, 0).await;
        assert!(
            !res.error
                .as_deref()
                .unwrap_or("")
                .contains("not enabled for this agent"),
            "an explicitly delegation-armed tool must pass the gate in a delegated run"
        );
    }
}
