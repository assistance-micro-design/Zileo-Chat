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
 * @fileoverview Activity icon mapping utilities
 * @module lib/utils/activity-icons
 */

import type { Component } from 'svelte';
import { Activity, Wrench, Bot, Brain, ListTodo, ShieldCheck } from '@lucide/svelte';

/**
 * Activity type icon mapping
 */
export const ACTIVITY_TYPE_ICONS = {
	tool: Wrench,
	reasoning: Brain,
	sub_agent: Bot,
	validation: ShieldCheck,
	task: ListTodo,
	default: Activity
} as const;

/**
 * Get icon component based on activity type
 */
export function getActivityIcon(type: string): Component {
	if (type.startsWith('tool_')) return ACTIVITY_TYPE_ICONS.tool;
	if (type === 'reasoning') return ACTIVITY_TYPE_ICONS.reasoning;
	if (type.startsWith('sub_agent_')) return ACTIVITY_TYPE_ICONS.sub_agent;
	if (type === 'validation') return ACTIVITY_TYPE_ICONS.validation;
	if (type.startsWith('task_')) return ACTIVITY_TYPE_ICONS.task;
	return ACTIVITY_TYPE_ICONS.default;
}
