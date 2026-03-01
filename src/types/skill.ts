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

// Skill Types
// Synchronized with src-tauri/src/models/skill.rs

// ===== Core Types =====

/** Category for organizing skills */
export type SkillCategory = 'system' | 'coding' | 'workflow' | 'analysis' | 'custom';

/** Full skill entity (from backend) */
export interface Skill {
	/** Unique identifier */
	id: string;
	/** Skill name - unique, 1-128 chars, [a-zA-Z0-9_-] only */
	name: string;
	/** Short description shown in system prompt, 1-500 chars */
	description: string;
	/** Category for organization */
	category: SkillCategory;
	/** Markdown content, 1-50000 chars */
	content: string;
	/** Whether the skill is active */
	enabled: boolean;
	/** ISO 8601 creation timestamp */
	created_at: string;
	/** ISO 8601 last update timestamp */
	updated_at: string;
}

/** Lightweight skill for list display */
export interface SkillSummary {
	/** Unique identifier */
	id: string;
	/** Skill name */
	name: string;
	/** Short description */
	description: string;
	/** Category */
	category: SkillCategory;
	/** Whether the skill is active */
	enabled: boolean;
	/** Character count of content (for UI display) */
	content_length: number;
	/** ISO 8601 last update timestamp */
	updated_at: string;
}

/** Skill creation payload (no id, no timestamps) */
export interface SkillCreate {
	/** Skill name - unique, [a-zA-Z0-9_-] only */
	name: string;
	/** Short description */
	description: string;
	/** Category */
	category: SkillCategory;
	/** Markdown content */
	content: string;
}

/** Skill update payload (all fields optional) */
export interface SkillUpdate {
	/** New name */
	name?: string;
	/** New description */
	description?: string;
	/** New category */
	category?: SkillCategory;
	/** New content */
	content?: string;
	/** Enable/disable */
	enabled?: boolean;
}

// ===== Store State =====

export interface SkillStoreState {
	skills: SkillSummary[];
	selectedId: string | null;
	loading: boolean;
	error: string | null;
	formMode: 'create' | 'edit' | null;
	editingSkill: Skill | null;
}

// ===== Category Labels (for UI) =====

export const SKILL_CATEGORY_LABELS: Record<SkillCategory, string> = {
	system: 'System',
	coding: 'Coding',
	workflow: 'Workflow',
	analysis: 'Analysis',
	custom: 'Custom'
};
