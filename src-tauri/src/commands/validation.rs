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

//! Validation commands for human-in-the-loop operations.
//!
//! Provides Tauri commands for managing validation requests that require
//! human approval before execution (tools, sub-agents, MCP calls, etc.).

use crate::{
    commands::validation_audit::{write_audit_entry, AuditEntryDraft},
    constants::{
        audit::{RETENTION_MAX_DAYS, RETENTION_MIN_DAYS},
        validation::{VALIDATION_TIMEOUT_MAX_SECS, VALIDATION_TIMEOUT_MIN_SECS},
    },
    models::{
        AuditConfig, AuditDecision, DecidedBy, PartialAuditConfig, PartialRiskThresholds,
        PartialSelectiveConfig, RiskLevel, RiskThresholdConfig, SelectiveValidationConfig,
        UpdateValidationSettingsRequest, ValidationRequest, ValidationRequestCreate,
        ValidationSettings, ValidationStatus, ValidationType,
    },
    security::{serialize_for_query, validate_uuid_field, Validator},
    tools::registry::TOOL_REGISTRY,
    AppState,
};
use chrono::Utc;
use serde::Serialize;
use tauri::State;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Creates a new validation request for human-in-the-loop approval.
///
/// # Arguments
/// * `workflow_id` - Associated workflow ID
/// * `validation_type` - Type of operation requiring validation
/// * `operation` - Description of the operation
/// * `details` - Additional details about the operation
/// * `risk_level` - Risk assessment of the operation
///
/// # Returns
/// The created validation request
#[tauri::command]
#[instrument(
    name = "create_validation_request",
    skip(state, details),
    fields(workflow_id = %workflow_id, validation_type = ?validation_type, risk_level = ?risk_level)
)]
pub async fn create_validation_request(
    workflow_id: String,
    validation_type: ValidationType,
    operation: String,
    details: serde_json::Value,
    risk_level: RiskLevel,
    state: State<'_, AppState>,
) -> Result<ValidationRequest, String> {
    info!("Creating validation request");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Validate operation description
    let validated_operation = Validator::validate_message(&operation).map_err(|e| {
        warn!(error = %e, "Invalid operation description");
        format!("Invalid operation description: {}", e)
    })?;

    let request_id = Uuid::new_v4().to_string();

    // Use ValidationRequestCreate to avoid passing datetime field
    // The database will set created_at via DEFAULT time::now()
    // ID is passed separately using table:id format
    let request_create = ValidationRequestCreate::new(
        validated_workflow_id.clone(),
        validation_type.clone(),
        validated_operation.clone(),
        details.clone(),
        risk_level.clone(),
        ValidationStatus::Pending,
    );

    let id = state
        .db
        .create("validation_request", &request_id, request_create)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create validation request");
            format!("Failed to create validation request: {}", e)
        })?;

    // Build response object with current timestamp for immediate use
    let request = ValidationRequest {
        id: request_id,
        workflow_id: validated_workflow_id,
        validation_type,
        operation: validated_operation,
        details,
        risk_level,
        status: ValidationStatus::Pending,
        created_at: Utc::now(),
    };

    info!(validation_id = %id, "Validation request created successfully");
    Ok(request)
}

/// Lists all pending validation requests.
///
/// # Returns
/// Vector of pending validation requests sorted by creation time (newest first)
#[tauri::command]
#[instrument(name = "list_pending_validations", skip(state))]
pub async fn list_pending_validations(
    state: State<'_, AppState>,
) -> Result<Vec<ValidationRequest>, String> {
    info!("Loading pending validations");

    let validations: Vec<ValidationRequest> = state
        .db
        .query(
            "SELECT meta::id(id) AS id, workflow_id, validation_type, details, status, \
             risk_level, created_at, updated_at \
             FROM validation_request WHERE status = 'pending' ORDER BY created_at DESC",
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load pending validations");
            format!("Failed to load pending validations: {}", e)
        })?;

    info!(count = validations.len(), "Pending validations loaded");
    Ok(validations)
}

/// Lists all validation requests for a specific workflow.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to filter by
///
/// # Returns
/// Vector of validation requests for the workflow
#[tauri::command]
#[instrument(name = "list_workflow_validations", skip(state), fields(workflow_id = %workflow_id))]
pub async fn list_workflow_validations(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ValidationRequest>, String> {
    info!("Loading workflow validations");

    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    let validations: Vec<ValidationRequest> = state
        .db
        .query_json_with_params(
            "SELECT meta::id(id) AS id, workflow_id, validation_type, details, status, \
             risk_level, created_at, updated_at \
             FROM validation_request WHERE workflow_id = $wf_id ORDER BY created_at DESC",
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflow validations");
            format!("Failed to load workflow validations: {}", e)
        })?
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    info!(count = validations.len(), "Workflow validations loaded");
    Ok(validations)
}

