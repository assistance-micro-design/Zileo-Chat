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
use crate::models::streaming::{events, ValidationRequiredEvent};
use crate::models::{
    RiskLevel, ValidationMode, ValidationRequestCreate, ValidationSettings, ValidationStatus,
    ValidationType,
};
use crate::tools::constants::sub_agent::{VALIDATION_POLL_MS, VALIDATION_TIMEOUT_SECS};
use crate::tools::ToolError;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};

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
    // Check mode first
    match settings.mode {
        ValidationMode::Auto => {
            // In auto mode, only validate if always_confirm_high is set AND risk is high
            if settings.risk_thresholds.always_confirm_high
                && (*risk_level == RiskLevel::High || *risk_level == RiskLevel::Critical)
            {
                info!("Auto mode but high/critical risk requires confirmation");
                return true;
            }
            info!("Auto mode: skipping validation");
            return false;
        }
        ValidationMode::Manual => {
            // Manual mode: always validate unless auto_approve_low is set AND risk is low
            if settings.risk_thresholds.auto_approve_low && *risk_level == RiskLevel::Low {
                info!("Manual mode but auto-approving low risk operation");
                return false;
            }
            return true;
        }
        ValidationMode::Selective => {
            // Selective mode: check operation type below
        }
    }

    // Selective mode: check if operation type requires validation
    let type_requires_validation = match validation_type {
        ValidationType::SubAgent => settings.selective_config.sub_agents,
        ValidationType::Tool => settings.selective_config.tools,
        ValidationType::Mcp => settings.selective_config.mcp,
        ValidationType::FileOp => settings.selective_config.file_ops,
        ValidationType::DbOp => settings.selective_config.db_ops,
    };

    if !type_requires_validation {
        info!(
            validation_type = %validation_type,
            "Selective mode: operation type does not require validation"
        );
        return false;
    }

    // Check risk thresholds
    if settings.risk_thresholds.auto_approve_low && *risk_level == RiskLevel::Low {
        info!("Auto-approving low risk operation");
        return false;
    }

    true
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
    /// # Arguments
    /// * `validation_id` - Validation request ID to check
    /// * `timeout` - Maximum time to wait for response
    ///
    /// # Returns
    /// * `Ok(true)` - If approved
    /// * `Ok(false)` - If rejected
    /// * `Err(ToolError::Timeout)` - If timed out
    async fn wait_for_validation(
        &self,
        validation_id: &str,
        timeout: Duration,
    ) -> Result<bool, ToolError> {
        let poll_interval = Duration::from_millis(VALIDATION_POLL_MS);
        let start_time = std::time::Instant::now();

        loop {
            // Check if timeout exceeded
            if start_time.elapsed() >= timeout {
                // Update validation status to rejected (timeout)
                let update_query = format!(
                    "UPDATE validation_request:`{}` SET status = 'rejected', \
                     details.rejection_reason = 'Validation timed out'",
                    validation_id
                );
                let _ = self.db.execute(&update_query).await;

                return Err(ToolError::Timeout(format!(
                    "Validation request '{}' timed out after {} seconds. \
                     User did not respond in time.",
                    validation_id,
                    timeout.as_secs()
                )));
            }

            // Query validation status
            let query = format!("SELECT status FROM validation_request:`{}`", validation_id);

            let result: Vec<Value> = self.db.query(&query).await.map_err(|e| {
                ToolError::DatabaseError(format!("Failed to query validation status: {}", e))
            })?;

            if let Some(first) = result.first() {
                let status = first["status"].as_str().unwrap_or("pending");

                match status {
                    "approved" => return Ok(true),
                    "rejected" => return Ok(false),
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

    /// Creates a validation request and waits for response.
    /// This is the common logic shared by all validation types.
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

        // Wait for validation response
        let result = self
            .wait_for_validation(validation_id, Duration::from_secs(VALIDATION_TIMEOUT_SECS))
            .await;

        match result {
            Ok(true) => {
                info!(validation_id = %validation_id, "Validation approved");
                Ok(())
            }
            Ok(false) => {
                info!(validation_id = %validation_id, "Validation rejected");
                Err(ToolError::PermissionDenied(format!(
                    "Operation was rejected by user: {}",
                    description
                )))
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
#[path = "validation_helper_tests.rs"]
mod tests;
