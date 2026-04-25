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
use crate::models::streaming::{events, ValidationRequiredEvent, ValidationResolvedEvent};
use crate::models::{
    RiskLevel, TimeoutBehavior, ValidationMode, ValidationRequestCreate, ValidationSettings,
    ValidationStatus, ValidationType,
};
use crate::tools::constants::sub_agent::{VALIDATION_POLL_MS, VALIDATION_TIMEOUT_SECS};
use crate::tools::ToolError;
use serde_json::Value;
use std::sync::Arc;
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
}

impl ValidationHelper {
    /// Creates a new ValidationHelper.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `app_handle` - Optional Tauri app handle for event emission
    pub fn new(db: Arc<DBClient>, app_handle: Option<AppHandle>) -> Self {
        Self { db, app_handle }
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
        let outcome = match timeout_behavior {
            TimeoutBehavior::Reject => {
                let query = format!(
                    "UPDATE validation_request:`{}` SET status = 'rejected', \
                     details.rejection_reason = $reason",
                    validation_id
                );
                let reason = format!(
                    "Validation timed out after {} seconds (auto-reject)",
                    timeout.as_secs()
                );
                if let Err(e) = self
                    .db
                    .execute_with_params(
                        &query,
                        vec![("reason".to_string(), Value::String(reason))],
                    )
                    .await
                {
                    warn!(error = %e, validation_id, "Failed to mark validation rejected on timeout");
                }
                info!(
                    validation_id,
                    elapsed_secs = timeout.as_secs(),
                    "Validation timed out -> auto-reject"
                );
                WaitOutcome::from_timeout(WaitDecision::Rejected)
            }
            TimeoutBehavior::Approve => {
                let query = format!(
                    "UPDATE validation_request:`{}` SET status = 'approved', \
                     details.timeout_decision = 'approved'",
                    validation_id
                );
                if let Err(e) = self.db.execute(&query).await {
                    warn!(error = %e, validation_id, "Failed to mark validation approved on timeout");
                }
                info!(
                    validation_id,
                    elapsed_secs = timeout.as_secs(),
                    "Validation timed out -> auto-approve"
                );
                WaitOutcome::from_timeout(WaitDecision::Approved)
            }
            TimeoutBehavior::Skip => {
                info!(
                    validation_id,
                    elapsed_secs = timeout.as_secs(),
                    "Validation timed out -> skip (agent proceeds without decision)"
                );
                WaitOutcome::from_timeout(WaitDecision::Skipped)
            }
        };

        self.emit_resolved_event(validation_id, outcome.decision, "timeout");

        outcome
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
    pub(crate) async fn create_and_wait_validation(
        &self,
        validation_id: &str,
        workflow_id: &str,
        validation_type: ValidationType,
        description: &str,
        details: Value,
        risk_level: RiskLevel,
    ) -> Result<(), ToolError> {
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
}

#[cfg(test)]
#[path = "validation_helper_tests.rs"]
mod tests;
