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
 * Folder store for managing workflow folder state in the frontend.
 * @module stores/folders
 */

import type { WorkflowFolder } from '$types/workflow';

import { writable, derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import { getErrorMessage } from '$lib/utils/error';
import { LocalStorage, STORAGE_KEYS } from '$lib/services/localStorage.service';

/**
 * State interface for the folder store
 */
interface FolderState {
	/** List of all folders */
	folders: WorkflowFolder[];
	/** Loading state */
	loading: boolean;
	/** Error message if any */
	error: string | null;
	/** Set of expanded folder IDs (accordion state) */
	expandedFolderIds: Set<string>;
}

/**
 * Initial state
 */
const initialState: FolderState = {
	folders: [],
	loading: false,
	error: null,
	expandedFolderIds: new Set<string>(
		LocalStorage.get<string[]>(STORAGE_KEYS.EXPANDED_FOLDER_IDS, [])
	)
};

/**
 * Internal writable store
 */
const folderWritable = writable<FolderState>(initialState);

/**
 * Persist expanded folder IDs to localStorage
 */
function persistExpandedIds(ids: Set<string>): void {
	LocalStorage.set(STORAGE_KEYS.EXPANDED_FOLDER_IDS, [...ids]);
}

/**
 * Folder store with CRUD operations.
 */
export const folderStore = {
	subscribe: folderWritable.subscribe,

	/**
	 * Load all folders from backend.
	 */
	async loadFolders(): Promise<void> {
		folderWritable.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const folders = await invoke<WorkflowFolder[]>('list_workflow_folders');
			folderWritable.update((s) => ({ ...s, folders, loading: false }));
		} catch (e) {
			const error = getErrorMessage(e);
			folderWritable.update((s) => ({ ...s, error, loading: false }));
		}
	},

	/**
	 * Create a new folder.
	 *
	 * @param name - Folder display name
	 * @param color - Hex color (#RRGGBB)
	 * @returns Created folder ID
	 */
	async createFolder(name: string, color: string): Promise<string> {
		const folder = await invoke<WorkflowFolder>('create_workflow_folder', { name, color });
		folderWritable.update((s) => ({
			...s,
			folders: [...s.folders, folder]
		}));
		return folder.id;
	},

	/**
	 * Rename an existing folder.
	 *
	 * @param folderId - The folder ID to rename
	 * @param name - The new name
	 */
	async renameFolder(folderId: string, name: string): Promise<void> {
		const updated = await invoke<WorkflowFolder>('rename_workflow_folder', { folderId, name });
		folderWritable.update((s) => ({
			...s,
			folders: s.folders.map((f) => (f.id === updated.id ? updated : f))
		}));
	},

	/**
	 * Update a folder's color.
	 *
	 * @param folderId - The folder ID to update
	 * @param color - The new hex color
	 */
	async updateColor(folderId: string, color: string): Promise<void> {
		const updated = await invoke<WorkflowFolder>('update_folder_color', { folderId, color });
		folderWritable.update((s) => ({
			...s,
			folders: s.folders.map((f) => (f.id === updated.id ? updated : f))
		}));
	},

	/**
	 * Delete a folder. Workflows in it become uncategorized.
	 *
	 * @param folderId - The folder ID to delete
	 */
	async deleteFolder(folderId: string): Promise<void> {
		await invoke('delete_workflow_folder', { folderId });
		folderWritable.update((s) => {
			const expandedFolderIds = new Set(s.expandedFolderIds);
			expandedFolderIds.delete(folderId);
			persistExpandedIds(expandedFolderIds);
			return {
				...s,
				folders: s.folders.filter((f) => f.id !== folderId),
				expandedFolderIds
			};
		});
	},

	/**
	 * Reorder folders by providing ordered list of IDs.
	 *
	 * @param folderIds - Ordered list of folder IDs
	 */
	async reorderFolders(folderIds: string[]): Promise<void> {
		await invoke('reorder_workflow_folders', { folderIds });
		folderWritable.update((s) => {
			const reordered = folderIds
				.map((id, i) => {
					const folder = s.folders.find((f) => f.id === id);
					return folder ? { ...folder, sort_order: i } : null;
				})
				.filter((f): f is WorkflowFolder => f !== null);
			return { ...s, folders: reordered };
		});
	},

	/**
	 * Toggle a folder's expanded/collapsed state (local only).
	 *
	 * @param folderId - The folder ID to toggle
	 */
	toggleExpanded(folderId: string): void {
		folderWritable.update((s) => {
			const expandedFolderIds = new Set(s.expandedFolderIds);
			if (expandedFolderIds.has(folderId)) {
				expandedFolderIds.delete(folderId);
			} else {
				expandedFolderIds.add(folderId);
			}
			persistExpandedIds(expandedFolderIds);
			return { ...s, expandedFolderIds };
		});
	},

	/**
	 * Reset store to initial state.
	 */
	reset(): void {
		folderWritable.set(initialState);
	}
};

/**
 * Derived store: list of all folders
 */
export const folders = derived(folderWritable, ($s) => $s.folders);

/**
 * Derived store: loading state
 */
export const foldersLoading = derived(folderWritable, ($s) => $s.loading);

/**
 * Derived store: expanded folder IDs
 */
export const expandedFolderIds = derived(folderWritable, ($s) => $s.expandedFolderIds);