/// Approves a validation request.
///
/// # Arguments
/// * `validation_id` - The validation request ID to approve
#[tauri::command]
#[instrument(name = "approve_validation", skip(state), fields(validation_id = %validation_id))]
pub async fn approve_validation(
    validation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Approving validation request");

    let validated_id = validate_uuid_field(&validation_id, "validation_id")?;

    // Update status to approved using bind param for status value
    state
        .db
        .execute_with_params(
            &format!(
                "UPDATE validation_request:`{}` SET status = $status",
                validated_id
            ),
            vec![("status".to_string(), serde_json::json!("approved"))],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to approve validation");
            format!("Failed to approve validation: {}", e)
        })?;

    // append to audit log (best-effort, never blocks user flow).
    if let Some(draft) = build_audit_draft(
        &state,
        &validated_id,
        AuditDecision::Approved,
        DecidedBy::User,
        None,
    )
    .await
    {
        let settings = get_validation_settings_internal(&state)
            .await
            .unwrap_or_default();
        write_audit_entry(&state.db, &settings, draft).await;
    }

    info!("Validation request approved successfully");
    Ok(())
}

/// Rejects a validation request with a reason.
///
/// # Arguments
/// * `validation_id` - The validation request ID to reject
/// * `reason` - The reason for rejection
#[tauri::command]
#[instrument(name = "reject_validation", skip(state, reason), fields(validation_id = %validation_id))]
pub async fn reject_validation(
    validation_id: String,
    reason: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Rejecting validation request");

    let validated_id = validate_uuid_field(&validation_id, "validation_id")?;

    // Validate reason
    let validated_reason = Validator::validate_message(&reason).map_err(|e| {
        warn!(error = %e, "Invalid rejection reason");
        format!("Invalid rejection reason: {}", e)
    })?;

    // Update status to rejected and store reason using bind params
    state
        .db
        .execute_with_params(
            &format!(
                "UPDATE validation_request:`{}` SET status = $status, details.rejection_reason = $reason",
                validated_id
            ),
            vec![
                ("status".to_string(), serde_json::json!("rejected")),
                ("reason".to_string(), serde_json::json!(validated_reason)),
            ],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to reject validation");
            format!("Failed to reject validation: {}", e)
        })?;

    // append to audit log (best-effort).
    if let Some(draft) = build_audit_draft(
        &state,
        &validated_id,
        AuditDecision::Rejected,
        DecidedBy::User,
        Some(serde_json::json!({"rejection_reason": validated_reason})),
    )
    .await
    {
        let settings = get_validation_settings_internal(&state)
            .await
            .unwrap_or_default();
        write_audit_entry(&state.db, &settings, draft).await;
    }

    info!("Validation request rejected successfully");
    Ok(())
}

/// Builds an `AuditEntryDraft` from a stored `validation_request` row.
///
/// Returns `None` if the row cannot be located — audit must be best-effort.
async fn build_audit_draft(
    state: &State<'_, AppState>,
    validated_id: &str,
    decision: AuditDecision,
    decided_by: DecidedBy,
    extra_metadata: Option<serde_json::Value>,
) -> Option<AuditEntryDraft> {
    let q = format!(
        "SELECT meta::id(id) AS id, workflow_id, type, operation, details, risk_level \
         FROM validation_request:`{}`",
        validated_id
    );
    let rows: Vec<serde_json::Value> = state.db.query_json(&q).await.ok()?;
    let row = rows.first()?;
    let tool_name = row
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let workflow_id = row
        .get("workflow_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let operation = row.get("operation").and_then(|v| v.as_str()).unwrap_or("");
    let prompt_preview = if operation.is_empty() {
        None
    } else {
        Some(operation.to_string())
    };
    let risk_level: RiskLevel = row
        .get("risk_level")
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_value(serde_json::json!(s)).ok())
        .unwrap_or(RiskLevel::Medium);

    Some(AuditEntryDraft {
        validation_id: validated_id.to_string(),
        tool_name,
        decision,
        decided_by,
        risk_level,
        workflow_id,
        agent_id: None,
        prompt_preview,
        metadata: extra_metadata,
    })
}

/// Deletes a validation request.
///
/// # Arguments
/// * `validation_id` - The validation request ID to delete
#[tauri::command]
#[instrument(name = "delete_validation", skip(state), fields(validation_id = %validation_id))]
pub async fn delete_validation(
    validation_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Deleting validation request");

    let validated_id = validate_uuid_field(&validation_id, "validation_id")?;

    state
        .db
        .delete(&format!("validation_request:{}", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete validation");
            format!("Failed to delete validation: {}", e)
        })?;

    info!("Validation request deleted successfully");
    Ok(())
}

