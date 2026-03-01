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
 * Skill Store
 *
 * Manages skill state with Tauri IPC integration.
 * Uses the CRUD store factory for standardized state management.
 *
 * @module stores/skills
 */

import { derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { createCRUDStore, createDerivedStores } from './factory/createCRUDStore';
import { getErrorMessage } from '$lib/utils/error';
import type {
	Skill,
	SkillCreate,
	SkillUpdate,
	SkillSummary,
	SkillStoreState
} from '$types/skill';

// ============================================================================
// Base CRUD Store
// ============================================================================

const baseCrudStore = createCRUDStore<Skill, SkillCreate, SkillUpdate, SkillSummary>({
	name: 'skill',
	idParamName: 'skillId',
	commands: {
		list: 'list_skills',
		get: 'get_skill',
		create: 'create_skill',
		update: 'update_skill',
		delete: 'delete_skill'
	}
});

// ============================================================================
// Skill Store (with backward-compatible API + toggle extension)
// ============================================================================

/**
 * Skill store with actions for CRUD operations and UI state management.
 * Extends the base CRUD store with skill-specific toggle functionality.
 */
export const skillStore = {
	/**
	 * Subscribe to store changes.
	 * Maps internal state to SkillStoreState interface.
	 */
	subscribe: (run: (value: SkillStoreState) => void) => {
		return baseCrudStore.subscribe((state) => {
			run({
				skills: state.items,
				selectedId: state.selectedId,
				loading: state.loading,
				error: state.error,
				formMode: state.formMode,
				editingSkill: state.editing
			});
		});
	},

	// ===== CRUD Operations =====

	/** Load all skills from backend */
	loadSkills: () => baseCrudStore.loadItems(),

	/** Get full skill by ID */
	getSkill: (id: string) => baseCrudStore.getItem(id),

	/** Create a new skill */
	createSkill: (config: SkillCreate) => baseCrudStore.createItem(config),

	/** Update an existing skill */
	async updateSkill(id: string, updates: SkillUpdate): Promise<Skill> {
		baseCrudStore._store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const updated = await invoke<Skill>('update_skill', {
				skillId: id,
				config: updates
			});
			await baseCrudStore.loadItems();
			baseCrudStore._store.update((s) => ({
				...s,
				formMode: null,
				editing: null,
				loading: false
			}));
			return updated;
		} catch (e) {
			baseCrudStore._store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	/** Delete a skill */
	deleteSkill: (id: string) => baseCrudStore.deleteItem(id),

	/**
	 * Toggle skill enabled/disabled state.
	 * Skill-specific extension, not part of base CRUD.
	 */
	async toggleEnabled(id: string, enabled: boolean): Promise<void> {
		baseCrudStore._store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			await invoke('update_skill', {
				skillId: id,
				config: { enabled }
			});
			await baseCrudStore.loadItems();
		} catch (e) {
			baseCrudStore._store.update((s) => ({ ...s, error: getErrorMessage(e), loading: false }));
			throw e;
		}
	},

	// ===== UI State Management =====

	/** Select a skill by ID */
	select: (skillId: string | null) => baseCrudStore.select(skillId),

	/** Open the create form */
	openCreateForm: () => baseCrudStore.openCreateForm(),

	/** Open the edit form for a specific skill */
	openEditForm: (id: string) => baseCrudStore.openEditForm(id),

	/** Close the form (create or edit) */
	closeForm: () => baseCrudStore.closeForm(),

	/** Clear error state */
	clearError: () => baseCrudStore.clearError(),

	/** Reset store to initial state */
	reset: () => baseCrudStore.reset()
};

// ============================================================================
// Derived Stores
// ============================================================================

const derivedStores = createDerivedStores(baseCrudStore);

/** All skills (summaries) */
export const skills = derivedStores.items;

/** Currently selected skill ID */
export const selectedSkillId = derived(baseCrudStore._store, (s) => s.selectedId);

/** Currently selected skill (from list) */
export const selectedSkill = derivedStores.selected;

/** Skill loading state */
export const skillLoading = derivedStores.isLoading;

/** Skill error state */
export const skillError = derivedStores.error;

/** Skill form mode */
export const skillFormMode = derivedStores.formMode;

/** Skill being edited (full data) */
export const editingSkill = derivedStores.editing;

/** Whether any skills exist */
export const hasSkills = derivedStores.hasItems;

/** Number of skills */
export const skillCount = derivedStores.count;

/** Only enabled skills */
export const enabledSkills = derived(baseCrudStore._store, (s) =>
	s.items.filter((skill) => skill.enabled)
);
