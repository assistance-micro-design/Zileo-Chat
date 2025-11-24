// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Validation mode for human-in-the-loop
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationMode {
    Auto,
    Manual,
    Selective,
}

/// Type of operation requiring validation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationType {
    Tool,
    SubAgent,
    Mcp,
    FileOp,
    DbOp,
}

/// Risk level of the operation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

/// Validation status for human-in-the-loop requests
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Pending,
    Approved,
    Rejected,
}

impl Default for ValidationStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Validation request entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    /// Unique identifier
    pub id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Type of validation
    #[serde(rename = "type")]
    pub validation_type: ValidationType,
    /// Operation description
    pub operation: String,
    /// Additional details about the operation
    pub details: serde_json::Value,
    /// Risk level assessment
    pub risk_level: RiskLevel,
    /// Current validation status
    #[serde(default)]
    pub status: ValidationStatus,
    /// Creation timestamp
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_mode_serialization() {
        let mode = ValidationMode::Auto;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"auto\"");

        let deserialized: ValidationMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ValidationMode::Auto);
    }

    #[test]
    fn test_validation_type_serialization() {
        let vtype = ValidationType::SubAgent;
        let json = serde_json::to_string(&vtype).unwrap();
        assert_eq!(json, "\"sub_agent\"");

        let deserialized: ValidationType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ValidationType::SubAgent);
    }

    #[test]
    fn test_risk_level_serialization() {
        let level = RiskLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"high\"");

        let deserialized: RiskLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, RiskLevel::High);
    }

    #[test]
    fn test_validation_status_serialization() {
        let status = ValidationStatus::Approved;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"approved\"");

        let deserialized: ValidationStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ValidationStatus::Approved);
    }

    #[test]
    fn test_validation_request_serialization() {
        let request = ValidationRequest {
            id: "req_001".to_string(),
            workflow_id: "wf_001".to_string(),
            validation_type: ValidationType::Tool,
            operation: "file_write".to_string(),
            details: serde_json::json!({"path": "/tmp/test.txt"}),
            risk_level: RiskLevel::Medium,
            status: ValidationStatus::Pending,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&request).unwrap();
        let deserialized: ValidationRequest = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, request.id);
        assert_eq!(deserialized.validation_type, request.validation_type);
        assert_eq!(deserialized.risk_level, request.risk_level);
        assert_eq!(deserialized.status, request.status);
    }

    #[test]
    fn test_validation_request_type_field_rename() {
        let request = ValidationRequest {
            id: "req_001".to_string(),
            workflow_id: "wf_001".to_string(),
            validation_type: ValidationType::Mcp,
            operation: "call_server".to_string(),
            details: serde_json::json!({}),
            risk_level: RiskLevel::Low,
            status: ValidationStatus::Pending,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&request).unwrap();
        // Verify that 'validation_type' is serialized as 'type' in JSON
        assert!(json.contains("\"type\":\"mcp\""));
    }
}
