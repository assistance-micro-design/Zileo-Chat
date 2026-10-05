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
//! Pure *Manager write governance: classification + decision.
use crate::models::RiskLevel;

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
pub(crate) const PROMPT_MANAGER_WRITE_OPS: &[&str] = &["create_prompt", "update_prompt"];
pub(crate) const SKILL_MANAGER_CONTENT_WRITE_OPS: &[&str] =
    &["create_skill", "update_skill", "restore_skill_version"];
pub(crate) const SKILL_MANAGER_PRIVILEGE_OPS: &[&str] =
    &["grant_skill_to_agent", "revoke_skill_from_agent"];
pub(crate) const WORKFLOW_MANAGER_WRITE_OPS: &[&str] = &[
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
