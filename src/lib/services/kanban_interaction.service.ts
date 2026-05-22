// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

/**
 * @fileoverview Service for loading kanban meta-interaction history.
 *
 * Wraps the `load_card_interactions` Tauri command. Used by
 * `KanbanCardReportViewer` to render the compose / analyze history of a card.
 *
 * @module lib/services/kanban_interaction
 */

import { tauriInvoke as invoke } from '$lib/tauri';
import type { KanbanCardInteraction } from '$types/kanban_interaction';
import { getErrorMessage } from '$lib/utils/error';

/**
 * Loads all persisted meta-interactions for a card (compose + analyze, in
 * chronological order). Returns an empty array for cards predating the
 * feature.
 *
 * @param cardId - UUID of the kanban card
 * @returns chronological list of interactions
 * @throws when the IPC call fails or the card_id is malformed
 */
export async function loadCardInteractions(cardId: string): Promise<KanbanCardInteraction[]> {
	try {
		return await invoke<KanbanCardInteraction[]>('load_card_interactions', {
			cardId
		});
	} catch (e) {
		throw new Error(getErrorMessage(e));
	}
}
