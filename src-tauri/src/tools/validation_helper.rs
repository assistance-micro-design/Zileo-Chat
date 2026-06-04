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

//! Validation helper for human-in-the-loop approval.
//!
//! Provides validation for all operation types:
//! - Sub-agent: SpawnAgentTool, DelegateTaskTool, ParallelTasksTool
//! - Tool: Local tool execution (MemoryTool, TodoTool, etc.)
//! - MCP: MCP server tool calls
//!
//! # Flow
//!
//! 1. Caller invokes the appropriate `request_*_validation()` method
//! 2. Helper checks `ValidationSettings` to determine if validation is needed
//! 3. If needed, creates a `ValidationRequest` in the database
//! 4. Emits `validation_required` Tauri event to frontend
//! 5. Waits for approval/rejection (polling with timeout)
//! 6. Returns result to caller
//!
//! All validation types share a single flow via `create_and_wait_validation()`.

use crate::db::DBClient;
use crate::models::streaming::{
    events, ManagerWriteNoticeEvent, ValidationRequiredEvent, ValidationResolvedEvent,
};
use crate::models::{
    RiskLevel, TimeoutBehavior, ValidationMode, ValidationRequestCreate, ValidationSettings,
    ValidationStatus, ValidationType,
};
use crate::tools::constants::sub_agent::{VALIDATION_POLL_MS, VALIDATION_TIMEOUT_SECS};
use crate::tools::ToolError;
use serde_json::Value;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

/// Minimum allowed timeout (seconds) for a validation request.
/// Re-export of [`crate::constants::validation::VALIDATION_TIMEOUT_MIN_SECS`].
pub(crate) const VALIDATION_TIMEOUT_MIN_SECS: u64 =
    crate::constants::validation::VALIDATION_TIMEOUT_MIN_SECS;

/// Maximum allowed timeout (seconds) for a validation request.
/// Re-export of [`crate::constants::validation::VALIDATION_TIMEOUT_MAX_SECS`].
pub(crate) const VALIDATION_TIMEOUT_MAX_SECS: u64 =
    crate::constants::validation::VALIDATION_TIMEOUT_MAX_SECS;

/// Outcome of `wait_for_validation` after polling.
///
/// Carries both the resulting decision and whether it came from a timeout
/// (so callers can route audit logging through the right `DecidedBy` source).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WaitOutcome {
    pub decision: WaitDecision,
    pub via_timeout: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitDecision {
    Approved,
    Rejected,
    /// Skip is only reachable via timeout + `TimeoutBehavior::Skip`.
    Skipped,
}

impl WaitOutcome {
    fn user_decision(d: WaitDecision) -> Self {
        Self {
            decision: d,
            via_timeout: false,
        }
    }
    fn from_timeout(d: WaitDecision) -> Self {
        Self {
            decision: d,
            via_timeout: true,
        }
    }
}

/// Clamps a user-configured timeout (in seconds) into the allowed range.
///
/// `raw <= 0` falls back to [`VALIDATION_TIMEOUT_SECS`] (60s — the documented
/// default for validation responses). Otherwise the value is clamped into
/// `[VALIDATION_TIMEOUT_MIN_SECS, VALIDATION_TIMEOUT_MAX_SECS]` = `[5, 600]` seconds.
pub(crate) fn clamp_timeout_seconds(raw: i32) -> u64 {
    if raw <= 0 {
        return VALIDATION_TIMEOUT_SECS;
    }
    (raw as u64).clamp(VALIDATION_TIMEOUT_MIN_SECS, VALIDATION_TIMEOUT_MAX_SECS)
}

/// Effective result of an attempt to atomically resolve a `pending` validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolveOutcome {
    /// `true` if THIS call performed the resolution (the row was still pending).
    pub won: bool,
    /// The status stored AFTER the attempt — the first writer's decision.
    pub effective: String,
}

