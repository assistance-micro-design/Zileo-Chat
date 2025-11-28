// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Validation commands for human-in-the-loop operations.
//!
//! Provides Tauri commands for managing validation requests that require
//! human approval before execution (tools, sub-agents, MCP calls, etc.).

use crate::{
    models::{
        RiskLevel, ValidationRequest, ValidationRequestCreate, ValidationStatus, ValidationType,
    },
    security::Validator,
    AppState,
};
use chrono::Utc;
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

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

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
        .query("SELECT * FROM validation_request WHERE status = 'pending' ORDER BY created_at DESC")
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

    // Validate workflow ID
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow ID");
        format!("Invalid workflow ID: {}", e)
    })?;

    let validations: Vec<ValidationRequest> = state
        .db
        .query(&format!(
            "SELECT * FROM validation_request WHERE workflow_id = '{}' ORDER BY created_at DESC",
            validated_workflow_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to load workflow validations");
            format!("Failed to load workflow validations: {}", e)
        })?;

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

    // Validate validation ID
    let validated_id = Validator::validate_uuid(&validation_id).map_err(|e| {
        warn!(error = %e, "Invalid validation ID");
        format!("Invalid validation ID: {}", e)
    })?;

    // Update status to approved using SurrealDB record ID format
    state
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'approved'",
            validated_id
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to approve validation");
            format!("Failed to approve validation: {}", e)
        })?;

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

    // Validate validation ID
    let validated_id = Validator::validate_uuid(&validation_id).map_err(|e| {
        warn!(error = %e, "Invalid validation ID");
        format!("Invalid validation ID: {}", e)
    })?;

    // Validate reason
    let validated_reason = Validator::validate_message(&reason).map_err(|e| {
        warn!(error = %e, "Invalid rejection reason");
        format!("Invalid rejection reason: {}", e)
    })?;

    // Update status to rejected and store reason in details using SurrealDB record ID format
    // Use JSON encoding for the reason to handle special characters
    let reason_json = serde_json::to_string(&validated_reason)
        .unwrap_or_else(|_| "\"Unknown reason\"".to_string());
    state
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'rejected', details.rejection_reason = {}",
            validated_id, reason_json
        ))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to reject validation");
            format!("Failed to reject validation: {}", e)
        })?;

    info!("Validation request rejected successfully");
    Ok(())
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

    // Validate validation ID
    let validated_id = Validator::validate_uuid(&validation_id).map_err(|e| {
        warn!(error = %e, "Invalid validation ID");
        format!("Invalid validation ID: {}", e)
    })?;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::{AgentOrchestrator, AgentRegistry};
    use crate::db::DBClient;
    use crate::llm::ProviderManager;
    use std::sync::Arc;
    use tempfile::tempdir;

    #[allow(dead_code)]
    async fn setup_test_state() -> AppState {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_validation_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = Arc::new(
            DBClient::new(db_path_str)
                .await
                .expect("Failed to create test DB"),
        );
        db.initialize_schema().await.expect("Schema init failed");

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
        let llm_manager = Arc::new(ProviderManager::new());
        let mcp_manager = Arc::new(
            crate::mcp::MCPManager::new(db.clone())
                .await
                .expect("Failed to create MCP manager"),
        );

        std::mem::forget(temp_dir);

        AppState {
            db: db.clone(),
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory: Arc::new(crate::tools::ToolFactory::new(db, None)),
            embedding_service: Arc::new(tokio::sync::RwLock::new(None)),
            streaming_cancellations: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
            app_handle: Arc::new(std::sync::RwLock::new(None)),
        }
    }

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
