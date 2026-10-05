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

/**
 * @fileoverview Typed Tauri API client for the scheduler domain.
 *
 * Covers the Kanban scheduler budget and recurrence schedules
 * (`kanban_schedule` rows).
 *
 * @module lib/api/scheduler
 */

import { invokeCommand } from './internal';
import type {
	KanbanSchedule,
	KanbanScheduleCreate,
	KanbanScheduleUpdate
} from '$types/kanban';

/** Backend worker-promotion concurrency budget. */
export async function getMaxConcurrentWorkflows(): Promise<number> {
	return invokeCommand<number>('get_max_concurrent_workflows');
}

/** List all recurrence schedules. */
export async function listKanbanSchedules(): Promise<KanbanSchedule[]> {
	return invokeCommand<KanbanSchedule[]>('list_kanban_schedules');
}

/** Get a single schedule. */
export async function getKanbanSchedule(id: string): Promise<KanbanSchedule> {
	return invokeCommand<KanbanSchedule>('get_kanban_schedule', { id });
}

/** Create a schedule, returning its ID. */
export async function createKanbanSchedule(config: KanbanScheduleCreate): Promise<string> {
	return invokeCommand<string>('create_kanban_schedule', { config });
}

/** Update a schedule. */
export async function updateKanbanSchedule(
	id: string,
	config: KanbanScheduleUpdate
): Promise<KanbanSchedule> {
	return invokeCommand<KanbanSchedule>('update_kanban_schedule', { id, config });
}

/** Delete a schedule. */
export async function deleteKanbanSchedule(id: string): Promise<void> {
	return invokeCommand<void>('delete_kanban_schedule', { id });
}