/// Atomically resolves a still-`pending` validation request to `new_status`.
///
/// First-writer-wins: the UPDATE is guarded by `WHERE status = 'pending'`, so a
/// decision already recorded by the competing resolution path (the user's
/// approve/reject command vs. the timeout behavior) is never clobbered. This is
/// the security invariant for the human-in-the-loop gate — a security rejection
/// must not be flipped to an approval by a late timeout, and vice-versa.
///
/// `extra_set` is an already-safe SurrealQL fragment appended to the `SET`
/// clause (a literal or a `$param`); any `$param` it references must be supplied
/// in `extra_params`. `validation_id` must be a validated UUID (it is
/// interpolated into the record id).
///
/// # Returns
/// Whether this call won the race and the effective stored status afterwards.
///
/// # Errors
/// Returns an error string if the conditional UPDATE query fails.
pub(crate) async fn resolve_validation_if_pending(
    db: &DBClient,
    validation_id: &str,
    new_status: &str,
    extra_set: Option<&str>,
    extra_params: Vec<(String, Value)>,
) -> Result<ResolveOutcome, String> {
    // Short-circuit if already resolved. The atomic `WHERE status = 'pending'`
    // guard below is what truly prevents clobbering; this read only avoids a
    // pointless write and lets us report the surviving decision.
    let before = current_validation_status(db, validation_id).await;
    if before.as_deref() != Some("pending") {
        return Ok(ResolveOutcome {
            won: false,
            effective: before.unwrap_or_else(|| "pending".to_string()),
        });
    }

    // Conditional write. `execute_with_params` is used (not a returning query) to
    // avoid SurrealDB record deserialization quirks on UPDATE. The `WHERE status = 'pending'` clause keeps it atomic: a
    // decision the competing path records in the meantime is never overwritten.
    let extra = extra_set.map(|s| format!(", {}", s)).unwrap_or_default();
    let query = format!(
        "UPDATE validation_request:`{}` SET status = $status{} WHERE status = 'pending'",
        validation_id, extra
    );
    let mut params = vec![("status".to_string(), Value::String(new_status.to_string()))];
    params.extend(extra_params);
    db.execute_with_params(&query, params)
        .await
        .map_err(|e| format!("Conditional validation resolve failed: {}", e))?;

    // Re-read the authoritative status: it reflects the first writer's decision.
    let effective = current_validation_status(db, validation_id)
        .await
        .unwrap_or_else(|| "pending".to_string());
    let won = effective == new_status;
    Ok(ResolveOutcome { won, effective })
}

