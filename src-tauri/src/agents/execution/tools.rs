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
use crate::constants::validation::MANAGER_MAX_WRITES_PER_RUN;
use crate::db::DBClient;
use crate::mcp::MCPManager;
use crate::models::agent::AgentKind;
use crate::models::function_calling::{FunctionCall, FunctionCallResult};
use crate::models::mcp::MCPTool;
use crate::models::{AgentConfig, RiskLevel, ValidationType};
use crate::tools::{
    context::AgentToolContext,
    validation_helper::{is_destructive_file_op, should_require_validation, ValidationHelper},
    Tool, ToolDefinition, ToolFactory,
};
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

// ===========================================================================
// *Manager write governance: pure classification + decision.
//
// These replace the old `ReadOnlyToolGuard`/`harden_detached_writes` blanket
// refusal: a *Manager content write is no longer refused in a
// detached run, it transits the EXISTING validation flow classified by risk.
// The decision is concentrated in `manager_write_gate` (in `execute_function_call`)
// — the single enforcement point — so a tool-wrapper backstop (which would
// refuse the very Auto-execute path it is meant to allow, since the gate calls
// the wrapped tool) is unnecessary. The fail-closed guarantees asked of the
// backstop (refuse a validation-required detached write, refuse when no helper
// can enforce) are encoded in `manager_write_action` below and consumed by the
// gate, which is armed by the explicit `is_detached` flag (NOT `context.is_none()`
// — so `rerun_worker`, detached WITH a context, is covered).
// ===========================================================================

/// Source-single sets of write operations per *Manager tool. The classification
/// (`classify_manager_op`) is the ONLY reader; keeping them here makes the
/// covering-union test (CONTENT ∪ PRIVILEGE ∪ READONLY == dispatched ops) the
/// guard against a future op slipping through unclassified (fail-open).
const PROMPT_MANAGER_WRITE_OPS: &[&str] = &["create_prompt", "update_prompt"];
const SKILL_MANAGER_CONTENT_WRITE_OPS: &[&str] =
    &["create_skill", "update_skill", "restore_skill_version"];
const SKILL_MANAGER_PRIVILEGE_OPS: &[&str] = &["grant_skill_to_agent", "revoke_skill_from_agent"];
const WORKFLOW_MANAGER_WRITE_OPS: &[&str] = &[
    "rename_workflow",
    "create_workflow_folder",
    "move_workflow_to_folder",
];

/// Governance category of a *Manager tool operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ManagerOpClass {
    /// A content write governed by the EXISTING validation flow at the given
    /// risk (prompt/skill content = High; workflow organization = Low).
    Content(RiskLevel),
    /// A privilege-escalation write (grant/revoke a skill on an agent's
    /// allowlist) — always Critical so `always_confirm_high` catches it.
    Privilege,
    /// A read-only operation — never governed.
    ReadOnly,
}

/// Classifies a `(tool_id, operation)` pair into its governance category.
///
/// Pure and total: any operation not in the write/privilege sets — including a
/// read, an unknown op, or a non-*Manager tool — is `ReadOnly` (ungoverned).
/// The covering-union test pins the write/read partition per tool so a newly
/// dispatched write can never silently fall through as `ReadOnly` (fail-open).
pub(crate) fn classify_manager_op(tool_id: &str, op: &str) -> ManagerOpClass {
    match tool_id {
        "PromptManagerTool" if PROMPT_MANAGER_WRITE_OPS.contains(&op) => {
            ManagerOpClass::Content(RiskLevel::High)
        }
        "SkillManagerTool" if SKILL_MANAGER_CONTENT_WRITE_OPS.contains(&op) => {
            ManagerOpClass::Content(RiskLevel::High)
        }
        "SkillManagerTool" if SKILL_MANAGER_PRIVILEGE_OPS.contains(&op) => {
            ManagerOpClass::Privilege
        }
        "WorkflowManagerTool" if WORKFLOW_MANAGER_WRITE_OPS.contains(&op) => {
            ManagerOpClass::Content(RiskLevel::Low)
        }
        _ => ManagerOpClass::ReadOnly,
    }
}

/// Risk level a `ManagerOpClass` carries (Privilege = Critical).
pub(crate) fn manager_op_risk(class: &ManagerOpClass) -> Option<RiskLevel> {
    match class {
        ManagerOpClass::Content(risk) => Some(risk.clone()),
        ManagerOpClass::Privilege => Some(RiskLevel::Critical),
        ManagerOpClass::ReadOnly => None,
    }
}

/// Outcome of the single *Manager-write decision predicate, consumed by the
/// gate in `execute_function_call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ManagerWriteAction {
    /// Run the write without a modal, then record a `PreApproved` audit + toast.
    Execute,
    /// Attended run requiring validation → emit the modal (attached human).
    Validate,
    /// Refuse with a stable reason (audited).
    Refuse(ManagerWriteRefusal),
}

/// Why a *Manager write was refused. Each maps to a stable, secret-free message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ManagerWriteRefusal {
    /// The agent does not own the targeted resource (ownership scope).
    Scope,
    /// The per-run write cap was reached.
    Volume,
    /// Validation is required but the run is unattended (detached) — no human
    /// can answer the modal, so the detached policy applies directly.
    Detached,
    /// Validation is required, the run is attended, but no validation helper is
    /// available to enforce it — fail closed.
    NoHelper,
}

