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
 * Modal Controller Factory
 *
 * Creates a reusable modal controller with create/edit modes and proper state management.
 * Uses Svelte 5 runes for reactivity outside components.
 *
 * @module utils/modal
 *
 * @example
 * ```typescript
 * // In a .svelte file
 * const serverModal = createModalController<MCPServerConfig>();
 *
 * // Open for creation
 * serverModal.openCreate();
 *
 * // Open for editing
 * serverModal.openEdit(existingServer);
 *
 * // Close modal
 * serverModal.close();
 *
 * // Use in template
 * {#if serverModal.show}
 *   <Modal title={serverModal.mode === 'create' ? 'Add' : 'Edit'}>
 *     <Form data={serverModal.editing} />
 *   </Modal>
 * {/if}
 * ```
 */

/**
 * Modal mode type for create vs edit operations
 */
export type ModalMode = 'create' | 'edit';

/**
 * Modal controller interface with reactive getters
 */
export interface ModalController<T> {
	/** Whether modal is currently visible */
	readonly show: boolean;
	/** Current mode (create or edit) */
	readonly mode: ModalMode;
	/** Item being edited (undefined in create mode) */
	readonly editing: T | undefined;
	/** Opens modal in create mode */
	openCreate(): void;
	/** Opens modal in edit mode with the given item */
	openEdit(item: T): void;
	/** Closes modal and clears editing state */
	close(): void;
}

/**
 * Creates a modal controller with reactive state management.
 *
 * This factory function encapsulates the common pattern of managing
 * modal visibility, create/edit modes, and the item being edited.
 *
 * @template T - The type of item being created/edited
 * @returns A modal controller with reactive state
 */
export function createModalController<T>(): ModalController<T> {
	let show = $state(false);
	let mode = $state<ModalMode>('create');
	let editing = $state<T | undefined>(undefined);

	return {
		get show() {
			return show;
		},
		get mode() {
			return mode;
		},
		get editing() {
			return editing;
		},
		openCreate() {
			mode = 'create';
			editing = undefined;
			show = true;
		},
		openEdit(item: T) {
			mode = 'edit';
			editing = item;
			show = true;
		},
		close() {
			show = false;
			editing = undefined;
		}
	};
}
