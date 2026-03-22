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
 * Sidebar-related types for workflow management.
 * @module types/sidebar
 */

import type { WorkflowStatus } from './workflow';

/** Active status filter in the sidebar */
export type StatusFilter = 'all' | WorkflowStatus;

/** Context menu item for workflow actions */
export interface ContextMenuItem {
	/** Unique identifier for the action */
	id: string;
	/** i18n translation key for the label (used if label is not set) */
	labelKey?: string;
	/** Direct label text (takes precedence over labelKey) */
	label?: string;
	/** Lucide icon component */
	icon?: import('svelte').Component;
	/** Visual variant */
	variant?: 'default' | 'danger';
	/** Whether the item is disabled */
	disabled?: boolean;
	/** Show a visual separator before this item */
	separator?: boolean;
}