/// The SINGLE *Manager-write decision. Pure: consumed by the gate
/// and exercised in isolation by the unit tests.
///
/// Order matters: scope and volume are hard invariants checked first (a refusal
/// there never reaches the validation flow), then the validation requirement is
/// resolved. `requires_validation` is `should_require_validation(settings,
/// ManagerWrite | risk)` precomputed by the caller; `has_helper` is whether a
/// `ValidationHelper` is available to drive the modal/audit.
pub(crate) fn manager_write_action(
    requires_validation: bool,
    is_detached: bool,
    has_helper: bool,
    owns_target: bool,
    writes_so_far: usize,
    max_writes: usize,
) -> ManagerWriteAction {
    if !owns_target {
        return ManagerWriteAction::Refuse(ManagerWriteRefusal::Scope);
    }
    if writes_so_far >= max_writes {
        return ManagerWriteAction::Refuse(ManagerWriteRefusal::Volume);
    }
    if !requires_validation {
        return ManagerWriteAction::Execute;
    }
    // Validation IS required from here on.
    if is_detached {
        // No human to answer: apply the detached policy directly (refuse+audit),
        // never block the poll on a modal nobody can see (DoS-boot fix).
        return ManagerWriteAction::Refuse(ManagerWriteRefusal::Detached);
    }
    if !has_helper {
        // Attended but nothing can drive the modal → fail closed.
        return ManagerWriteAction::Refuse(ManagerWriteRefusal::NoHelper);
    }
    ManagerWriteAction::Validate
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
    // a delegation caller (they are already
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
    } else {
        // The *Manager tools (prompt / skill / workflow) are reserved to
        // Kanban supervisors. Strip them
        // defensively for a non-Kanban agent so they never appear in the tool
        // set — the tools ALSO self-gate at execution (`ensure_kanban`), so this
        // is defense-in-depth, not the sole guard. Without it a user could
        // persist a Manager tool on a standard agent and at least see it in the
        // system prompt (it would still refuse on call).
        tool_names.retain(|t| {
            t != "PromptManagerTool" && t != "SkillManagerTool" && t != "WorkflowManagerTool"
        });
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

    // A detached run NO LONGER wraps the *Manager tools read-only.
    // Write governance is now enforced centrally by `manager_write_gate` in
    // `execute_function_call`, armed by the explicit `is_detached` flag (which,
    // unlike the old `context.is_none()` wrapper trigger, also covers
    // `rerun_worker` — detached WITH a context). A detached content write
    // either passes per the user's validation settings (audited `PreApproved`)
    // or is refused there.
    tools
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

/// Pure decision: is `(server_id, tool)` armed in the agent's detached
/// allowlist? Keyed by the immutable `server_id` (never the display name).
/// `server_id == None` (the server name could not be resolved to an id) is
/// fail-closed → `false`. An empty allowlist arms nothing.
///
/// Delegated runs (`is_delegated`): when this detached run is a DELEGATED sub-agent
/// (DelegateTask / ParallelTasks), the matching entry must ALSO set
/// `allow_in_delegated_runs`. A DIRECT detached run (rerun-primary / analyze /
/// compose) or a SPAWNED sub-agent (`is_delegated == false`) ignores the flag,
/// preserving the non-delegated behavior. This closes the UNION confused-deputy where
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

/// Pure decision: may another MCP call proceed in this run?
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

/// Total bytes a tool result actually pushes to the run's sinks —
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

/// Pure decision: must a just-returned MCP result be replaced for
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
/// SUCCESS-AGNOSTIC by design: under this threat model a compromised MCP
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

/// Pure accounting: bytes a just-returned tool result contributes to the
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

/// Scope decision: does the calling agent OWN the resource a *Manager
/// write targets? Coupling the auto-improvement to the agent's own scope cuts
/// cross-agent skill poisoning while leaving genuine self-improvement open.
///
/// - `update_skill` / `restore_skill_version`: resolve the skill (`skill_id`)
///   to its current name and require it to be in the agent's `config.skills`.
///   An unresolved / not-owned skill is NOT owned (fail-closed → Scope refusal;
///   the inner tool would `NotFound` anyway).
/// - Everything else (create_skill = new content; prompt writes = no ownership,
///   risk accepted; workflow organization = not agent-scoped;
///   grant/revoke = bounded by the same-kind guard + Critical risk, recoverable
///   in AgentForm) is owned by construction.
async fn manager_owns_target(
    db: &DBClient,
    agent_skills: &[String],
    tool_id: &str,
    op: &str,
    args: &Value,
) -> bool {
    if tool_id == "SkillManagerTool" && (op == "update_skill" || op == "restore_skill_version") {
        let Some(skill_id) = args.get("skill_id").and_then(|v| v.as_str()) else {
            return false; // missing id → cannot own
        };
        let Ok(id) = crate::security::validate_uuid_field(skill_id, "skill_id") else {
            return false; // malformed id → not owned
        };
        let q = format!("SELECT name FROM skill:`{}`", id);
        let name = match db.query_json(&q).await {
            Ok(rows) => rows
                .into_iter()
                .next()
                .and_then(|r| r["name"].as_str().map(String::from)),
            Err(_) => None,
        };
        return name.is_some_and(|n| agent_skills.iter().any(|s| s == &n));
    }
    true
}

/// Builds the `details` payload for a *Manager write validation modal.
///
/// The authority fields (`tool_id`, `operation`) come from the BACKEND
/// classification — never echoed from the (untrusted) tool arguments — so an
/// injected arg cannot spoof what the human sees as the operation. The arg
/// `preview` is neutralized (bidi/control stripped, then truncated) and clearly
/// labeled untrusted on the frontend. The whole object is run through
/// `sanitize_for_surrealdb` BEFORE `create_and_wait_validation` because
/// `db.create` does NOT sanitize (a `\0` would panic).
fn build_manager_validation_details(tool_id: &str, operation: &str, args: &Value) -> Value {
    // Pick the most informative free-text arg for the preview, if present.
    let preview_src = ["content", "new_name", "name", "skill_name", "edit_summary"]
        .iter()
        .find_map(|k| args.get(*k).and_then(|v| v.as_str()))
        .unwrap_or("");
    let details = serde_json::json!({
        // Authority — backend-controlled, dominant in the modal.
        "tool_id": tool_id,
        "operation": operation,
        // Untrusted, agent-supplied — neutralized + labeled on the frontend.
        "agent_preview": crate::tools::utils::neutralize_for_display(preview_src, 200),
    });
    crate::db::sanitize_for_surrealdb(details)
}

/// Per-resource discriminant for the `record_preapproved` dedup key, so the
/// audit counts distinct resources (not just `(tool, op)`). Picks the first
/// stable identifier present in the args (id before name). Empty when none.
fn manager_target_discriminant(args: &Value) -> String {
    [
        "prompt_id",
        "skill_id",
        "version_id",
        "name",
        "skill_name",
        "target_agent_id",
    ]
    .iter()
    .find_map(|k| args.get(*k).and_then(|v| v.as_str()))
    .unwrap_or("")
    .to_string()
}

/// Stable, secret-free refusal message for a `ManagerWriteRefusal`.
fn manager_refusal_message(reason: ManagerWriteRefusal, tool_id: &str, op: &str) -> String {
    match reason {
        ManagerWriteRefusal::Scope => format!(
            "Refused: this agent may only modify its OWN skills (operation '{}' on {} targets a \
             resource it does not own).",
            op, tool_id
        ),
        ManagerWriteRefusal::Volume => format!(
            "Refused: per-run *Manager write limit reached ({MANAGER_MAX_WRITES_PER_RUN}); no \
             further self-improvement writes are permitted in this run."
        ),
        ManagerWriteRefusal::Detached => format!(
            "Refused: operation '{}' on {} requires validation but the run is unattended \
             (detached) — no human can approve it.",
            op, tool_id
        ),
        ManagerWriteRefusal::NoHelper => format!(
            "Refused: operation '{}' on {} requires validation but no approval channel is \
             available.",
            op, tool_id
        ),
    }
}

/// The SINGLE *Manager-write enforcement point. Returns `Some(result)`
/// when `call` is a *Manager WRITE op (fully handled here: executed + audited,
/// validated, or refused); returns `None` when it is not a *Manager write (a
/// read or a non-*Manager tool) so the caller proceeds with the normal flow.
///
/// Centralizes what the old read-only wrapper backstop did, but governed by the
/// user's EXISTING validation settings instead of a blanket refusal — armed by
/// the explicit `ctx.is_detached` (covers `rerun_worker`).
async fn manager_write_gate(
    call: &FunctionCall,
    ctx: &FunctionCallContext<'_>,
    tool: &Arc<dyn Tool>,
    manager_writes_made: &mut usize,
    start: std::time::Instant,
) -> Option<FunctionCallResult> {
    let op = call
        .arguments
        .get("operation")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let class = classify_manager_op(&call.name, op);
    let risk = match manager_op_risk(&class) {
        Some(r) => r,
        None => return None, // ReadOnly / non-Manager → normal flow.
    };

    // The decision needs a helper to load settings, look up ownership, drive the
    // modal and write the audit. Without one we cannot enforce: fail closed in a
    // detached run (defense-in-depth), otherwise defer to the normal flow.
    let Some(helper) = ctx.validation_helper else {
        if ctx.is_detached {
            warn!(
                tool = %call.name, operation = %op,
                "Manager write refused: detached run with no validation helper (fail-closed)"
            );
            return Some(FunctionCallResult::failure(
                &call.id,
                &call.name,
                manager_refusal_message(ManagerWriteRefusal::NoHelper, &call.name, op),
            ));
        }
        return None;
    };

    let owns_target = manager_owns_target(
        &helper.db,
        ctx.agent_skills,
        &call.name,
        op,
        &call.arguments,
    )
    .await;
    let settings = helper.load_validation_settings().await;
    // *Manager content writes are governed UNIFORMLY by the user's existing
    // settings, exactly like prompts: a `create_skill` (incl. one a Kanban
    // supervisor composes FOR a worker — the legitimate auto-improvement flow in
    // compose_card) passes in Auto + always_confirm_high OFF (PreApproved), and
    // requires review when always_confirm_high is ON (High → detached refuse /
    // attended modal). The cross-agent skill-creation risk is accepted-and-named,
    // consistent with prompts (option B) and grant/revoke:
    // mitigated by Kanban-only gating + audit + toast + manual recovery.
    let requires_validation =
        should_require_validation(&settings, &ValidationType::ManagerWrite, &risk);

    let action = manager_write_action(
        requires_validation,
        ctx.is_detached,
        true, // helper present (checked above)
        owns_target,
        *manager_writes_made,
        MANAGER_MAX_WRITES_PER_RUN,
    );

    match action {
        ManagerWriteAction::Execute => {
            // Auto / permissive: run it, then record PreApproved + opportunistic
            // toast. Count it toward the per-run cap (self-grants included).
            *manager_writes_made += 1;
            match tool.execute(call.arguments.clone()).await {
                Ok(result) => {
                    info!(tool = %call.name, operation = %op, "Manager write pre-approved (auto)");
                    helper
                        .record_preapproved(
                            &call.name,
                            op,
                            &manager_target_discriminant(&call.arguments),
                            &risk,
                            ctx.workflow_id,
                        )
                        .await;
                    Some(
                        FunctionCallResult::success(&call.id, &call.name, result)
                            .with_execution_time(start.elapsed().as_millis() as u64),
                    )
                }
                Err(e) => {
                    warn!(tool = %call.name, error = %e, "Manager write (pre-approved) failed");
                    Some(FunctionCallResult::failure(
                        &call.id,
                        &call.name,
                        e.to_string(),
                    ))
                }
            }
        }
        ManagerWriteAction::Validate => {
            // Attended: emit the modal with a neutralized, authority-dominant
            // payload (sanitized for the DB). On approval, execute + count.
            let validation_id = uuid::Uuid::new_v4().to_string();
            let details = build_manager_validation_details(&call.name, op, &call.arguments);
            let description = format!("{}: {}", call.name, op);
            match helper
                .create_and_wait_validation(
                    &validation_id,
                    ctx.workflow_id,
                    ValidationType::ManagerWrite,
                    &description,
                    details,
                    risk,
                    false, // attended (manager_write_action only reaches here when !is_detached)
                )
                .await
            {
                Ok(()) => {
                    *manager_writes_made += 1;
                    match tool.execute(call.arguments.clone()).await {
                        Ok(result) => Some(
                            FunctionCallResult::success(&call.id, &call.name, result)
                                .with_execution_time(start.elapsed().as_millis() as u64),
                        ),
                        Err(e) => Some(FunctionCallResult::failure(
                            &call.id,
                            &call.name,
                            e.to_string(),
                        )),
                    }
                }
                Err(e) => {
                    warn!(tool = %call.name, error = %e, "Manager write validation rejected");
                    Some(FunctionCallResult::failure(
                        &call.id,
                        &call.name,
                        e.to_string(),
                    ))
                }
            }
        }
        ManagerWriteAction::Refuse(reason) => {
            let msg = manager_refusal_message(reason, &call.name, op);
            warn!(tool = %call.name, operation = %op, reason = ?reason, "Manager write refused");
            // Audit the refusal so a no-review block is visible (best-effort).
            let reason_label = match reason {
                ManagerWriteRefusal::Scope => "agent does not own target (scope)",
                ManagerWriteRefusal::Volume => "per-run manager write cap reached",
                ManagerWriteRefusal::Detached => "validation required in a detached run",
                ManagerWriteRefusal::NoHelper => "validation required, no approval channel",
            };
            helper
                .record_manager_refusal(&call.name, op, reason_label, &risk, ctx.workflow_id)
                .await;
            Some(FunctionCallResult::failure(&call.id, &call.name, msg))
        }
    }
}

/// Executes a single function call (local or MCP tool).
///
/// `mcp_result_bytes` is the cumulative sink byte size (serialized result +
/// error) of all prior MCP results in this run — success AND error — consulted by
/// the per-run budget gate (the caller accumulates it in `iteration.rs`
/// after each MCP call via [`result_sink_byte_len`] + [`mcp_result_budget_charge`]).
///
/// `manager_writes_made` is the run-scoped count of *Manager content/privilege
/// writes already executed, consulted + incremented by the *Manager write gate
/// for the per-run volume cap.
pub(crate) async fn execute_function_call(
    call: &FunctionCall,
    ctx: &FunctionCallContext<'_>,
    tools_used: &mut Vec<String>,
    mcp_calls_made: &mut Vec<String>,
    mcp_result_bytes: usize,
    manager_writes_made: &mut usize,
) -> FunctionCallResult {
    let start = std::time::Instant::now();

    // Check if MCP tool
    if let Some((server, tool)) = call.parse_mcp_name() {
        // Execute via MCP
        if let Some(mcp) = ctx.mcp_manager {
            // Snapshot the pre-call count BEFORE recording this
            // attempt, so the per-run cap (CHECK-BEFORE-REFUSE) counts only
            // prior calls and allows EXACTLY MCP_MAX_CALLS_PER_RUN.
            let calls_before = mcp_calls_made.len();
            mcp_calls_made.push(call.name.clone());

            // In a DETACHED run there is no human to answer the
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
                    // Structured refusal audit (no secret). `delegated`
                    // distinguishes the confused-deputy refusal (armed for the
                    // agent but not flagged `allow_in_delegated_runs`) from a
                    // plain unarmed refusal.
                    warn!(
                        server = %server,
                        server_id = server_id.as_deref().unwrap_or("<unknown>"),
                        tool = %tool,
                        delegated = ctx.is_delegated,
                        "MCP tool refused: not armed for this agent in a detached run"
                    );
                    // Persist the refusal into the EXISTING audit log
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
                // Armed: proceed without the interactive modal (no human
                // present). Record the pre-approved execution into the audit
                // log so an unattended MCP call that ran by allowlist policy is
                // traceable (decided_by = PreApproved). Best-effort + run-scoped
                // dedup per (tool, op) inside the helper; MCP is Medium risk so
                // no toast is emitted.
                if let Some(helper) = ctx.validation_helper {
                    helper
                        .record_preapproved(
                            &call.name,
                            "armed_mcp_call",
                            // Discriminant empty: the full mcp__server__tool name
                            // is already in `tool_name`, so dedup is per-tool/run.
                            "",
                            &RiskLevel::Medium,
                            ctx.workflow_id,
                        )
                        .await;
                }
            }

            // Per-run cap + cumulative byte budget. Checked AFTER the
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
                            // This branch only runs for an ATTENDED run
                            // (`if !ctx.is_detached`), so the modal has a human.
                            false,
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

            // *Manager WRITE gate (prompt/skill/workflow). Runs BEFORE
            // the generic tool validation: a write op is fully handled here
            // (executed + audited PreApproved, validated via modal, or refused).
            // A read op / non-Manager tool returns None and falls through.
            if let Some(result) =
                manager_write_gate(call, ctx, tool, manager_writes_made, start).await
            {
                return result;
            }

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
                                    ctx.is_detached,
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
                            ctx.is_detached,
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
    /// even with a full context present (strict Kanban separation). The
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

    /// A non-Kanban (kind = None) agent must NOT receive ANY *Manager
    /// tool — they are reserved to Kanban supervisors. The defensive strip in
    /// `create_local_tools` removes them from the tool set entirely, even when
    /// the user persisted them on the config.
    #[tokio::test]
    async fn non_kanban_agent_does_not_receive_manager_tools() {
        let (state, _g) = setup_test_state().await;
        let context = AgentToolContext::from_app_state_full(&state);
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "Std",
            "tools": [
                "MemoryTool",
                "PromptManagerTool",
                "SkillManagerTool",
                "WorkflowManagerTool"
            ],
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
        for forbidden in [
            "PromptManagerTool",
            "SkillManagerTool",
            "WorkflowManagerTool",
        ] {
            assert!(
                !ids.contains(&forbidden.to_string()),
                "non-Kanban agent must NOT receive {forbidden}, got {ids:?}"
            );
        }
        assert!(
            ids.contains(&"MemoryTool".to_string()),
            "non-Kanban agent keeps its non-Manager tools, got {ids:?}"
        );
    }

    /// A Kanban-kind agent DOES receive the *Manager tools (they are the
    /// supervisors that curate prompts/skills/workflows). The detached
    /// classification is no longer about wrapping — write governance moved to
    /// the validation gate in `execute_function_call`.
    #[tokio::test]
    async fn kanban_agent_receives_manager_tools() {
        let (state, _g) = setup_test_state().await;
        let config = agent_config_from(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(),
            "name": "KanbanDetached",
            "kind": "kanban",
            "tools": ["PromptManagerTool", "SkillManagerTool", "WorkflowManagerTool"],
        }));

        // Detached (no context) Kanban analyze/compose path.
        let tools = create_local_tools(
            &config,
            Some(&state.tool_factory),
            None,
            Some("wf-1".to_string()),
            false,
            None,
        )
        .await;
        let ids = tool_ids(&tools);
        for expected in [
            "PromptManagerTool",
            "SkillManagerTool",
            "WorkflowManagerTool",
        ] {
            assert!(
                ids.contains(&expected.to_string()),
                "Kanban agent must receive {expected}, got {ids:?}"
            );
        }
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
    // *Manager write classification + decision (pure).
    // ----------------------------------------------------------------------

    #[test]
    fn classify_manager_op_partitions_each_tool() {
        use ManagerOpClass::*;
        // Prompt: writes are High content, reads are ReadOnly.
        assert_eq!(
            classify_manager_op("PromptManagerTool", "create_prompt"),
            Content(RiskLevel::High)
        );
        assert_eq!(
            classify_manager_op("PromptManagerTool", "update_prompt"),
            Content(RiskLevel::High)
        );
        assert_eq!(
            classify_manager_op("PromptManagerTool", "list_prompts"),
            ReadOnly
        );
        // Skill: content writes High, privilege ops Privilege, reads ReadOnly.
        assert_eq!(
            classify_manager_op("SkillManagerTool", "restore_skill_version"),
            Content(RiskLevel::High)
        );
        assert_eq!(
            classify_manager_op("SkillManagerTool", "grant_skill_to_agent"),
            Privilege
        );
        assert_eq!(
            classify_manager_op("SkillManagerTool", "revoke_skill_from_agent"),
            Privilege
        );
        assert_eq!(
            classify_manager_op("SkillManagerTool", "read_skill"),
            ReadOnly
        );
        // Workflow organization ops are Low content.
        assert_eq!(
            classify_manager_op("WorkflowManagerTool", "rename_workflow"),
            Content(RiskLevel::Low)
        );
        assert_eq!(
            classify_manager_op("WorkflowManagerTool", "list_workflows"),
            ReadOnly
        );
        // Non-Manager tool / unknown op are ungoverned.
        assert_eq!(classify_manager_op("MemoryTool", "create_prompt"), ReadOnly);
        assert_eq!(
            classify_manager_op("PromptManagerTool", "nonexistent_op"),
            ReadOnly
        );
    }

    /// Every operation a *Manager tool actually dispatches must
    /// be covered exactly once by CONTENT ∪ PRIVILEGE ∪ READONLY. An op present
    /// in the dispatch `match` but absent from the classification would be a
    /// fail-open silent write once the hard refusal is replaced by a flow.
    #[test]
    fn manager_op_classification_is_a_covering_partition() {
        // The dispatched ops are mirrored from each tool's execute() match arms;
        // if a tool gains an op, this list must grow with it or the assertion
        // below trips (CI-blocking).
        let cases: &[(&str, &[&str], &[&str])] = &[
            (
                "PromptManagerTool",
                // dispatched ops
                &[
                    "list_prompts",
                    "get_prompt",
                    "create_prompt",
                    "update_prompt",
                ],
                // expected WRITE ops (Content or Privilege)
                PROMPT_MANAGER_WRITE_OPS,
            ),
            (
                "SkillManagerTool",
                &[
                    "list_skills",
                    "read_skill",
                    "create_skill",
                    "update_skill",
                    "list_skill_versions",
                    "restore_skill_version",
                    "grant_skill_to_agent",
                    "revoke_skill_from_agent",
                ],
                // content + privilege writes
                &[
                    "create_skill",
                    "update_skill",
                    "restore_skill_version",
                    "grant_skill_to_agent",
                    "revoke_skill_from_agent",
                ],
            ),
            (
                "WorkflowManagerTool",
                &[
                    "list_workflows",
                    "rename_workflow",
                    "list_workflow_folders",
                    "create_workflow_folder",
                    "move_workflow_to_folder",
                    "read_workflow",
                    "list_workflow_errors",
                    "list_workflow_sub_agents",
                ],
                WORKFLOW_MANAGER_WRITE_OPS,
            ),
        ];

        for (tool, dispatched, expected_writes) in cases {
            for op in *dispatched {
                let class = classify_manager_op(tool, op);
                let is_write = matches!(
                    class,
                    ManagerOpClass::Content(_) | ManagerOpClass::Privilege
                );
                let should_be_write = expected_writes.contains(op);
                assert_eq!(
                    is_write, should_be_write,
                    "{tool}.{op}: classified write={is_write} but expected write={should_be_write} \
                     — every dispatched op must be in exactly one of CONTENT∪PRIVILEGE (write) \
                     or READONLY (read)"
                );
            }
        }
    }

    #[test]
    fn manager_write_action_executes_when_validation_not_required() {
        // Auto + accept-high (requires_validation = false) → execute directly.
        assert_eq!(
            manager_write_action(false, false, true, true, 0, MANAGER_MAX_WRITES_PER_RUN),
            ManagerWriteAction::Execute
        );
        // Even detached: no validation required → execute (the nominal case).
        assert_eq!(
            manager_write_action(false, true, true, true, 0, MANAGER_MAX_WRITES_PER_RUN),
            ManagerWriteAction::Execute
        );
    }

    #[test]
    fn manager_write_action_validates_attended_else_refuses_detached() {
        // Attended + requires validation + helper present → modal.
        assert_eq!(
            manager_write_action(true, false, true, true, 0, MANAGER_MAX_WRITES_PER_RUN),
            ManagerWriteAction::Validate
        );
        // Detached + requires validation → refuse (court-circuit): no modal.
        assert_eq!(
            manager_write_action(true, true, true, true, 0, MANAGER_MAX_WRITES_PER_RUN),
            ManagerWriteAction::Refuse(ManagerWriteRefusal::Detached)
        );
        // Attended + requires validation but NO helper → fail closed.
        assert_eq!(
            manager_write_action(true, false, false, true, 0, MANAGER_MAX_WRITES_PER_RUN),
            ManagerWriteAction::Refuse(ManagerWriteRefusal::NoHelper)
        );
    }

    #[test]
    fn manager_write_action_refuses_scope_and_volume_first() {
        // Scope violation refused regardless of validation requirement.
        assert_eq!(
            manager_write_action(false, false, true, false, 0, MANAGER_MAX_WRITES_PER_RUN),
            ManagerWriteAction::Refuse(ManagerWriteRefusal::Scope)
        );
        // Volume cap reached → refuse (owns_target true, validation off).
        assert_eq!(
            manager_write_action(
                false,
                false,
                true,
                true,
                MANAGER_MAX_WRITES_PER_RUN,
                MANAGER_MAX_WRITES_PER_RUN
            ),
            ManagerWriteAction::Refuse(ManagerWriteRefusal::Volume)
        );
        // Scope takes precedence over volume when both would trip.
        assert_eq!(
            manager_write_action(
                false,
                false,
                true,
                false,
                MANAGER_MAX_WRITES_PER_RUN,
                MANAGER_MAX_WRITES_PER_RUN
            ),
            ManagerWriteAction::Refuse(ManagerWriteRefusal::Scope)
        );
    }

    #[test]
    fn manager_op_risk_maps_privilege_to_critical() {
        assert_eq!(
            manager_op_risk(&ManagerOpClass::Content(RiskLevel::High)),
            Some(RiskLevel::High)
        );
        assert_eq!(
            manager_op_risk(&ManagerOpClass::Privilege),
            Some(RiskLevel::Critical)
        );
        assert_eq!(manager_op_risk(&ManagerOpClass::ReadOnly), None);
    }

    // ----------------------------------------------------------------------
    // *Manager write gate, end-to-end through execute_function_call.
    // ----------------------------------------------------------------------

    /// Builds a FunctionCallContext for the gate integration tests.
    fn manager_ctx<'a>(
        helper: &'a ValidationHelper,
        local_tools: &'a [Arc<dyn Tool>],
        is_detached: bool,
        agent_skills: &'a [String],
    ) -> FunctionCallContext<'a> {
        FunctionCallContext {
            local_tools,
            mcp_manager: None,
            workflow_id: "wf-mgr",
            validation_helper: Some(helper),
            require_file_confirmation: false,
            is_detached,
            is_delegated: false,
            mcp_tool_allowlist: &[],
            agent_skills,
        }
    }

    async fn count_preapproved_audit(db: &crate::db::DBClient) -> usize {
        let rows = db
            .query_json("SELECT decided_by FROM validation_audit")
            .await
            .unwrap_or_default();
        rows.iter()
            .filter(|r| r.get("decided_by").and_then(|v| v.as_str()) == Some("pre_approved"))
            .count()
    }

    /// Nominal: a DETACHED Kanban content write executes under the DEFAULT
    /// validation settings (Selective + tools unchecked → no validation
    /// required) and is recorded as a `PreApproved` audit entry. This is the
    /// auto-improvement path now permitted (the old wrapper refused it).
    #[tokio::test]
    async fn detached_manager_write_executes_and_audits_preapproved() {
        let (state, _g) = setup_test_state().await;
        let helper = ValidationHelper::new(state.db.clone(), None);
        let pm: Arc<dyn Tool> = Arc::new(crate::tools::prompt_manager::PromptManagerTool::new(
            state.db.clone(),
            uuid::Uuid::new_v4().to_string(),
            Some(AgentKind::Kanban),
        ));
        let tools = vec![pm];
        let ctx = manager_ctx(&helper, &tools, true, &[]);

        let call = FunctionCall {
            id: "m1".to_string(),
            name: "PromptManagerTool".to_string(),
            arguments: serde_json::json!({
                "operation": "create_prompt",
                "name": "auto-improved",
                "content": "Refined {{x}}",
                "category": "custom"
            }),
        };
        let (mut tu, mut mc, mut mw) = (Vec::new(), Vec::new(), 0usize);
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut mw).await;
        assert!(
            res.success,
            "detached create_prompt must execute under default settings, got {:?}",
            res.error
        );
        assert_eq!(mw, 1, "the write must count toward the per-run cap");
        assert_eq!(
            count_preapproved_audit(&state.db).await,
            1,
            "an executed manager write must leave a PreApproved audit row"
        );
    }

    /// Scope: a DETACHED update_skill targeting a skill the agent
    /// does NOT own (its name is not in `config.skills`) is refused for scope —
    /// regardless of the validation mode — and audited.
    #[tokio::test]
    async fn detached_skill_update_outside_scope_is_refused() {
        let (state, _g) = setup_test_state().await;
        let helper = ValidationHelper::new(state.db.clone(), None);
        let sm: Arc<dyn Tool> = Arc::new(crate::tools::skill_manager::SkillManagerTool::new(
            state.db.clone(),
            uuid::Uuid::new_v4().to_string(),
            Some(AgentKind::Kanban),
        ));
        let tools = vec![sm];
        // agent_skills empty → no skill is owned.
        let ctx = manager_ctx(&helper, &tools, true, &[]);

        let call = FunctionCall {
            id: "m2".to_string(),
            name: "SkillManagerTool".to_string(),
            arguments: serde_json::json!({
                "operation": "update_skill",
                "skill_id": uuid::Uuid::new_v4().to_string(),
                "content": "poisoned",
                "edit_summary": "x"
            }),
        };
        let (mut tu, mut mc, mut mw) = (Vec::new(), Vec::new(), 0usize);
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut mw).await;
        assert!(!res.success, "an out-of-scope skill write must be refused");
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("OWN skills"),
            "refusal must name the scope rule, got: {err:?}"
        );
        assert_eq!(mw, 0, "a refused write must NOT count toward the cap");
    }

    /// At the gate: a DETACHED content write that WOULD require validation
    /// (Manual mode) is refused immediately (no modal), not executed.
    #[tokio::test]
    async fn detached_manager_write_refused_when_validation_required() {
        let (state, _g) = setup_test_state().await;
        // Seed Manual mode so ManagerWrite (High) requires validation.
        let upsert = "UPSERT settings:`settings:validation` CONTENT { id: 'settings:validation', \
             config: { mode: 'manual', selectiveConfig: { tools: false, subAgents: true, mcp: true, \
             fileOps: true, dbOps: true }, riskThresholds: { autoApproveLow: true, \
             alwaysConfirmHigh: false }, timeoutSeconds: 60, timeoutBehavior: 'reject', \
             audit: { enableLogging: true, retentionDays: 30 }, updatedAt: time::now() } }";
        state
            .db
            .execute(upsert)
            .await
            .expect("seed manual settings");

        let helper = ValidationHelper::new(state.db.clone(), None);
        let pm: Arc<dyn Tool> = Arc::new(crate::tools::prompt_manager::PromptManagerTool::new(
            state.db.clone(),
            uuid::Uuid::new_v4().to_string(),
            Some(AgentKind::Kanban),
        ));
        let tools = vec![pm];
        let ctx = manager_ctx(&helper, &tools, true, &[]);

        let call = FunctionCall {
            id: "m3".to_string(),
            name: "PromptManagerTool".to_string(),
            arguments: serde_json::json!({
                "operation": "create_prompt", "name": "x", "content": "y", "category": "custom"
            }),
        };
        let started = std::time::Instant::now();
        let (mut tu, mut mc, mut mw) = (Vec::new(), Vec::new(), 0usize);
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut mw).await;
        assert!(
            !res.success,
            "a detached validation-required write must be refused"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "detached refusal must be immediate (no modal/poll)"
        );
        assert_eq!(mw, 0, "a refused write must NOT count toward the cap");
    }

    /// `create_skill` is governed UNIFORMLY like `create_prompt`: in a detached
    /// run under default (Auto-permissive) settings it PASSES the gate (it is the
    /// legitimate compose_card auto-improvement flow — a Kanban supervisor
    /// composing a skill, possibly for a worker). It is NOT refused for being
    /// cross-agent; the gate lets it through to the tool (which here fails for an
    /// UNRELATED reason — the target agent is not seeded — proving the gate did
    /// not refuse it). Mirrors the create_prompt behavior the user expects.
    #[tokio::test]
    async fn detached_create_skill_passes_gate_in_auto_like_prompt() {
        let (state, _g) = setup_test_state().await;
        let helper = ValidationHelper::new(state.db.clone(), None);
        let sm: Arc<dyn Tool> = Arc::new(crate::tools::skill_manager::SkillManagerTool::new(
            state.db.clone(),
            uuid::Uuid::new_v4().to_string(),
            Some(AgentKind::Kanban),
        ));
        let tools = vec![sm];
        let ctx = manager_ctx(&helper, &tools, true, &[]);

        let call = FunctionCall {
            id: "cs1".to_string(),
            name: "SkillManagerTool".to_string(),
            arguments: serde_json::json!({
                "operation": "create_skill",
                "name": "composed-skill",
                "content": "improve the worker",
                "description": "d",
                // A worker target distinct from the caller — must NOT be refused.
                "target_agent_id": uuid::Uuid::new_v4().to_string()
            }),
        };
        let (mut tu, mut mc, mut mw) = (Vec::new(), Vec::new(), 0usize);
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut mw).await;
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            !err.contains("unattended") && !err.contains("requires validation"),
            "detached create_skill in Auto must pass the gate like create_prompt, got: {err:?}"
        );
    }

    // ----------------------------------------------------------------------
    // Detached MCP tool allowlist gate
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
    /// (`allow_in_delegated_runs = true`).
    fn allow_delegated(server_id: &str, tools: &[&str]) -> McpToolAllowlistEntry {
        McpToolAllowlistEntry {
            server_id: server_id.to_string(),
            tools: tools.iter().map(|s| s.to_string()).collect(),
            allow_in_delegated_runs: true,
        }
    }

    /// Delegated-flag matrix on the pure decision: the per-entry `allow_in_delegated_runs`
    /// flag ONLY gates DELEGATED runs (Delegate/Parallel). A DIRECT detached run
    /// (rerun-primary / analyze / compose / Spawn-clone, `is_delegated = false`)
    /// ignores the flag — its behavior is exactly the non-delegated armed check.
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
        // A run well under both ceilings proceeds.
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
        // An MCP result past the per-result cap is replaced by an
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
        // (the threat model is a compromised MCP server, not local tools).
        assert!(mcp_oversized_result_refusal(false, MCP_MAX_SINGLE_RESULT_BYTES * 4).is_none());
    }

    #[test]
    fn mcp_result_budget_charge_counts_every_mcp_result() {
        // Twin fix: the cumulative budget charges EVERY MCP result, not
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
            agent_skills: &[],
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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;
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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;
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
        // Allowlist persistence (Rust side): the nested array<object> survives a
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
        // A detached refusal must (1) name the unattended/detached
        // context in the returned failure so the audit trail is unambiguous,
        // and (2) the refused call must still be recorded in `mcp_calls_made`
        // (the counter is bumped before the gate — relevant for the per-run MCP
        // budget/audit). The allowlist here references a SERVER ID
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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;

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
        // The per-run byte budget is NOT coupled to detached mode — it
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
        let res = execute_function_call(
            &call,
            &ctx,
            &mut tu,
            &mut mc,
            MCP_MAX_RESULT_BYTES_PER_RUN,
            &mut 0usize,
        )
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
        // Once MCP_MAX_CALLS_PER_RUN calls have been made, the next is
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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;
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
        // The byte budget must NOT fail-open the detached gate: a detached UNARMED call that is
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
        let res = execute_function_call(
            &call,
            &ctx,
            &mut tu,
            &mut mc,
            MCP_MAX_RESULT_BYTES_PER_RUN,
            &mut 0usize,
        )
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
        // The byte budget applies on the detached path too: an ARMED tool clears the
        // allowlist gate but is still refused once the run is over budget (the
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
        let res = execute_function_call(
            &call,
            &ctx,
            &mut tu,
            &mut mc,
            MCP_MAX_RESULT_BYTES_PER_RUN,
            &mut 0usize,
        )
        .await;
        assert!(!res.success, "an over-budget armed call must be refused");
        let err = res.error.as_deref().unwrap_or("");
        assert!(
            err.contains("result budget reached"),
            "an armed tool clears the gate but the budget still refuses it, got: {err:?}"
        );
    }

    /// Transitive gate — the hole the direct gate tests above do NOT
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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;

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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;
        assert!(
            !res.error
                .as_deref()
                .unwrap_or("")
                .contains("not enabled for this agent"),
            "an attended sub-agent must not be refused by the detached allowlist gate"
        );
    }

    /// Delegated flag end-to-end at the gate, with a RESOLVABLE server (so the
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
        let res = execute_function_call(&call, &ctx, &mut tu, &mut mc, 0, &mut 0usize).await;
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
        let res = execute_function_call(&call, &ctx_direct, &mut tu, &mut mc, 0, &mut 0usize).await;
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
        let res = execute_function_call(&call, &ctx_ok, &mut tu, &mut mc, 0, &mut 0usize).await;
        assert!(
            !res.error
                .as_deref()
                .unwrap_or("")
                .contains("not enabled for this agent"),
            "an explicitly delegation-armed tool must pass the gate in a delegated run"
        );
    }
}
