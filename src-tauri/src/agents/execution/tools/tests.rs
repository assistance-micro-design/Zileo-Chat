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
//! Tests for tool governance, factory, permissions and dispatch.
use super::*;
use crate::constants::mcp::{
    MCP_MAX_CALLS_PER_RUN, MCP_MAX_RESULT_BYTES_PER_RUN, MCP_MAX_SINGLE_RESULT_BYTES,
};
use crate::constants::validation::MANAGER_MAX_WRITES_PER_RUN;
use crate::mcp::MCPManager;
use crate::models::agent::AgentKind;
use crate::models::function_calling::FunctionCallResult;
use crate::models::{AgentConfig, RiskLevel};
use crate::tools::{validation_helper::ValidationHelper, Tool};
use std::sync::Arc;

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
        mcp_run_budget_check(MCP_MAX_CALLS_PER_RUN - 1, MCP_MAX_RESULT_BYTES_PER_RUN - 1).is_ok()
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
