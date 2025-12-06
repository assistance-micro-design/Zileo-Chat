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
 * @fileoverview Task types for workflow decomposition and Todo Tool support.
 *
 * Tasks break down complex workflows into trackable units with:
 * - Priority levels (1=critical to 5=low)
 * - Status tracking (pending/in_progress/completed/blocked)
 * - Agent assignment for multi-agent coordination
 * - Dependency management between tasks
 * - Duration tracking for metrics
 *
 * @module types/task
 *
 * @example
 * ```typescript
 * import type { Task, TaskStatus, CreateTaskParams } from '$types/task';
 *
 * const params: CreateTaskParams = {
 *   workflowId: 'wf_001',
 *   name: 'Analyze code',
 *   description: 'Deep analysis of the codebase',
 *   priority: 1,
 *   agentAssigned: 'db_agent'
 * };
 *
 * const taskId = await invoke<string>('create_task', params);
 * ```
 */

/**
 * Task status for workflow decomposition.
 *
 * Represents the current state of a task in its lifecycle:
 * - `pending`: Task is waiting to be started
 * - `in_progress`: Task is currently being worked on
 * - `completed`: Task has been finished successfully
 * - `blocked`: Task is blocked by dependencies or external factors
 */
export type TaskStatus = 'pending' | 'in_progress' | 'completed' | 'blocked';

/**
 * Task priority level (1=critical, 5=low).
 *
 * Priority determines the order of task execution:
 * - 1: Critical - must be done immediately
 * - 2: High - should be done soon
 * - 3: Medium - normal priority (default)
 * - 4: Low - can wait
 * - 5: Minimal - do when time permits
 */
export type TaskPriority = 1 | 2 | 3 | 4 | 5;

/**
 * Task entity for workflow decomposition.
 *
 * Represents a single unit of work within a workflow, with support for:
 * - Agent assignment for multi-agent coordination
 * - Priority-based scheduling
 * - Dependency tracking between tasks
 * - Execution metrics (duration)
 *
 * Field names use snake_case to match SurrealDB/Rust serialization.
 */
export interface Task {
  /** Unique identifier (UUID) */
  id: string;
  /** Associated workflow ID */
  workflow_id: string;
  /** Task name (short identifier, max 128 chars) */
  name: string;
  /** Detailed description (max 1000 chars) */
  description: string;
  /** Agent responsible for this task (optional) */
  agent_assigned?: string;
  /** Priority level (1-5, 1=critical) */
  priority: TaskPriority;
  /** Current status */
  status: TaskStatus;
  /** Task dependencies (other task IDs that must complete first) */
  dependencies: string[];
  /** Execution duration in milliseconds (if completed) */
  duration_ms?: number;
  /** Creation timestamp (ISO string from backend) */
  created_at: string;
  /** Completion timestamp (if completed, ISO string from backend) */
  completed_at?: string;
}

/**
 * Parameters for creating a new task via Tauri IPC.
 *
 * Uses camelCase for invoke() calls (Tauri auto-converts to snake_case).
 *
 * @example
 * ```typescript
 * const params: CreateTaskParams = {
 *   workflowId: 'wf_001',
 *   name: 'Analyze code',
 *   description: 'Deep analysis',
 *   priority: 1,
 *   agentAssigned: 'db_agent',
 *   dependencies: ['task_prev_001']
 * };
 *
 * const taskId = await invoke<string>('create_task', params);
 * ```
 */
export interface CreateTaskParams {
  /** Associated workflow ID */
  workflowId: string;
  /** Task name (short identifier, max 128 chars) */
  name: string;
  /** Detailed description (max 1000 chars) */
  description: string;
  /** Priority level 1-5 (optional, default: 3) */
  priority?: TaskPriority;
  /** Agent ID to assign (optional) */
  agentAssigned?: string;
  /** Task dependencies - IDs of tasks that must complete first (optional) */
  dependencies?: string[];
}

/**
 * Parameters for updating a task via Tauri IPC.
 *
 * All fields are optional - only provided fields will be updated.
 * Uses camelCase for invoke() calls.
 *
 * @example
 * ```typescript
 * const updates: UpdateTaskParams = {
 *   priority: 2,
 *   description: 'Updated description'
 * };
 *
 * const updated = await invoke<Task>('update_task', {
 *   taskId: 'task_001',
 *   updates
 * });
 * ```
 */
export interface UpdateTaskParams {
  /** New task name */
  name?: string;
  /** New description */
  description?: string;
  /** New agent assignment */
  agentAssigned?: string;
  /** New priority */
  priority?: TaskPriority;
  /** New status */
  status?: TaskStatus;
  /** New dependencies list */
  dependencies?: string[];
  /** Execution duration in milliseconds */
  durationMs?: number;
}

/**
 * Result of a task list query with count.
 *
 * Used for paginated results or summary information.
 */
export interface TaskListResult {
  /** Array of tasks */
  tasks: Task[];
  /** Total count matching filter */
  total: number;
}

/**
 * Parameters for listing tasks by status.
 */
export interface ListTasksByStatusParams {
  /** Status to filter by */
  status: TaskStatus;
  /** Optional workflow ID to further filter */
  workflowId?: string;
}

/**
 * Parameters for completing a task.
 */
export interface CompleteTaskParams {
  /** Task ID to complete */
  taskId: string;
  /** Optional execution duration in milliseconds */
  durationMs?: number;
}
