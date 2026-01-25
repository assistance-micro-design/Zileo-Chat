/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

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
 * Details for spawn sub-agent validation
 */
export interface SpawnValidationDetails {
  /** Name of the sub-agent to spawn */
  sub_agent_name: string;
  /** Preview of the prompt (truncated to 200 chars) */
  prompt_preview: string;
  /** Full length of the prompt */
  prompt_length: number;
  /** Tools assigned to the sub-agent */
  tools: string[];
  /** MCP servers assigned to the sub-agent */
  mcp_servers: string[];
  /** Allow additional custom fields */
  [key: string]: unknown;
}

/**
 * Details for delegate operation validation
 */
export interface DelegateValidationDetails {
  /** Target agent ID to delegate to */
  target_agent_id: string;
  /** Target agent name */
  target_agent_name: string;
  /** Preview of the prompt (truncated to 200 chars) */
  prompt_preview: string;
  /** Full length of the prompt */
  prompt_length: number;
  /** Allow additional custom fields */
  [key: string]: unknown;
}

/**
 * Task information for parallel batch validation
 */
export interface ParallelTaskInfo {
  /** Agent ID for the task */
  agent_id: string;
  /** Preview of the prompt (truncated to 100 chars) */
  prompt_preview: string;
}

/**
 * Details for parallel batch operation validation
 */
export interface ParallelValidationDetails {
  /** Number of tasks to execute in parallel */
  task_count: number;
  /** List of tasks with agent IDs and prompt previews */
  tasks: ParallelTaskInfo[];
  /** Allow additional custom fields */
  [key: string]: unknown;
}

/**
 * Generic validation details for operations without specific structure
 */
export interface GenericValidationDetails {
  /** Optional file path for file operations */
  path?: string;
  /** Rejection reason (added when status changes to rejected) */
  rejection_reason?: string;
  /** Allow additional custom fields */
  [key: string]: unknown;
}

/**
 * Union type for all validation details.
 *
 * The structure depends on the ValidationType:
 * - sub_agent with Spawn -> SpawnValidationDetails
 * - sub_agent with Delegate -> DelegateValidationDetails
 * - sub_agent with ParallelBatch -> ParallelValidationDetails
 * - Other types -> GenericValidationDetails
 */
export type ValidationDetails =
  | SpawnValidationDetails
  | DelegateValidationDetails
  | ParallelValidationDetails
  | GenericValidationDetails;

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
  details: ValidationDetails;
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
 * Information about an available tool for validation settings
 */
export interface AvailableToolInfo {
  /** Tool name/ID */
  name: string;
  /** Tool category (basic, sub_agent) */
  category: 'basic' | 'sub_agent';
  /** Whether the tool requires context */
  requiresContext: boolean;
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