/// Gets the current validation settings.
/// Returns default settings if none are configured.
///
/// # Returns
/// The current validation settings with defaults applied if not configured
#[tauri::command]
#[instrument(name = "get_validation_settings", skip(state))]
pub async fn get_validation_settings(
    state: State<'_, AppState>,
) -> Result<ValidationSettings, String> {
    info!("Loading validation settings");

    // Try to load existing settings from settings:validation record
    let query = "SELECT config FROM settings:`settings:validation`";
    let results: Vec<serde_json::Value> = state.db.query_json(query).await.map_err(|e| {
        error!(error = %e, "Failed to query validation settings");
        format!("Failed to query validation settings: {}", e)
    })?;

    // If we have a result with a config field, parse it
    if let Some(first) = results.first() {
        if let Some(config) = first.get("config") {
            if !config.is_null() {
                match serde_json::from_value::<ValidationSettings>(config.clone()) {
                    Ok(settings) => {
                        info!("Validation settings loaded successfully");
                        return Ok(settings);
                    }
                    Err(e) => {
                        warn!(error = %e, "Failed to parse stored settings, using defaults");
                    }
                }
            }
        }
    }

    // Return defaults if not found or parsing failed
    info!("No validation settings found, returning defaults");
    Ok(ValidationSettings::default())
}

/// Updates validation settings with partial update support.
/// Only provided fields will be updated.
///
/// # Arguments
/// * `config` - The partial update request with fields to update
///
/// # Returns
/// The updated validation settings
#[tauri::command]
#[instrument(name = "update_validation_settings", skip(state, config))]
pub async fn update_validation_settings(
    config: UpdateValidationSettingsRequest,
    state: State<'_, AppState>,
) -> Result<ValidationSettings, String> {
    info!("Updating validation settings");

    // Load current settings (or defaults)
    let mut current = get_validation_settings_internal(&state).await?;

    // Apply partial updates
    if let Some(mode) = config.mode {
        current.mode = mode;
    }

    if let Some(selective) = config.selective_config {
        apply_selective_config(&mut current.selective_config, selective);
    }

    if let Some(risk) = config.risk_thresholds {
        apply_risk_thresholds(&mut current.risk_thresholds, risk);
    }

    if let Some(timeout) = config.timeout_seconds {
        let min = VALIDATION_TIMEOUT_MIN_SECS as i32;
        let max = VALIDATION_TIMEOUT_MAX_SECS as i32;
        if !(min..=max).contains(&timeout) {
            warn!(timeout, "Invalid timeout value");
            return Err(format!(
                "Timeout must be between {} and {} seconds",
                min, max
            ));
        }
        current.timeout_seconds = timeout;
    }

    if let Some(behavior) = config.timeout_behavior {
        current.timeout_behavior = behavior;
    }

    if let Some(audit) = config.audit {
        apply_audit_config(&mut current.audit, audit)?;
    }

    // Update timestamp
    current.updated_at = Utc::now();

    // Save to database using UPSERT
    // Follow the same pattern as embedding config (CONTENT with id field)
    let json_config = serialize_for_query(&current, "settings")?;

    let upsert_query = format!(
        "UPSERT settings:`settings:validation` CONTENT {{ id: 'settings:validation', config: {} }}",
        json_config
    );

    state.db.execute(&upsert_query).await.map_err(|e| {
        error!(error = %e, "Failed to save validation settings");
        format!("Failed to save validation settings: {}", e)
    })?;

    info!("Validation settings updated successfully");
    Ok(current)
}

/// Resets validation settings to defaults.
///
/// # Returns
/// The default validation settings
#[tauri::command]
#[instrument(name = "reset_validation_settings", skip(state))]
pub async fn reset_validation_settings(
    state: State<'_, AppState>,
) -> Result<ValidationSettings, String> {
    info!("Resetting validation settings to defaults");

    let settings = ValidationSettings::default();

    // Save defaults to database (follow embedding config pattern)
    let json_config = serialize_for_query(&settings, "default settings")?;

    let upsert_query = format!(
        "UPSERT settings:`settings:validation` CONTENT {{ id: 'settings:validation', config: {} }}",
        json_config
    );

    state.db.execute(&upsert_query).await.map_err(|e| {
        error!(error = %e, "Failed to save default settings");
        format!("Failed to save default settings: {}", e)
    })?;

    info!("Validation settings reset to defaults successfully");
    Ok(settings)
}

