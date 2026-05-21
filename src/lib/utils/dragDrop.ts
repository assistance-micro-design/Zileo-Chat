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
 * Drag & drop utilities for sidebar lists and board surfaces.
 * @module utils/dragDrop
 */

/** MIME type used for workflow drag data */
export const WORKFLOW_DRAG_TYPE = 'application/x-workflow-ids';

/** MIME type used for kanban card drag data */
export const KANBAN_CARD_MIME = 'application/x-kanban-card-ids';

/**
 * Extract workflow IDs from a drag event's data transfer.
 * @param event - The drag event to extract IDs from
 * @returns Array of workflow IDs, or null if data is invalid/missing
 */
export function getWorkflowIdsFromDrag(event: DragEvent): string[] | null {
	const data = event.dataTransfer?.getData(WORKFLOW_DRAG_TYPE);
	if (!data) return null;
	try {
		const ids: unknown = JSON.parse(data);
		if (Array.isArray(ids) && ids.every((id) => typeof id === 'string')) {
			return ids as string[];
		}
	} catch {
		// Invalid JSON in drag data
	}
	return null;
}

/**
 * Check if a drag event contains workflow data.
 * @param event - The drag event to check
 * @returns true if the event carries workflow IDs
 */
export function hasWorkflowDragData(event: DragEvent): boolean {
	return event.dataTransfer?.types.includes(WORKFLOW_DRAG_TYPE) ?? false;
}

/**
 * Serialise kanban card IDs into a drag event's data transfer.
 * @param event - The dragstart event
 * @param cardIds - One or more card IDs being dragged
 */
export function setCardDragData(event: DragEvent, cardIds: string[]): void {
	if (!event.dataTransfer) return;
	event.dataTransfer.setData(KANBAN_CARD_MIME, JSON.stringify(cardIds));
	event.dataTransfer.effectAllowed = 'move';
}

/**
 * Extract kanban card IDs from a drag event's data transfer.
 * @param event - The drag event to extract IDs from
 * @returns Array of card IDs, or null if data is invalid/missing
 */
export function getCardIdsFromDrag(event: DragEvent): string[] | null {
	const data = event.dataTransfer?.getData(KANBAN_CARD_MIME);
	if (!data) return null;
	try {
		const ids: unknown = JSON.parse(data);
		if (Array.isArray(ids) && ids.every((id) => typeof id === 'string')) {
			return ids as string[];
		}
	} catch {
		// Invalid JSON in drag data
	}
	return null;
}

/**
 * Check if a drag event contains kanban card data.
 * @param event - The drag event to check
 * @returns true if the event carries card IDs
 */
export function hasCardDragData(event: DragEvent): boolean {
	return event.dataTransfer?.types.includes(KANBAN_CARD_MIME) ?? false;
}
