// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Validation helper for sub-agent operations.
//!
//! Provides human-in-the-loop validation for sub-agent tools:
//! - SpawnAgentTool
//! - DelegateTaskTool
//! - ParallelTasksTool
//!
//! # Flow
//!
//! 1. Tool calls `request_validation()` before executing operation
//! 2. Helper creates a `ValidationRequest` in the database
//! 3. Helper emits `validation_required` Tauri event
//! 4. Helper waits for approval/rejection (polling with timeout)
//! 5. Helper returns result to tool
//!
//! # Events
//!
//! - `validation_required`: Emitted when validation is needed
//! - `validation_response`: Listened for approval/rejection from frontend

use crate::db::DBClient;
use crate::models::streaming::{events, SubAgentOperationType, ValidationRequiredEvent};
use crate::models::{
    RiskLevel, ValidationMode, ValidationRequestCreate, ValidationSettings, ValidationStatus,
    ValidationType,
};
use crate::tools::ToolError;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Default timeout for validation responses (60 seconds)
const DEFAULT_VALIDATION_TIMEOUT_SECS: u64 = 60;

/// Polling interval for checking validation status (500ms)
const VALIDATION_POLL_INTERVAL_MS: u64 = 500;

/// Validation helper for sub-agent operations.
///
/// Handles the full validation flow:
/// 1. Creates validation request in database
/// 2. Emits Tauri event to frontend
/// 3. Waits for approval/rejection
/// 4. Returns result
pub struct ValidationHelper {
    /// Database client for persistence
    db: Arc<DBClient>,
    /// Tauri app handle for event emission
    app_handle: Option<AppHandle>,
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
    async fn load_validation_settings(&self) -> ValidationSettings {
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
                    if let Ok(settings) = serde_json::from_value::<ValidationSettings>(config.clone())
                    {
                        return settings;
                    }
                }
            }
        }

        ValidationSettings::default()
    }

    /// Checks if validation is required based on settings.
    fn needs_validation(
        &self,
        settings: &ValidationSettings,
        operation_type: &SubAgentOperationType,
        risk_level: &RiskLevel,
    ) -> bool {
        // Check mode first
        match settings.mode {
            ValidationMode::Auto => {
                // In auto mode, only validate if always_confirm_high is set AND risk is high
                if settings.risk_thresholds.always_confirm_high && *risk_level == RiskLevel::High {
                    info!("Auto mode but high risk requires confirmation");
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
                // Selective mode: check operation type
            }
        }

        // Selective mode: check if operation type requires validation
        let type_requires_validation = match operation_type {
            SubAgentOperationType::Spawn => settings.selective_config.sub_agents,
            SubAgentOperationType::Delegate => settings.selective_config.sub_agents,
            SubAgentOperationType::ParallelBatch => settings.selective_config.sub_agents,
        };

        if !type_requires_validation {
            info!(
                operation_type = %operation_type,
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

    /// Requests validation for a sub-agent operation.
    ///
    /// First checks ValidationSettings to determine if validation is needed.
    /// Creates a validation request, emits event to frontend, and waits for response.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `operation_type` - Type of sub-agent operation
    /// * `operation_description` - Human-readable operation description
    /// * `details` - Additional details about the operation (JSON)
    /// * `risk_level` - Risk assessment for the operation
    ///
    /// # Returns
    /// * `Ok(())` - If operation was approved (or validation was skipped)
    /// * `Err(ToolError::PermissionDenied)` - If operation was rejected
    /// * `Err(ToolError::Timeout)` - If validation timed out
    #[allow(clippy::too_many_arguments)]
    pub async fn request_validation(
        &self,
        workflow_id: &str,
        operation_type: SubAgentOperationType,
        operation_description: &str,
        details: Value,
        risk_level: RiskLevel,
    ) -> Result<(), ToolError> {
        // 0. Load validation settings and check if validation is needed
        let settings = self.load_validation_settings().await;

        if !self.needs_validation(&settings, &operation_type, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                operation_type = %operation_type,
                "Skipping validation based on settings (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        // 1. Generate validation request ID
        let validation_id = Uuid::new_v4().to_string();

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            operation_type = %operation_type,
            "Creating validation request for sub-agent operation"
        );

        // 2. Create validation request in database
        let validation_create = ValidationRequestCreate::new(
            workflow_id.to_string(),
            ValidationType::SubAgent,
            operation_description.to_string(),
            details.clone(),
            risk_level.clone(),
            ValidationStatus::Pending,
        );

        // Use db.create() which properly handles serialization for SurrealDB
        self.db
            .create("validation_request", &validation_id, validation_create)
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to create validation request in database");
                ToolError::DatabaseError(format!("Failed to create validation request: {}", e))
            })?;

        // 3. Emit validation_required event to frontend
        if let Some(ref app_handle) = self.app_handle {
            let event = ValidationRequiredEvent {
                validation_id: validation_id.clone(),
                workflow_id: workflow_id.to_string(),
                operation_type: operation_type.clone(),
                operation: operation_description.to_string(),
                risk_level: risk_level.to_string(),
                details,
            };

            if let Err(e) = app_handle.emit(events::VALIDATION_REQUIRED, &event) {
                warn!(error = %e, "Failed to emit validation_required event");
            } else {
                debug!(validation_id = %validation_id, "Emitted validation_required event");
            }
        } else {
            warn!("No app handle available, skipping event emission");
        }

        // 4. Wait for validation response (polling with timeout)
        let result = self
            .wait_for_validation(&validation_id, Duration::from_secs(DEFAULT_VALIDATION_TIMEOUT_SECS))
            .await;

        // 5. Return result
        match result {
            Ok(true) => {
                info!(validation_id = %validation_id, "Validation approved");
                Ok(())
            }
            Ok(false) => {
                info!(validation_id = %validation_id, "Validation rejected");
                Err(ToolError::PermissionDenied(format!(
                    "Sub-agent operation was rejected by user. Operation: {}",
                    operation_description
                )))
            }
            Err(e) => Err(e),
        }
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
        let poll_interval = Duration::from_millis(VALIDATION_POLL_INTERVAL_MS);
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

    /// Determines the risk level based on operation type.
    ///
    /// # Risk Level Guidelines
    /// - `Low`: Read-only operations, listing
    /// - `Medium`: Single agent spawn/delegate
    /// - `High`: Parallel execution, multiple agents
    pub fn determine_risk_level(operation_type: &SubAgentOperationType) -> RiskLevel {
        match operation_type {
            SubAgentOperationType::Spawn => RiskLevel::Medium,
            SubAgentOperationType::Delegate => RiskLevel::Medium,
            SubAgentOperationType::ParallelBatch => RiskLevel::High,
        }
    }

    /// Creates operation details JSON for spawn operation.
    pub fn spawn_details(
        name: &str,
        prompt: &str,
        tools: &[String],
        mcp_servers: &[String],
    ) -> Value {
        serde_json::json!({
            "sub_agent_name": name,
            "prompt_preview": if prompt.len() > 200 {
                format!("{}...", &prompt[..200])
            } else {
                prompt.to_string()
            },
            "prompt_length": prompt.len(),
            "tools": tools,
            "mcp_servers": mcp_servers
        })
    }

    /// Creates operation details JSON for delegate operation.
    pub fn delegate_details(target_agent_id: &str, target_agent_name: &str, prompt: &str) -> Value {
        serde_json::json!({
            "target_agent_id": target_agent_id,
            "target_agent_name": target_agent_name,
            "prompt_preview": if prompt.len() > 200 {
                format!("{}...", &prompt[..200])
            } else {
                prompt.to_string()
            },
            "prompt_length": prompt.len()
        })
    }

    /// Creates operation details JSON for parallel batch operation.
    pub fn parallel_details(tasks: &[(String, String)]) -> Value {
        let task_list: Vec<Value> = tasks
            .iter()
            .map(|(agent_id, prompt)| {
                serde_json::json!({
                    "agent_id": agent_id,
                    "prompt_preview": if prompt.len() > 100 {
                        format!("{}...", &prompt[..100])
                    } else {
                        prompt.to_string()
                    }
                })
            })
            .collect();

        serde_json::json!({
            "task_count": tasks.len(),
            "tasks": task_list
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_risk_level() {
        assert_eq!(
            ValidationHelper::determine_risk_level(&SubAgentOperationType::Spawn),
            RiskLevel::Medium
        );
        assert_eq!(
            ValidationHelper::determine_risk_level(&SubAgentOperationType::Delegate),
            RiskLevel::Medium
        );
        assert_eq!(
            ValidationHelper::determine_risk_level(&SubAgentOperationType::ParallelBatch),
            RiskLevel::High
        );
    }

    #[test]
    fn test_spawn_details() {
        let details = ValidationHelper::spawn_details(
            "TestAgent",
            "Analyze this code for bugs",
            &["MemoryTool".to_string(), "TodoTool".to_string()],
            &["serena".to_string()],
        );

        assert_eq!(details["sub_agent_name"], "TestAgent");
        assert!(details["prompt_preview"]
            .as_str()
            .unwrap()
            .contains("Analyze"));
        assert_eq!(details["tools"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_spawn_details_long_prompt() {
        let long_prompt = "A".repeat(300);
        let details = ValidationHelper::spawn_details("Agent", &long_prompt, &[], &[]);

        let preview = details["prompt_preview"].as_str().unwrap();
        assert!(preview.ends_with("..."));
        assert!(preview.len() <= 203); // 200 + "..."
    }

    #[test]
    fn test_delegate_details() {
        let details =
            ValidationHelper::delegate_details("db_agent", "Database Agent", "Analyze the schema");

        assert_eq!(details["target_agent_id"], "db_agent");
        assert_eq!(details["target_agent_name"], "Database Agent");
    }

    #[test]
    fn test_parallel_details() {
        let tasks = vec![
            ("agent_1".to_string(), "Task 1".to_string()),
            ("agent_2".to_string(), "Task 2".to_string()),
            ("agent_3".to_string(), "Task 3".to_string()),
        ];
        let details = ValidationHelper::parallel_details(&tasks);

        assert_eq!(details["task_count"], 3);
        assert_eq!(details["tasks"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_validation_timeout_default() {
        assert_eq!(DEFAULT_VALIDATION_TIMEOUT_SECS, 60);
    }
}
