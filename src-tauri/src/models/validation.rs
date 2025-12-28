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

use super::serde_utils::deserialize_thing_id;
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
    /// Unique identifier (deserialized from SurrealDB Thing type)
    #[serde(deserialize_with = "deserialize_thing_id")]
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
    /// Creation timestamp (set by database)
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

/// Validation request creation payload - only fields needed for creation
/// ID is passed separately to db.create() using table:id format
/// Datetime field is handled by database default
/// Enum fields are converted to strings for SurrealDB compatibility
#[derive(Debug, Clone, Serialize)]
pub struct ValidationRequestCreate {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Type of validation (as string for SurrealDB)
    #[serde(rename = "type")]
    pub validation_type: String,
    /// Operation description
    pub operation: String,
    /// Additional details about the operation
    pub details: serde_json::Value,
    /// Risk level assessment (as string for SurrealDB)
    pub risk_level: String,
    /// Current validation status (as string for SurrealDB)
    pub status: String,
}

impl ValidationRequestCreate {
    /// Creates a new ValidationRequestCreate with the given parameters
    pub fn new(
        workflow_id: String,
        validation_type: ValidationType,
        operation: String,
        details: serde_json::Value,
        risk_level: RiskLevel,
        status: ValidationStatus,
    ) -> Self {
        Self {
            workflow_id,
            validation_type: validation_type.to_string(),
            operation,
            details,
            risk_level: risk_level.to_string(),
            status: status.to_string(),
        }
    }
}

impl std::fmt::Display for ValidationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationType::Tool => write!(f, "tool"),
            ValidationType::SubAgent => write!(f, "sub_agent"),
            ValidationType::Mcp => write!(f, "mcp"),
            ValidationType::FileOp => write!(f, "file_op"),
            ValidationType::DbOp => write!(f, "db_op"),
        }
    }
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Low => write!(f, "low"),
            RiskLevel::Medium => write!(f, "medium"),
            RiskLevel::High => write!(f, "high"),
        }
    }
}

impl std::fmt::Display for ValidationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationStatus::Pending => write!(f, "pending"),
            ValidationStatus::Approved => write!(f, "approved"),
            ValidationStatus::Rejected => write!(f, "rejected"),
        }
    }
}

// =====================================================
// Validation Settings Types (Global Configuration)
// =====================================================

/// Timeout behavior when validation request expires
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TimeoutBehavior {
    #[default]
    Reject,
    Approve,
    AskAgain,
}

impl std::fmt::Display for TimeoutBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TimeoutBehavior::Reject => write!(f, "reject"),
            TimeoutBehavior::Approve => write!(f, "approve"),
            TimeoutBehavior::AskAgain => write!(f, "ask_again"),
        }
    }
}

/// Selective validation configuration - which operations require validation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectiveValidationConfig {
    /// Validate internal tool execution
    pub tools: bool,
    /// Validate sub-agent spawn
    pub sub_agents: bool,
    /// Validate MCP server calls
    pub mcp: bool,
    /// Validate file write/delete operations
    pub file_ops: bool,
    /// Validate database write/delete operations
    pub db_ops: bool,
}

impl Default for SelectiveValidationConfig {
    fn default() -> Self {
        Self {
            tools: false,
            sub_agents: true,
            mcp: true,
            file_ops: true,
            db_ops: true,
        }
    }
}

/// Risk threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RiskThresholdConfig {
    /// Skip validation for low-risk operations
    pub auto_approve_low: bool,
    /// Always require validation for high-risk (even in Auto mode)
    pub always_confirm_high: bool,
}

impl Default for RiskThresholdConfig {
    fn default() -> Self {
        Self {
            auto_approve_low: true,
            always_confirm_high: false,
        }
    }
}

/// Audit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditConfig {
    /// Enable validation decision logging
    pub enable_logging: bool,
    /// Log retention in days (7-90)
    pub retention_days: i32,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self {
            enable_logging: true,
            retention_days: 30,
        }
    }
}

/// Main validation settings configuration
#[allow(dead_code)] // API type for validation settings creation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSettingsConfig {
    /// Validation mode
    pub mode: ValidationMode,
    /// Selective config (used when mode = 'selective')
    pub selective_config: SelectiveValidationConfig,
    /// Risk threshold settings
    pub risk_thresholds: RiskThresholdConfig,
    /// Timeout in seconds (30-300)
    pub timeout_seconds: i32,
    /// Behavior when timeout expires
    pub timeout_behavior: TimeoutBehavior,
    /// Audit settings
    pub audit: AuditConfig,
}

impl Default for ValidationSettingsConfig {
    fn default() -> Self {
        Self {
            mode: ValidationMode::Selective,
            selective_config: SelectiveValidationConfig::default(),
            risk_thresholds: RiskThresholdConfig::default(),
            timeout_seconds: 60,
            timeout_behavior: TimeoutBehavior::default(),
            audit: AuditConfig::default(),
        }
    }
}

/// Validation settings with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationSettings {
    /// Validation mode
    pub mode: ValidationMode,
    /// Selective config (used when mode = 'selective')
    pub selective_config: SelectiveValidationConfig,
    /// Risk threshold settings
    pub risk_thresholds: RiskThresholdConfig,
    /// Timeout in seconds (30-300)
    pub timeout_seconds: i32,
    /// Behavior when timeout expires
    pub timeout_behavior: TimeoutBehavior,
    /// Audit settings
    pub audit: AuditConfig,
    /// Last update timestamp
    pub updated_at: DateTime<Utc>,
}

impl Default for ValidationSettings {
    fn default() -> Self {
        Self {
            mode: ValidationMode::Selective,
            selective_config: SelectiveValidationConfig::default(),
            risk_thresholds: RiskThresholdConfig::default(),
            timeout_seconds: 60,
            timeout_behavior: TimeoutBehavior::default(),
            audit: AuditConfig::default(),
            updated_at: Utc::now(),
        }
    }
}

/// Update request for partial updates (all fields optional)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateValidationSettingsRequest {
    /// New validation mode
    pub mode: Option<ValidationMode>,
    /// Update selective config
    pub selective_config: Option<PartialSelectiveConfig>,
    /// Update risk thresholds
    pub risk_thresholds: Option<PartialRiskThresholds>,
    /// New timeout in seconds
    pub timeout_seconds: Option<i32>,
    /// New timeout behavior
    pub timeout_behavior: Option<TimeoutBehavior>,
    /// Update audit settings
    pub audit: Option<PartialAuditConfig>,
}

/// Partial selective config for updates
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialSelectiveConfig {
    pub tools: Option<bool>,
    pub sub_agents: Option<bool>,
    pub mcp: Option<bool>,
    pub file_ops: Option<bool>,
    pub db_ops: Option<bool>,
}

/// Partial risk thresholds for updates
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialRiskThresholds {
    pub auto_approve_low: Option<bool>,
    pub always_confirm_high: Option<bool>,
}

/// Partial audit config for updates
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartialAuditConfig {
    pub enable_logging: Option<bool>,
    pub retention_days: Option<i32>,
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
