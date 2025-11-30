// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Validation mode for human-in-the-loop
 */
export type ValidationMode = 'auto' | 'manual' | 'selective';

/**
 * Type of operation requiring validation
 */
export type ValidationType = 'tool' | 'sub_agent' | 'mcp' | 'file_op' | 'db_op';

/**
 * Risk level of the operation
 */
export type RiskLevel = 'low' | 'medium' | 'high' | 'critical';

/**
 * Validation status for human-in-the-loop requests
 */
export type ValidationStatus = 'pending' | 'approved' | 'rejected';

/**
 * Validation request entity
 */
export interface ValidationRequest {
  /** Unique identifier */
  id: string;
  /** Associated workflow ID */
  workflow_id: string;
  /** Type of validation */
  type: ValidationType;
  /** Operation description */
  operation: string;
  /** Additional details about the operation */
  details: Record<string, unknown>;
  /** Risk level assessment */
  risk_level: RiskLevel;
  /** Current validation status */
  status: ValidationStatus;
  /** Creation timestamp */
  created_at: Date;
}

// =====================================================
// Validation Settings Types (Global Configuration)
// =====================================================

/**
 * Timeout behavior when validation request expires
 */
export type TimeoutBehavior = 'reject' | 'approve' | 'ask_again';

/**
 * Selective validation config - which operations require validation
 */
export interface SelectiveValidationConfig {
  /** Validate internal tool execution */
  tools: boolean;
  /** Validate sub-agent spawn */
  subAgents: boolean;
  /** Validate MCP server calls */
  mcp: boolean;
  /** Validate file write/delete operations */
  fileOps: boolean;
  /** Validate database write/delete operations */
  dbOps: boolean;
}

/**
 * Risk threshold configuration
 */
export interface RiskThresholdConfig {
  /** Skip validation for low-risk operations */
  autoApproveLow: boolean;
  /** Always require validation for high-risk (even in Auto mode) */
  alwaysConfirmHigh: boolean;
}

/**
 * Audit/logging configuration
 */
export interface AuditConfig {
  /** Enable validation decision logging */
  enableLogging: boolean;
  /** Log retention in days (7-90) */
  retentionDays: number;
}

/**
 * Main validation settings configuration
 */
export interface ValidationSettingsConfig {
  /** Validation mode */
  mode: ValidationMode;
  /** Selective config (used when mode = 'selective') */
  selectiveConfig: SelectiveValidationConfig;
  /** Risk threshold settings */
  riskThresholds: RiskThresholdConfig;
  /** Timeout in seconds (30-300) */
  timeoutSeconds: number;
  /** Behavior when timeout expires */
  timeoutBehavior: TimeoutBehavior;
  /** Audit settings */
  audit: AuditConfig;
}

/**
 * Validation settings with metadata
 */
export interface ValidationSettings extends ValidationSettingsConfig {
  /** Last update timestamp (ISO 8601) */
  updatedAt: string;
}

/**
 * Update request for partial updates
 */
export interface UpdateValidationSettingsRequest {
  mode?: ValidationMode;
  selectiveConfig?: Partial<SelectiveValidationConfig>;
  riskThresholds?: Partial<RiskThresholdConfig>;
  timeoutSeconds?: number;
  timeoutBehavior?: TimeoutBehavior;
  audit?: Partial<AuditConfig>;
}

/**
 * Default validation settings values
 */
export const DEFAULT_VALIDATION_SETTINGS: ValidationSettingsConfig = {
  mode: 'selective',
  selectiveConfig: {
    tools: false,
    subAgents: true,
    mcp: true,
    fileOps: true,
    dbOps: true,
  },
  riskThresholds: {
    autoApproveLow: true,
    alwaysConfirmHigh: false,
  },
  timeoutSeconds: 60,
  timeoutBehavior: 'reject',
  audit: {
    enableLogging: true,
    retentionDays: 30,
  },
};
