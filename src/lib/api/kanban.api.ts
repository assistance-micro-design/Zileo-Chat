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
 * @fileoverview Typed Tauri API client for the Kanban domain.
 *
 * Covers Kanban cards, card interactions, the card review chat,
 * card composition and report analysis.
 *
 * @module lib/api/kanban
 */

import { invokeCommand } from './internal';
import type {
	AnalyzeReport,
	CardReviewChatInit,
	ComposeStartResponse,
	KanbanCard,
	KanbanCardCreate,
	KanbanCardUpdate,
	KanbanColumn
} from '$types/kanban';
import type { KanbanCardInteraction } from '$types/kanban_interaction';

// ============================================================================
// Cards
// ============================================================================

/** List Kanban cards, optionally filtered by Kanban agent. */
export async function listKanbanCards(kanbanAgentId?: string | null): Promise<KanbanCard[]> {
	return invokeCommand<KanbanCard[]>('list_kanban_cards', {
		kanbanAgentId: kanbanAgentId
	});
}

/** Get a single Kanban card. */
export async function getKanbanCard(cardId: string): Promise<KanbanCard> {
	return invokeCommand<KanbanCard>('get_kanban_card', { cardId });
}

/** Create a Kanban card, returning its ID. */
export async function createKanbanCard(config: KanbanCardCreate): Promise<string> {
	return invokeCommand<string>('create_kanban_card', { config });
}

/** Update a Kanban card. */
export async function updateKanbanCard(
	cardId: string,
	config: KanbanCardUpdate
): Promise<KanbanCard> {
	return invokeCommand<KanbanCard>('update_kanban_card', { cardId, config });
}

/** Delete a Kanban card, optionally deleting its schedule too. */
export async function deleteKanbanCard(
	cardId: string,
	alsoDeleteSchedule?: boolean | null
): Promise<void> {
	return invokeCommand<void>('delete_kanban_card', {
		cardId,
		alsoDeleteSchedule: alsoDeleteSchedule
	});
}

/** Move a Kanban card to another column / order. */
export async function moveKanbanCard(
	cardId: string,
	newColumn: KanbanColumn,
	newOrder: number
): Promise<KanbanCard> {
	return invokeCommand<KanbanCard>('move_kanban_card', { cardId, newColumn, newOrder });
}

/** Duplicate a card as a recurrence template. */
export async function duplicateKanbanCardAsTemplate(cardId: string): Promise<KanbanCard> {
	return invokeCommand<KanbanCard>('duplicate_kanban_card_as_template', { cardId });
}

/** Link a workflow to a Kanban card. */
export async function setKanbanCardWorkflowId(
	cardId: string,
	workflowId: string
): Promise<void> {
	return invokeCommand<void>('set_kanban_card_workflow_id', { cardId, workflowId });
}

// ============================================================================
// Interactions + review chat
// ============================================================================

/** Load the interaction history of a card. */
export async function loadCardInteractions(cardId: string): Promise<KanbanCardInteraction[]> {
	return invokeCommand<KanbanCardInteraction[]>('load_card_interactions', { cardId });
}

/** Open the review chat of a card. */
export async function openCardReviewChat(
	cardId: string,
	locale: string
): Promise<CardReviewChatInit> {
	return invokeCommand<CardReviewChatInit>('open_card_review_chat', { cardId, locale });
}

// ============================================================================
// Compose + analysis
// ============================================================================

/** Start a detached card composition. */
export async function startComposeCard(
	kanbanAgentId: string,
	description: string,
	locale: string
): Promise<ComposeStartResponse> {
	return invokeCommand<ComposeStartResponse>('start_compose_card', {
		kanbanAgentId,
		description,
		locale
	});
}

/** Approve a proposed card. */
export async function approveProposedCard(cardId: string): Promise<KanbanCard> {
	return invokeCommand<KanbanCard>('approve_proposed_card', { cardId });
}

/** Analyze the report of a review card. */
export async function analyzeCardReport(cardId: string): Promise<AnalyzeReport> {
	return invokeCommand<AnalyzeReport>('analyze_card_report', { cardId });
}