/// Reads the current `status` of a `validation_request` row (`None` if missing).
async fn current_validation_status(db: &DBClient, validation_id: &str) -> Option<String> {
    let query = format!("SELECT status FROM validation_request:`{}`", validation_id);
    let rows: Vec<Value> = db.query_json(&query).await.ok()?;
    rows.first()
        .and_then(|r| r.get("status"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Validates a trimmed name with configurable field name and max length.
///
/// Centralized validation extracted from agent.rs and mcp.rs.
/// Trims whitespace, checks emptiness, length, and control characters.
///
/// # Arguments
/// * `value` - The raw name string to validate
/// * `field_name` - Human-readable field name for error messages (e.g. "Agent name")
/// * `max_len` - Maximum allowed length in bytes
///
/// # Returns
/// The trimmed name or an error message
pub fn validate_trimmed_name(
    value: &str,
    field_name: &str,
    max_len: usize,
) -> Result<String, String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }

    if trimmed.len() > max_len {
        return Err(format!(
            "{} exceeds maximum length of {} characters",
            field_name, max_len
        ));
    }

    if trimmed.chars().any(|c| c.is_control() && c != '\n') {
        return Err(format!("{} cannot contain control characters", field_name));
    }

    Ok(trimmed.to_string())
}

/// Returns `true` when `auto_approve_low` is enabled and the risk level is low.
fn is_auto_approved_low(settings: &ValidationSettings, risk_level: &RiskLevel) -> bool {
    settings.risk_thresholds.auto_approve_low && *risk_level == RiskLevel::Low
}

/// Returns `true` when the operation type is selected for validation in
/// `Selective` mode.
fn type_requires_validation(
    settings: &ValidationSettings,
    validation_type: &ValidationType,
) -> bool {
    match validation_type {
        ValidationType::SubAgent => settings.selective_config.sub_agents,
        ValidationType::Tool => settings.selective_config.tools,
        ValidationType::Mcp => settings.selective_config.mcp,
        ValidationType::FileOp => settings.selective_config.file_ops,
        ValidationType::DbOp => settings.selective_config.db_ops,
        // A *Manager write reuses the existing `tools` checkbox in Selective
        // mode (no new tri-state). A dedicated "Manager writes" checkbox
        // is an optional future refinement.
        ValidationType::ManagerWrite => settings.selective_config.tools,
    }
}

/// Checks if validation is required based on settings for any operation type.
///
/// Pure logic function (no I/O) that evaluates the validation mode, operation type,
/// and risk level to determine if human approval is needed.
///
/// # Arguments
/// * `settings` - Current validation settings
/// * `validation_type` - Type of operation (SubAgent, Tool, Mcp, etc.)
/// * `risk_level` - Risk level of the operation
///
/// # Returns
/// `true` if validation is required, `false` if the operation can proceed automatically.
pub(crate) fn should_require_validation(
    settings: &ValidationSettings,
    validation_type: &ValidationType,
    risk_level: &RiskLevel,
) -> bool {
    match settings.mode {
        ValidationMode::Auto => {
            // Auto mode: only validate when always_confirm_high covers the operation.
            if settings.risk_thresholds.always_confirm_high
                && (*risk_level == RiskLevel::High || *risk_level == RiskLevel::Critical)
            {
                info!("Auto mode but high/critical risk requires confirmation");
                return true;
            }
            info!("Auto mode: skipping validation");
            false
        }
        ValidationMode::Manual => {
            // Manual mode: always validate unless auto_approve_low covers the operation.
            if is_auto_approved_low(settings, risk_level) {
                info!("Manual mode but auto-approving low risk operation");
                return false;
            }
            true
        }
        ValidationMode::Selective => {
            if !type_requires_validation(settings, validation_type) {
                info!(
                    validation_type = %validation_type,
                    "Selective mode: operation type does not require validation"
                );
                return false;
            }
            if is_auto_approved_low(settings, risk_level) {
                info!("Auto-approving low risk operation");
                return false;
            }
            true
        }
    }
}

/// Checks if a FileManagerTool operation is destructive and requires confirmation.
///
/// Destructive operations: write, replace, delete, move, rename
/// Non-destructive: list, read, create, search_glob, search_content
pub fn is_destructive_file_op(operation: &str) -> bool {
    matches!(
        operation,
        "write" | "replace" | "delete" | "move" | "rename"
    )
}

/// Validation helper for human-in-the-loop approval.
///
/// Handles the full validation flow for sub-agent, tool, and MCP operations.
/// All validation types share a single code path via `create_and_wait_validation()`.
pub struct ValidationHelper {
    /// Database client for persistence
    pub(crate) db: Arc<DBClient>,
    /// Tauri app handle for event emission
    pub(crate) app_handle: Option<AppHandle>,
    /// Run-scoped dedup set of `(server_id, tool_name)` already audited by
    /// [`Self::record_security_refusal`]. A ValidationHelper is built once per
    /// run (never shared across runs nor cloned), so this caps detached MCP
    /// refusal audit rows at one per distinct blocked tool per run (anti-flood)
    /// while preserving the first signal for each distinct tool.
    refused_audit_keys: Mutex<HashSet<(String, String)>>,
    /// Run-scoped dedup set of `(tool_name, operation)` already audited by
    /// [`Self::record_preapproved`]. Same anti-flood rationale as
    /// `refused_audit_keys`: a run that repeats the same pre-approved write/op
    /// records one row + one toast for that pair (the per-run write cap bounds
    /// total volume).
    preapproved_audit_keys: Mutex<HashSet<(String, String)>>,
}

impl ValidationHelper {
    /// Creates a new ValidationHelper.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `app_handle` - Optional Tauri app handle for event emission
    pub fn new(db: Arc<DBClient>, app_handle: Option<AppHandle>) -> Self {
        Self {
            db,
            app_handle,
            refused_audit_keys: Mutex::new(HashSet::new()),
            preapproved_audit_keys: Mutex::new(HashSet::new()),
        }
    }

    /// Loads validation settings from database.
    /// Returns default settings if not configured.
    pub(crate) async fn load_validation_settings(&self) -> ValidationSettings {
        let query = "SELECT config FROM settings:`settings:validation`";
        let results: Vec<Value> = match self.db.query(query).await {
            Ok(r) => r,
            Err(e) => {
                debug!(error = %e, "No validation settings found, using defaults");
                return ValidationSettings::default();
            }
        };

        if let Some(first) = results.first() {
            if let Some(config) = first.get("config") {
                if !config.is_null() {
                    if let Ok(settings) =
                        serde_json::from_value::<ValidationSettings>(config.clone())
                    {
                        return settings;
                    }
                }
            }
        }

        ValidationSettings::default()
    }

    /// Waits for validation response by polling the database.
    ///
    /// On timeout, applies the configured `timeout_behavior`:
    /// - `Reject` (default): updates the row to `rejected` and returns `WaitOutcome::Rejected`.
    /// - `Approve`: updates the row to `approved` (decided_by = `timeout`)
    ///   and returns `WaitOutcome::Approved`.
    /// - `Skip`: leaves the row pending and returns `WaitOutcome::Skipped`
    ///   so the agent can proceed without blocking.
    ///
    /// # Arguments
    /// * `validation_id` - Validation request ID to check
    /// * `timeout` - Maximum time to wait for response
    /// * `timeout_behavior` - Behavior to apply when the wait expires
    ///
    /// # Errors
    /// Returns [`ToolError::DatabaseError`] if the polling query fails.
    async fn wait_for_validation(
        &self,
        validation_id: &str,
        timeout: Duration,
        timeout_behavior: TimeoutBehavior,
    ) -> Result<WaitOutcome, ToolError> {
        let poll_interval = Duration::from_millis(VALIDATION_POLL_MS);
        let start_time = std::time::Instant::now();

        loop {
            // Check if timeout exceeded
            if start_time.elapsed() >= timeout {
                return Ok(self
                    .apply_timeout_behavior(validation_id, timeout, &timeout_behavior)
                    .await);
            }

            // Query validation status
            let query = format!("SELECT status FROM validation_request:`{}`", validation_id);

            let result: Vec<Value> = self.db.query(&query).await.map_err(|e| {
                ToolError::DatabaseError(format!("Failed to query validation status: {}", e))
            })?;

            if let Some(first) = result.first() {
                let status = first["status"].as_str().unwrap_or("pending");

                match status {
                    "approved" => {
                        return Ok(WaitOutcome::user_decision(WaitDecision::Approved));
                    }
                    "rejected" => {
                        return Ok(WaitOutcome::user_decision(WaitDecision::Rejected));
                    }
                    "pending" => {
                        // Continue waiting
                        debug!(
                            validation_id = %validation_id,
                            elapsed_secs = start_time.elapsed().as_secs(),
                            "Waiting for validation response..."
                        );
                    }
                    _ => {
                        warn!(
                            validation_id = %validation_id,
                            status = %status,
                            "Unexpected validation status"
                        );
                    }
                }
            }

            // Sleep before next poll
            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Applies the configured `timeout_behavior` once the wait period expires.
    ///
    /// Updates the validation row to reflect the auto-decision (Reject/Approve)
    /// or leaves it pending (Skip), then returns the resulting outcome.
    /// Database write failures are logged but never propagated, since the
    /// caller has already given up waiting and must move on.
    async fn apply_timeout_behavior(
        &self,
        validation_id: &str,
        timeout: Duration,
        timeout_behavior: &TimeoutBehavior,
    ) -> WaitOutcome {
        match timeout_behavior {
            TimeoutBehavior::Reject => {
                let reason = format!(
                    "Validation timed out after {} seconds (auto-reject)",
                    timeout.as_secs()
                );
                let resolved = resolve_validation_if_pending(
                    &self.db,
                    validation_id,
                    "rejected",
                    Some("details.rejection_reason = $reason"),
                    vec![("reason".to_string(), Value::String(reason))],
                )
                .await;
                self.finalize_timeout_resolution(
                    validation_id,
                    resolved,
                    WaitDecision::Rejected,
                    timeout,
                )
            }
            TimeoutBehavior::Approve => {
                let resolved = resolve_validation_if_pending(
                    &self.db,
                    validation_id,
                    "approved",
                    Some("details.timeout_decision = 'approved'"),
                    vec![],
                )
                .await;
                self.finalize_timeout_resolution(
                    validation_id,
                    resolved,
                    WaitDecision::Approved,
                    timeout,
                )
            }
            TimeoutBehavior::Skip => {
                // Skip never mutates the row: the request stays pending and the
                // agent proceeds without a recorded decision. No race to lose.
                info!(
                    validation_id,
                    elapsed_secs = timeout.as_secs(),
                    "Validation timed out -> skip (agent proceeds without decision)"
                );
                self.emit_resolved_event(validation_id, WaitDecision::Skipped, "timeout");
                WaitOutcome::from_timeout(WaitDecision::Skipped)
            }
        }
    }

    /// Maps the result of a conditional timeout resolution to a [`WaitOutcome`].
    ///
    /// If the timeout won (the row was still pending) the configured behavior
    /// applies and the decision is flagged `via_timeout` so it is audited as a
    /// timeout. If the user decided first in the final poll window, that decision
    /// is preserved and surfaced as a USER decision (`via_timeout = false`), so it
    /// is not double-audited and the security gate honors the human's choice.
    fn finalize_timeout_resolution(
        &self,
        validation_id: &str,
        resolved: Result<ResolveOutcome, String>,
        intended: WaitDecision,
        timeout: Duration,
    ) -> WaitOutcome {
        match resolved {
            Ok(outcome) if outcome.won => {
                info!(
                    validation_id,
                    elapsed_secs = timeout.as_secs(),
                    decision = ?intended,
                    "Validation timed out -> auto-decision applied"
                );
                self.emit_resolved_event(validation_id, intended, "timeout");
                WaitOutcome::from_timeout(intended)
            }
            Ok(outcome) => match outcome.effective.as_str() {
                "approved" => {
                    info!(
                        validation_id,
                        "Timeout lost the race to a user approval; preserving it"
                    );
                    WaitOutcome::user_decision(WaitDecision::Approved)
                }
                "rejected" => {
                    info!(
                        validation_id,
                        "Timeout lost the race to a user rejection; preserving it"
                    );
                    WaitOutcome::user_decision(WaitDecision::Rejected)
                }
                other => {
                    // Degenerate (row missing / unexpected status): apply the
                    // configured behavior so the agent is never left hanging.
                    warn!(
                        validation_id,
                        status = %other,
                        "Unexpected status after lost timeout race; applying behavior"
                    );
                    self.emit_resolved_event(validation_id, intended, "timeout");
                    WaitOutcome::from_timeout(intended)
                }
            },
            Err(e) => {
                // Conditional resolve failed (DB error). Stay best-effort: fall
                // back to the configured behavior so the agent is never stuck.
                warn!(
                    error = %e,
                    validation_id,
                    "Conditional timeout resolve failed; applying behavior best-effort"
                );
                self.emit_resolved_event(validation_id, intended, "timeout");
                WaitOutcome::from_timeout(intended)
            }
        }
    }

    /// Emits a `validation_resolved` Tauri event so the frontend can close the
    /// validation modal once the backend resolves the request itself (timeout).
    /// User-driven approve/reject already updates the frontend store via the
    /// Tauri command response, so we only emit here for server-side resolutions.
    fn emit_resolved_event(&self, validation_id: &str, decision: WaitDecision, source: &str) {
        let Some(ref app_handle) = self.app_handle else {
            return;
        };

        let resolution = match decision {
            WaitDecision::Approved => "approved",
            WaitDecision::Rejected => "rejected",
            WaitDecision::Skipped => "skipped",
        };

        let event = ValidationResolvedEvent {
            validation_id: validation_id.to_string(),
            resolution: resolution.to_string(),
            source: source.to_string(),
        };

        if let Err(e) = app_handle.emit(events::VALIDATION_RESOLVED, &event) {
            warn!(error = %e, validation_id, "Failed to emit validation_resolved event");
        } else {
            debug!(
                validation_id,
                resolution, source, "Emitted validation_resolved event"
            );
        }
    }

    /// Creates a validation request and waits for response.
    /// This is the common logic shared by all validation types.
    ///
    /// Loads the user's `ValidationSettings` to honor `timeout_seconds` (clamped
    /// to `[5, 600]`) and `timeout_behavior`. Falls back to the hardcoded
    /// `VALIDATION_TIMEOUT_SECS` (60s) and `Reject` if settings cannot be loaded.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn create_and_wait_validation(
        &self,
        validation_id: &str,
        workflow_id: &str,
        validation_type: ValidationType,
        description: &str,
        details: Value,
        risk_level: RiskLevel,
        is_detached: bool,
    ) -> Result<(), ToolError> {
        // Court-circuit: an UNATTENDED (detached) run has no human to answer
        // the modal, and the boot catch-up loop (`catchup_unanalyzed_review_cards`)
        // is sequential — emitting a modal and polling would block each card for
        // up to the full `timeout_seconds` (up to 600s × N cards = DoS-boot).
        // Apply the detached policy DIRECTLY: refuse + audit,
        // never create the request, never emit the event, never poll.
        if is_detached {
            warn!(
                validation_id = %validation_id,
                workflow_id = %workflow_id,
                validation_type = %validation_type,
                "Validation required in an unattended (detached) run — refusing without a modal"
            );
            self.record_detached_validation_refusal(
                workflow_id,
                &validation_type,
                description,
                &risk_level,
            )
            .await;
            return Err(ToolError::PermissionDenied(format!(
                "Operation requires validation but the run is unattended (detached): {}",
                description
            )));
        }

        // Create validation request in database
        let validation_create = ValidationRequestCreate::new(
            workflow_id.to_string(),
            validation_type.clone(),
            description.to_string(),
            details.clone(),
            risk_level.clone(),
            ValidationStatus::Pending,
        );

        self.db
            .create("validation_request", validation_id, validation_create)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to create validation request in database");
                ToolError::DatabaseError(format!("Failed to create validation request: {}", e))
            })?;

        // Emit validation_required event to frontend
        if let Some(ref app_handle) = self.app_handle {
            let event = ValidationRequiredEvent {
                validation_id: validation_id.to_string(),
                workflow_id: workflow_id.to_string(),
                validation_type: validation_type.to_string(),
                operation: description.to_string(),
                risk_level: risk_level.to_string(),
                details: details.clone(),
            };

            if let Err(e) = app_handle.emit(events::VALIDATION_REQUIRED, &event) {
                warn!(error = %e, "Failed to emit validation_required event");
            } else {
                debug!(validation_id = %validation_id, "Emitted validation_required event");
            }
        } else {
            warn!("No app handle available, skipping event emission");
        }

        // Resolve timeout + behavior from user settings (with safe fallbacks).
        let settings = self.load_validation_settings().await;
        let timeout_secs = clamp_timeout_seconds(settings.timeout_seconds);
        let timeout_behavior = settings.timeout_behavior.clone();

        let outcome = self
            .wait_for_validation(
                validation_id,
                Duration::from_secs(timeout_secs),
                timeout_behavior,
            )
            .await?;

        // append a timeout-driven audit entry.
        // User-driven approve/reject is audited from the Tauri command path
        // (commands/validation.rs) so we don't double-write here.
        if outcome.via_timeout {
            self.write_timeout_audit(
                &settings,
                validation_id,
                workflow_id,
                &validation_type,
                description,
                &risk_level,
                outcome.decision,
            )
            .await;
        }

        match outcome.decision {
            WaitDecision::Approved => {
                info!(validation_id = %validation_id, "Validation approved");
                Ok(())
            }
            WaitDecision::Rejected => {
                info!(validation_id = %validation_id, "Validation rejected");
                Err(ToolError::PermissionDenied(format!(
                    "Operation was rejected by user: {}",
                    description
                )))
            }
            WaitDecision::Skipped => {
                info!(
                    validation_id = %validation_id,
                    "Validation skipped on timeout (agent proceeds)"
                );
                Ok(())
            }
        }
    }

    /// Best-effort audit write for a timeout-driven decision. Never propagates errors.
    #[allow(clippy::too_many_arguments)]
    async fn write_timeout_audit(
        &self,
        settings: &ValidationSettings,
        validation_id: &str,
        workflow_id: &str,
        validation_type: &ValidationType,
        description: &str,
        risk_level: &RiskLevel,
        decision: WaitDecision,
    ) {
        use crate::commands::validation_audit::{write_audit_entry, AuditEntryDraft};
        use crate::models::{AuditDecision, DecidedBy};

        let audit_decision = match decision {
            WaitDecision::Approved => AuditDecision::Approved,
            WaitDecision::Rejected => AuditDecision::Rejected,
            WaitDecision::Skipped => AuditDecision::Skipped,
        };
        let draft = AuditEntryDraft {
            validation_id: validation_id.to_string(),
            tool_name: validation_type.to_string(),
            decision: audit_decision,
            decided_by: DecidedBy::Timeout,
            risk_level: risk_level.clone(),
            workflow_id: Some(workflow_id.to_string()),
            agent_id: None,
            prompt_preview: Some(description.to_string()),
            metadata: Some(serde_json::json!({
                "source": "timeout",
                "behavior": settings.timeout_behavior.to_string(),
                "timeout_seconds": settings.timeout_seconds,
            })),
        };
        write_audit_entry(&self.db, settings, draft).await;
    }

    /// Records a security-policy refusal of an MCP tool in a DETACHED run into
    /// the audit log, so refusals surface in the
    /// `Settings > Audit Log` page. Best-effort: never propagates errors.
    ///
    /// Written UNCONDITIONALLY (bypasses `audit.enable_logging`): a refusal with
    /// no human in the loop must stay traceable even when audit logging is off,
    /// otherwise an attacker-relevant refusal could be silently hidden. A FRESH
    /// uuid is used as `validation_id` because the audit index is UNIQUE — a
    /// constant would collide on the second refusal and the row would be lost.
    ///
    /// # Arguments
    /// * `tool_name` - Full tool name (`mcp__server__tool` form), as indexed.
    /// * `server_id` - Immutable MCP server id, or `None` if unresolved.
    /// * `reason` - Short, secret-free explanation of the refusal.
    /// * `is_delegated` - Whether the refused run was a delegated sub-agent;
    ///   distinguishes the confused-deputy refusal (armed for the agent but
    ///   not flagged `allow_in_delegated_runs`) from a plain unarmed refusal.
    /// * `workflow_id` - Workflow context; empty is recorded as no workflow.
    pub(crate) async fn record_security_refusal(
        &self,
        tool_name: &str,
        server_id: Option<&str>,
        reason: &str,
        is_delegated: bool,
        workflow_id: &str,
    ) {
        use crate::commands::validation_audit::{write_audit_entry_unconditional, AuditEntryDraft};
        use crate::models::{AuditDecision, DecidedBy};

        let server_key = server_id.unwrap_or("<unknown>").to_string();

        // Anti-flood (run-scoped dedup): record at most one row per
        // (server_id, tool_name) per run. A detached run that hammers the same
        // blocked tool across many iterations must not inflate the audit table;
        // the first refusal of a distinct (server, tool) is kept, identical
        // ones are skipped. The lock is released before the async write (never
        // held across an `.await`); poison-tolerant to stay best-effort.
        {
            let mut seen = self
                .refused_audit_keys
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !seen.insert((server_key.clone(), tool_name.to_string())) {
                return;
            }
        }

        let draft = AuditEntryDraft {
            validation_id: uuid::Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            decision: AuditDecision::Blocked,
            decided_by: DecidedBy::Policy,
            risk_level: RiskLevel::High,
            workflow_id: (!workflow_id.is_empty()).then(|| workflow_id.to_string()),
            // FunctionCallContext carries no agent_id at the gate; workflow_id
            // is enough for traceability (mirrors write_timeout_audit).
            agent_id: None,
            prompt_preview: None,
            metadata: Some(serde_json::json!({
                "source": "mcp_detached_gate",
                "reason": reason,
                "server_id": server_key,
                "delegated": is_delegated,
            })),
        };
        write_audit_entry_unconditional(&self.db, draft).await;
    }

    /// Records the court-circuit refusal of a validation-required operation
    /// in a DETACHED run into the audit log. Best-effort, unconditional (a
    /// no-human refusal must stay traceable even when audit logging is off).
    ///
    /// `validation_type` becomes the audited `tool_name` so the row is
    /// filterable; `description` is truncated into the preview. Mirrors
    /// [`Self::record_security_refusal`] (decision `Blocked`, source `Policy`).
    async fn record_detached_validation_refusal(
        &self,
        workflow_id: &str,
        validation_type: &ValidationType,
        description: &str,
        risk_level: &RiskLevel,
    ) {
        use crate::commands::validation_audit::{write_audit_entry_unconditional, AuditEntryDraft};
        use crate::models::{AuditDecision, DecidedBy};

        let draft = AuditEntryDraft {
            validation_id: uuid::Uuid::new_v4().to_string(),
            tool_name: validation_type.to_string(),
            decision: AuditDecision::Blocked,
            decided_by: DecidedBy::Policy,
            risk_level: risk_level.clone(),
            workflow_id: (!workflow_id.is_empty()).then(|| workflow_id.to_string()),
            agent_id: None,
            prompt_preview: Some(crate::tools::utils::safe_truncate(description, 200, true)),
            metadata: Some(serde_json::json!({
                "source": "detached_validation_court_circuit",
            })),
        };
        write_audit_entry_unconditional(&self.db, draft).await;
    }

    /// Records a refused *Manager write (scope / volume / detached-validation /
    /// no-helper) into the audit log so a no-review block is visible, not just
    /// traced. Best-effort, unconditional (decision `Blocked`, source `Policy`),
    /// with run-scoped dedup on `(tool_name, operation)` via `refused_audit_keys`
    /// (shared anti-flood set — MCP keys never collide with `(tool, op)` pairs).
    pub(crate) async fn record_manager_refusal(
        &self,
        tool_name: &str,
        operation: &str,
        reason: &str,
        risk_level: &RiskLevel,
        workflow_id: &str,
    ) {
        use crate::commands::validation_audit::{write_audit_entry_unconditional, AuditEntryDraft};
        use crate::models::{AuditDecision, DecidedBy};

        {
            let mut seen = self
                .refused_audit_keys
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !seen.insert((tool_name.to_string(), operation.to_string())) {
                return;
            }
        }

        let draft = AuditEntryDraft {
            validation_id: uuid::Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            decision: AuditDecision::Blocked,
            decided_by: DecidedBy::Policy,
            risk_level: risk_level.clone(),
            workflow_id: (!workflow_id.is_empty()).then(|| workflow_id.to_string()),
            agent_id: None,
            prompt_preview: Some(operation.to_string()),
            metadata: Some(serde_json::json!({
                "source": "manager_write_gate",
                "reason": reason,
                "operation": operation,
            })),
        };
        write_audit_entry_unconditional(&self.db, draft).await;
    }

    /// Records a *Manager / armed-MCP operation that EXECUTED without a human
    /// review because the active validation mode is permissive for its risk
    /// (Auto + `always_confirm_high` OFF, etc.). Audited as `Approved` /
    /// `DecidedBy::PreApproved`.
    ///
    /// Written UNCONDITIONALLY (bypasses `audit.enable_logging`): the persisted
    /// `PreApproved` row is the REAL safety net for self-improvement writes (the
    /// toast is opportunistic and lost if the app is closed), so it
    /// must survive even when audit logging is off. Run-scoped dedup on
    /// `(tool_name, operation)` (anti-flood, mirrors `record_security_refusal`).
    ///
    /// For a High or Critical write it ALSO emits a non-blocking
    /// `manager_write_notice` event so the frontend can toast it live. Best-effort
    /// throughout: never propagates an error, never blocks the write.
    ///
    /// `target` is a per-RESOURCE discriminant (prompt/skill id or name, server
    /// for MCP) folded into the dedup key so the audit stays accurate: rewriting
    /// FIVE distinct prompts in one run leaves FIVE rows (one per resource), while
    /// a true retry of the SAME op on the SAME resource is still deduped
    /// (anti-flood). Empty `target` degrades to per-`(tool, op)` dedup.
    pub(crate) async fn record_preapproved(
        &self,
        tool_name: &str,
        operation: &str,
        target: &str,
        risk_level: &RiskLevel,
        workflow_id: &str,
    ) {
        use crate::commands::validation_audit::{write_audit_entry_unconditional, AuditEntryDraft};
        use crate::models::{AuditDecision, DecidedBy};

        // Run-scoped dedup keyed by (tool, op|resource): one audit row + one
        // toast per distinct resource-operation, true retries collapsed.
        {
            let mut seen = self
                .preapproved_audit_keys
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if !seen.insert((tool_name.to_string(), format!("{}|{}", operation, target))) {
                return;
            }
        }

        let draft = AuditEntryDraft {
            validation_id: uuid::Uuid::new_v4().to_string(),
            tool_name: tool_name.to_string(),
            decision: AuditDecision::Approved,
            decided_by: DecidedBy::PreApproved,
            risk_level: risk_level.clone(),
            workflow_id: (!workflow_id.is_empty()).then(|| workflow_id.to_string()),
            agent_id: None,
            prompt_preview: Some(operation.to_string()),
            metadata: Some(serde_json::json!({
                "source": "manager_write_preapproved",
                "operation": operation,
            })),
        };
        write_audit_entry_unconditional(&self.db, draft).await;

        // Opportunistic live toast for High/Critical writes only.
        if matches!(risk_level, RiskLevel::High | RiskLevel::Critical) {
            if let Some(ref app_handle) = self.app_handle {
                let event = ManagerWriteNoticeEvent {
                    workflow_id: workflow_id.to_string(),
                    tool_name: tool_name.to_string(),
                    operation: operation.to_string(),
                    risk_level: risk_level.to_string(),
                };
                if let Err(e) = app_handle.emit(events::MANAGER_WRITE_NOTICE, &event) {
                    warn!(error = %e, "Failed to emit manager_write_notice event");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "validation_helper_tests.rs"]
mod tests;