/// Internal helper to get validation settings without State wrapper
async fn get_validation_settings_internal(
    state: &State<'_, AppState>,
) -> Result<ValidationSettings, String> {
    let query = "SELECT config FROM settings:`settings:validation`";
    let results: Vec<serde_json::Value> = state
        .db
        .query_json(query)
        .await
        .map_err(|e| format!("Failed to query validation settings: {}", e))?;

    if let Some(first) = results.first() {
        if let Some(config) = first.get("config") {
            if !config.is_null() {
                if let Ok(settings) = serde_json::from_value::<ValidationSettings>(config.clone()) {
                    return Ok(settings);
                }
            }
        }
    }

    Ok(ValidationSettings::default())
}

/// Apply partial selective config updates
fn apply_selective_config(
    current: &mut SelectiveValidationConfig,
    partial: PartialSelectiveConfig,
) {
    if let Some(v) = partial.tools {
        current.tools = v;
    }
    if let Some(v) = partial.sub_agents {
        current.sub_agents = v;
    }
    if let Some(v) = partial.mcp {
        current.mcp = v;
    }
    if let Some(v) = partial.file_ops {
        current.file_ops = v;
    }
    if let Some(v) = partial.db_ops {
        current.db_ops = v;
    }
}

/// Apply partial risk thresholds updates
fn apply_risk_thresholds(current: &mut RiskThresholdConfig, partial: PartialRiskThresholds) {
    if let Some(v) = partial.auto_approve_low {
        current.auto_approve_low = v;
    }
    if let Some(v) = partial.always_confirm_high {
        current.always_confirm_high = v;
    }
}

/// Apply partial audit config updates with validation
fn apply_audit_config(
    current: &mut AuditConfig,
    partial: PartialAuditConfig,
) -> Result<(), String> {
    if let Some(v) = partial.enable_logging {
        current.enable_logging = v;
    }
    if let Some(v) = partial.retention_days {
        if !(RETENTION_MIN_DAYS..=RETENTION_MAX_DAYS).contains(&v) {
            return Err(format!(
                "Retention must be between {} and {} days",
                RETENTION_MIN_DAYS, RETENTION_MAX_DAYS
            ));
        }
        current.retention_days = v;
    }
    Ok(())
}

/// Information about an available tool for validation settings
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableToolInfo {
    /// Tool name/ID
    pub name: String,
    /// Tool category (basic, sub_agent)
    pub category: String,
    /// Whether the tool requires context
    pub requires_context: bool,
}

/// Lists all available local tools.
///
/// Returns tools registered in the tool registry that can be validated.
/// This includes basic tools and sub-agent tools.
///
/// # Returns
/// Vector of available tool information
#[tauri::command]
#[instrument(name = "list_available_tools", skip(_state))]
pub async fn list_available_tools(
    _state: State<'_, AppState>,
) -> Result<Vec<AvailableToolInfo>, String> {
    info!("Listing available tools for validation");

    let tools: Vec<AvailableToolInfo> = TOOL_REGISTRY
        .available_tools()
        .into_iter()
        .map(|name| {
            let is_sub_agent = TOOL_REGISTRY.requires_context(name);
            AvailableToolInfo {
                name: name.to_string(),
                category: if is_sub_agent {
                    "sub_agent".to_string()
                } else {
                    "basic".to_string()
                },
                requires_context: is_sub_agent,
            }
        })
        .collect();

    info!(count = tools.len(), "Available tools listed");
    Ok(tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_type_serialization() {
        let vtype = ValidationType::Tool;
        let json = serde_json::to_string(&vtype).unwrap();
        assert_eq!(json, "\"tool\"");

        let vtype = ValidationType::SubAgent;
        let json = serde_json::to_string(&vtype).unwrap();
        assert_eq!(json, "\"sub_agent\"");
    }

    #[test]
    fn test_risk_level_serialization() {
        let level = RiskLevel::Low;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"low\"");

        let level = RiskLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"high\"");
    }

    #[test]
    fn test_validation_request_structure() {
        let request = ValidationRequest {
            id: "val_001".to_string(),
            workflow_id: "wf_001".to_string(),
            validation_type: ValidationType::Mcp,
            operation: "Call external API".to_string(),
            details: serde_json::json!({"server": "serena", "method": "search"}),
            risk_level: RiskLevel::Medium,
            status: ValidationStatus::Pending,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"type\":\"mcp\""));
        assert!(json.contains("\"risk_level\":\"medium\""));
        assert!(json.contains("\"status\":\"pending\""));
    }

    #[tokio::test]
    async fn test_validation_status_values() {
        assert_eq!(
            serde_json::to_string(&ValidationStatus::Pending).unwrap(),
            "\"pending\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationStatus::Approved).unwrap(),
            "\"approved\""
        );
        assert_eq!(
            serde_json::to_string(&ValidationStatus::Rejected).unwrap(),
            "\"rejected\""
        );
    }
}
