// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Service layer types for agent page workflows.
 *
 * @module types/services
 */

import type { ValidationRequest } from './validation';

/**
 * Result of a workflow state restoration operation.
 */
export interface RestorationResult {
	/** Whether restoration was successful */
	success: boolean;
	/** Workflow ID that was restored */
	workflowId: string;
	/** Number of messages restored */
	messagesCount: number;
	/** Number of activities restored (tools + thinking + sub-agents) */
	activitiesCount: number;
	/** Error message if restoration failed */
	error?: string;
}

/**
 * Modal state for agent page dialogs.
 */
export type ModalState =
	| { type: 'none' }
	| { type: 'new-workflow' }
	| { type: 'delete-workflow'; workflowId: string }
	| { type: 'validation'; request: ValidationRequest };
