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
//! *Manager ownership checks and the central write gate.
use super::definitions::FunctionCallContext;
use super::governance::{
    classify_manager_op, manager_op_risk, manager_write_action, ManagerWriteAction,
    ManagerWriteRefusal,
};
use crate::constants::validation::MANAGER_MAX_WRITES_PER_RUN;
use crate::db::DBClient;
use crate::models::function_calling::{FunctionCall, FunctionCallResult};
use crate::models::ValidationType;
use crate::tools::{validation_helper::should_require_validation, Tool};
use serde_json::Value;
use std::sync::Arc;
use tracing::{info, warn};

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
pub(crate) async fn manager_owns_target(
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
pub(crate) fn build_manager_validation_details(
    tool_id: &str,
    operation: &str,
    args: &Value,
) -> Value {
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
pub(crate) fn manager_target_discriminant(args: &Value) -> String {
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
pub(crate) fn manager_refusal_message(
    reason: ManagerWriteRefusal,
    tool_id: &str,
    op: &str,
) -> String {
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
pub(crate) async fn manager_write_gate(
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
