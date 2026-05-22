// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

/**
 * Kanban meta-interaction types - mirror of Rust models in
 * `src-tauri/src/models/kanban_card_interaction.rs`.
 *
 * Each interaction is one `tool_loop::execute_with_tools` run done by the
 * Kanban agent during a `compose` (card creation) or `analyze` (report
 * grading) operation. Persisted for display in `KanbanCardReportViewer`.
 */

/** Kind of meta-operation persisted in `kanban_card_interaction`. */
export type InteractionKind = 'compose' | 'analyze';

/** A single tool call (local or MCP) executed during an iteration. */
export interface InteractionToolCall {
	tool_name: string;
	mcp_server?: string;
	/** JSON-stringified input payload. */
	input_json: string;
	/** JSON-stringified output result. */
	output_json: string;
	duration_ms: number;
	success: boolean;
}

/** One iteration of the tool loop: LLM call + tool calls + tokens + cost. */
export interface InteractionIteration {
	iteration_index: number;
	reasoning?: string;
	response_content?: string;
	tool_calls: InteractionToolCall[];
	tokens_input: number;
	tokens_output: number;
	cached_tokens: number;
	cost_usd: number;
	duration_ms: number;
}

/** Full meta-interaction (one compose or one analyze) persisted for a card. */
export interface KanbanCardInteraction {
	id: string;
	card_id: string;
	kind: InteractionKind;
	kanban_agent_id: string;
	provider: string;
	model_id_used: string;
	task_input: string;
	iterations: InteractionIteration[];
	final_payload_summary?: string;
	final_response_text?: string;
	total_tokens_input: number;
	total_tokens_output: number;
	total_cost_usd: number;
	created_at: string;
}
