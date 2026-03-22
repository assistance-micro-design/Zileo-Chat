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
use crate::models::streaming::{events, SubAgentOperationType, ValidationRequiredEvent};
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
use uuid::Uuid;

use super::utils::safe_truncate;

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
fn should_require_validation(
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

    /// Requests validation for a sub-agent operation.
    ///
    /// Checks ValidationSettings, then delegates to `create_and_wait_validation()`.
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
        let settings = self.load_validation_settings().await;

        if !should_require_validation(&settings, &ValidationType::SubAgent, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                operation_type = %operation_type,
                "Skipping validation based on settings (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = Uuid::new_v4().to_string();

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            operation_type = %operation_type,
            "Creating validation request for sub-agent operation"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::SubAgent,
            operation_description,
            details,
            risk_level,
        )
        .await
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
            "prompt_preview": safe_truncate(prompt, 200, true),
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
            "prompt_preview": safe_truncate(prompt, 200, true),
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
                    "prompt_preview": safe_truncate(prompt, 100, true)
                })
            })
            .collect();

        serde_json::json!({
            "task_count": tasks.len(),
            "tasks": task_list
        })
    }

    // =========================================================================
    // Local Tool Validation
    // =========================================================================

    /// Requests validation for a local tool execution.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `tool_name` - Name of the tool being executed
    /// * `operation` - Operation being performed (e.g., "add", "delete")
    /// * `arguments` - Tool arguments (JSON)
    ///
    /// # Returns
    /// * `Ok(())` - If approved or validation skipped
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_tool_validation(
        &self,
        workflow_id: &str,
        tool_name: &str,
        operation: &str,
        arguments: Value,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;
        let risk_level = RiskLevel::Low; // Local tools are generally low risk

        if !should_require_validation(&settings, &ValidationType::Tool, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                tool_name = %tool_name,
                "Skipping validation for local tool (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = uuid::Uuid::new_v4().to_string();
        let details = Self::tool_details(tool_name, operation, &arguments);
        let description = format!("Execute {} tool: {}", tool_name, operation);

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            tool_name = %tool_name,
            "Creating validation request for local tool"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::Tool,
            &description,
            details,
            risk_level,
        )
        .await
    }

    /// Creates operation details JSON for tool execution.
    pub fn tool_details(tool_name: &str, operation: &str, arguments: &Value) -> Value {
        serde_json::json!({
            "tool_name": tool_name,
            "operation": operation,
            "arguments_preview": safe_truncate(&arguments.to_string(), 200, true)
        })
    }

    // =========================================================================
    // MCP Tool Validation
    // =========================================================================

    /// Requests validation for an MCP server tool call.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `server_name` - MCP server name
    /// * `tool_name` - Tool name on the server
    /// * `arguments` - Tool arguments (JSON)
    ///
    /// # Returns
    /// * `Ok(())` - If approved or validation skipped
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_mcp_validation(
        &self,
        workflow_id: &str,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;
        let risk_level = RiskLevel::Medium; // MCP calls are medium risk (external system)

        if !should_require_validation(&settings, &ValidationType::Mcp, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                server_name = %server_name,
                tool_name = %tool_name,
                "Skipping validation for MCP tool (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = uuid::Uuid::new_v4().to_string();
        let details = Self::mcp_details(server_name, tool_name, &arguments);
        let description = format!("Call MCP server '{}': {}", server_name, tool_name);

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            server_name = %server_name,
            tool_name = %tool_name,
            "Creating validation request for MCP tool"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::Mcp,
            &description,
            details,
            risk_level,
        )
        .await
    }

    /// Creates operation details JSON for MCP tool call.
    pub fn mcp_details(server_name: &str, tool_name: &str, arguments: &Value) -> Value {
        serde_json::json!({
            "server_name": server_name,
            "tool_name": tool_name,
            "arguments_preview": safe_truncate(&arguments.to_string(), 200, true)
        })
    }

    // =========================================================================
    // File Operation Validation
    // =========================================================================

    /// Requests validation for a destructive file operation.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `operation` - File operation name (e.g., "delete", "write", "move")
    /// * `path` - File/directory path being operated on
    /// * `details` - Additional details (destination, pattern, etc.)
    ///
    /// # Returns
    /// * `Ok(())` - If approved or validation skipped
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_file_validation(
        &self,
        workflow_id: &str,
        operation: &str,
        path: &str,
        details: Value,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;
        // Delete is high risk (permanent data loss), other destructive ops are medium
        let risk_level = if operation == "delete" {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        };

        if !should_require_validation(&settings, &ValidationType::FileOp, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                operation = %operation,
                path = %path,
                risk_level = %risk_level,
                "Skipping validation for file operation (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = Uuid::new_v4().to_string();
        let full_details = Self::file_op_details(operation, path, &details);
        let description = format!("File operation '{}' on: {}", operation, path);

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            operation = %operation,
            "Creating validation request for file operation"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::FileOp,
            &description,
            full_details,
            risk_level,
        )
        .await
    }

    /// Creates operation details JSON for file operations.
    pub fn file_op_details(operation: &str, path: &str, extra: &Value) -> Value {
        serde_json::json!({
            "operation": operation,
            "path": path,
            "details": extra
        })
    }

    // =========================================================================
    // Common Validation Logic
    // =========================================================================

    /// Creates a validation request and waits for response.
    /// This is the common logic shared by all validation types.
    async fn create_and_wait_validation(
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
mod tests {
    use super::*;
    use crate::models::{RiskThresholdConfig, SelectiveValidationConfig};

    // =========================================================================
    // should_require_validation tests
    // =========================================================================

    /// Helper to create ValidationSettings with custom mode, thresholds, and selective config.
    fn make_settings(
        mode: ValidationMode,
        always_confirm_high: bool,
        auto_approve_low: bool,
        selective_config: SelectiveValidationConfig,
    ) -> ValidationSettings {
        ValidationSettings {
            mode,
            risk_thresholds: RiskThresholdConfig {
                always_confirm_high,
                auto_approve_low,
            },
            selective_config,
            ..Default::default()
        }
    }

    /// Auto mode skips validation for low/medium risk
    #[test]
    fn test_should_require_validation_auto_mode_skips() {
        let settings = make_settings(
            ValidationMode::Auto,
            false,
            false,
            SelectiveValidationConfig::default(),
        );

        assert!(!should_require_validation(
            &settings,
            &ValidationType::SubAgent,
            &RiskLevel::Low
        ));
        assert!(!should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Medium
        ));
    }

    /// Auto mode with always_confirm_high validates high and critical risk
    #[test]
    fn test_should_require_validation_auto_mode_confirms_high() {
        let settings = make_settings(
            ValidationMode::Auto,
            true,
            false,
            SelectiveValidationConfig::default(),
        );

        assert!(should_require_validation(
            &settings,
            &ValidationType::SubAgent,
            &RiskLevel::High
        ));
        assert!(should_require_validation(
            &settings,
            &ValidationType::Mcp,
            &RiskLevel::Critical
        ));
        // Medium risk is still skipped in auto mode
        assert!(!should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Medium
        ));
    }

    /// Manual mode validates everything except auto-approved low risk
    #[test]
    fn test_should_require_validation_manual_mode() {
        let settings = make_settings(
            ValidationMode::Manual,
            false,
            true,
            SelectiveValidationConfig::default(),
        );

        // Low risk is auto-approved
        assert!(!should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Low
        ));
        // Medium and high require validation
        assert!(should_require_validation(
            &settings,
            &ValidationType::SubAgent,
            &RiskLevel::Medium
        ));
        assert!(should_require_validation(
            &settings,
            &ValidationType::Mcp,
            &RiskLevel::High
        ));
    }

    /// Selective mode respects per-type configuration
    #[test]
    fn test_should_require_validation_selective_mode() {
        let settings = make_settings(
            ValidationMode::Selective,
            false,
            false,
            SelectiveValidationConfig {
                sub_agents: true,
                tools: false,
                mcp: true,
                file_ops: false,
                db_ops: false,
            },
        );

        // sub_agents enabled -> validates
        assert!(should_require_validation(
            &settings,
            &ValidationType::SubAgent,
            &RiskLevel::Medium
        ));
        // tools disabled -> skips
        assert!(!should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Medium
        ));
        // mcp enabled -> validates
        assert!(should_require_validation(
            &settings,
            &ValidationType::Mcp,
            &RiskLevel::Medium
        ));
        // file_ops disabled -> skips
        assert!(!should_require_validation(
            &settings,
            &ValidationType::FileOp,
            &RiskLevel::High
        ));
    }

    /// Selective mode with auto_approve_low skips low risk even for enabled types
    #[test]
    fn test_should_require_validation_selective_auto_approve_low() {
        let settings = make_settings(
            ValidationMode::Selective,
            false,
            true,
            SelectiveValidationConfig {
                sub_agents: true,
                tools: true,
                mcp: true,
                file_ops: true,
                db_ops: true,
            },
        );

        // Low risk auto-approved even though type is enabled
        assert!(!should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Low
        ));
        // Medium risk still validates
        assert!(should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Medium
        ));
    }

    // =========================================================================
    // Existing tests
    // =========================================================================

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
        use crate::tools::constants::sub_agent::VALIDATION_TIMEOUT_SECS;
        assert_eq!(VALIDATION_TIMEOUT_SECS, 60);
    }

    #[test]
    fn test_parallel_details_utf8_prompt() {
        // Regression test for panic at line 420
        let tasks = vec![
            ("agent_1".to_string(), "Rechercher sources fiables sur ACTUALITE pour: Mistral AI nouveautes 2025 actualites recentes lancements produits avec accents francais".to_string()),
        ];
        // This should not panic
        let details = ValidationHelper::parallel_details(&tasks);
        assert_eq!(details["task_count"], 1);
        let task = &details["tasks"].as_array().unwrap()[0];
        let preview = task["prompt_preview"].as_str().unwrap();
        assert!(preview.ends_with("..."));
    }

    // Tests for validate_trimmed_name

    #[test]
    fn test_validate_trimmed_name_valid() {
        let result = validate_trimmed_name("My Agent", "agent name", 64);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "My Agent");
    }

    #[test]
    fn test_validate_trimmed_name_trims_whitespace() {
        let result = validate_trimmed_name("  My Agent  ", "agent name", 64);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "My Agent");
    }

    #[test]
    fn test_validate_trimmed_name_empty() {
        let result = validate_trimmed_name("", "agent name", 64);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_trimmed_name_whitespace_only() {
        let result = validate_trimmed_name("   ", "agent name", 64);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot be empty"));
    }

    #[test]
    fn test_validate_trimmed_name_too_long() {
        let long = "a".repeat(65);
        let result = validate_trimmed_name(&long, "agent name", 64);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("exceeds maximum length"));
    }

    #[test]
    fn test_validate_trimmed_name_exact_max() {
        let exact = "a".repeat(64);
        let result = validate_trimmed_name(&exact, "agent name", 64);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_trimmed_name_control_chars() {
        let result = validate_trimmed_name("agent\x00name", "agent name", 64);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("control characters"));
    }

    #[test]
    fn test_validate_trimmed_name_allows_newline() {
        let result = validate_trimmed_name("agent\nname", "agent name", 64);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_trimmed_name_utf8() {
        let result = validate_trimmed_name("Mon Agent Francais", "agent name", 64);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Mon Agent Francais");
    }

    #[test]
    fn test_spawn_details_utf8_prompt() {
        // Test spawn_details with UTF-8 text (must be > 200 chars to trigger truncation)
        let prompt = "Analyser le code pour trouver les problemes de securite. Verifier les entrees utilisateur et les acces a la base de donnees. Ceci est un texte long avec des accents francais pour tester la troncature UTF-8. Nous ajoutons encore plus de texte pour depasser la limite de 200 caracteres.";
        assert!(
            prompt.chars().count() > 200,
            "Test prompt must be > 200 chars"
        );
        let details = ValidationHelper::spawn_details(
            "SecurityAgent",
            prompt,
            &["MemoryTool".to_string()],
            &["serena".to_string()],
        );
        let preview = details["prompt_preview"].as_str().unwrap();
        assert!(preview.ends_with("..."), "Preview should end with ellipsis");
    }

    // =========================================================================
    // File operation validation tests
    // =========================================================================

    #[test]
    fn test_is_destructive_file_op() {
        assert!(is_destructive_file_op("write"));
        assert!(is_destructive_file_op("replace"));
        assert!(is_destructive_file_op("delete"));
        assert!(is_destructive_file_op("move"));
        assert!(is_destructive_file_op("rename"));
        assert!(!is_destructive_file_op("list"));
        assert!(!is_destructive_file_op("read"));
        assert!(!is_destructive_file_op("create"));
        assert!(!is_destructive_file_op("search_glob"));
        assert!(!is_destructive_file_op("search_content"));
        assert!(!is_destructive_file_op("unknown"));
    }

    #[test]
    fn test_should_require_validation_selective_file_ops() {
        let selective_with_file_ops = SelectiveValidationConfig {
            sub_agents: false,
            tools: false,
            mcp: false,
            file_ops: true,
            db_ops: false,
        };
        let settings = make_settings(
            ValidationMode::Selective,
            false,
            false,
            selective_with_file_ops,
        );
        assert!(should_require_validation(
            &settings,
            &ValidationType::FileOp,
            &RiskLevel::Medium
        ));
        assert!(!should_require_validation(
            &settings,
            &ValidationType::Tool,
            &RiskLevel::Medium
        ));
    }

    #[test]
    fn test_file_op_details() {
        let extra = serde_json::json!({"destination": "/tmp/backup"});
        let details = ValidationHelper::file_op_details("move", "/home/user/file.txt", &extra);

        assert_eq!(details["operation"], "move");
        assert_eq!(details["path"], "/home/user/file.txt");
        assert_eq!(details["details"]["destination"], "/tmp/backup");
    }
}
