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
