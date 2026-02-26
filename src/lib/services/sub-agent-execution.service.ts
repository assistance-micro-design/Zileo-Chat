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
 * @fileoverview Sub-agent execution service for loading execution data.
 *
 * @module lib/services/sub-agent-execution
 */

import { invoke } from '@tauri-apps/api/core';
import type { SubAgentExecution } from '$types/sub-agent';

/**
 * Service for sub-agent execution operations.
 *
 * Provides loading of sub-agent executions for message enrichment.
 */
export const SubAgentExecutionService = {
	/**
	 * Load sub-agent executions for a workflow.
	 *
	 * @param workflowId - Workflow ID
	 * @returns Array of sub-agent executions
	 */
	async loadSubAgentExecutions(workflowId: string): Promise<SubAgentExecution[]> {
		try {
			return await invoke<SubAgentExecution[]>('load_workflow_sub_agent_executions', { workflowId });
		} catch {
			return [];
		}
	}
};
