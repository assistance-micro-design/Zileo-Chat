// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

/**
 * Kanban card and schedule types — mirror of Rust models in
 * `src-tauri/src/models/kanban_card.rs` and `kanban_schedule.rs`.
 */

import type { Message } from '$types/message';

/** UI column for kanban cards. */
export type KanbanColumn = 'todo' | 'doing' | 'review' | 'done';

/** Business status of a kanban card. */
export type KanbanCardStatus = 'todo' | 'ready' | 'doing' | 'review' | 'done' | 'failed';

/**
 * A kanban card persisted in DB.
 *
 * `prompt_id` and `inline_prompt` are mutually exclusive (XOR).
 * `variables` is a JSON-stringified `Record<string, string>` (ERR_SURREAL_001).
 */
export interface KanbanCard {
	id: string;
	title: string;
	description: string;
	kanban_agent_id: string;
	target_agent_id: string;
	prompt_id?: string;
	inline_prompt?: string;
	variables: string;
	target_folder_id?: string;
	status: KanbanCardStatus;
	column: KanbanColumn;
	column_order: number;
	workflow_id?: string;
	/**
	 * Workflow backing the in-place review chat with the Kanban agent.
	 * Distinct from `workflow_id` (the worker run). Absent until the user
	 * first opens the chat from the report viewer.
	 */
	review_chat_workflow_id?: string;
	error_summary?: string;
	created_at: string;
	updated_at: string;
}

/**
 * Init payload returned by `open_card_review_chat`: the (resumed or freshly
 * created) chat workflow id plus the conversation messages to render
 * (existing history, or the single seed assistant message on first open).
 */
export interface CardReviewChatInit {
	workflow_id: string;
	messages: Message[];
}

/** Payload to create a new kanban card. */
export interface KanbanCardCreate {
	/**
	 * Optional pre-generated card id. Set by `compose_card_from_description`
	 * so the persisted `kanban_card_interaction` row can be linked to the card
	 * created afterwards. When absent, the backend generates a fresh UUID.
	 */
	id?: string;
	title: string;
	description?: string;
	kanban_agent_id: string;
	target_agent_id: string;
	prompt_id?: string;
	inline_prompt?: string;
	variables?: string;
	target_folder_id?: string;
}

/** PATCH payload (tri-state where clearing is meaningful: `null` = clear, absent = keep). */
export interface KanbanCardUpdate {
	title?: string;
	description?: string;
	target_agent_id?: string;
	prompt_id?: string | null;
	inline_prompt?: string | null;
	variables?: string;
	target_folder_id?: string | null;
}

/** Recurrence rule for a kanban_card template. `days_of_week`: 0 = Mon, 6 = Sun. */
export interface KanbanSchedule {
	id: string;
	card_template_id: string;
	days_of_week: number[];
	hour: number;
	minute: number;
	next_run_at: string;
	last_run_at?: string;
	enabled: boolean;
	skip_if_pending: boolean;
	created_at: string;
}

export interface KanbanScheduleCreate {
	card_template_id: string;
	days_of_week: number[];
	hour: number;
	minute: number;
	skip_if_pending?: boolean;
}

export interface KanbanScheduleUpdate {
	days_of_week?: number[];
	hour?: number;
	minute?: number;
	enabled?: boolean | null;
	skip_if_pending?: boolean;
}
