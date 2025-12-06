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
