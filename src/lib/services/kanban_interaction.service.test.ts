/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriInvoke } from '$lib/tauri';
import { loadCardInteractions } from './kanban_interaction.service';
import type { KanbanCardInteraction } from '$types/kanban_interaction';

vi.mock('$lib/tauri', () => ({
	tauriInvoke: vi.fn()
}));

const invokeMock = vi.mocked(tauriInvoke);

function sampleInteraction(): KanbanCardInteraction {
	return {
		id: 'int-1',
		card_id: 'card-1',
		kind: 'compose',
		kanban_agent_id: 'agent-1',
		provider: 'mistral',
		model_id_used: 'mistral-medium-2505',
		task_input: 'demo',
		iterations: [
			{
				iteration_index: 1,
				reasoning: undefined,
				response_content: 'rationale',
				tool_calls: [
					{
						tool_name: 'SubmitComposedCard',
						input_json: '{"title":"x"}',
						output_json: '{"success":true}',
						duration_ms: 12,
						success: true
					}
				],
				tokens_input: 100,
				tokens_output: 50,
				cached_tokens: 0,
				cost_usd: 0.0015,
				duration_ms: 1200
			}
		],
		final_payload_summary: 'title: x',
		final_response_text: 'rationale',
		total_tokens_input: 100,
		total_tokens_output: 50,
		total_cost_usd: 0.0015,
		created_at: '2026-05-21T10:00:00Z'
	};
}

describe('loadCardInteractions', () => {
	beforeEach(() => {
		invokeMock.mockReset();
	});

	it('invokes load_card_interactions with camelCase cardId', async () => {
		invokeMock.mockResolvedValue([]);
		await loadCardInteractions('card-abc');
		expect(invokeMock).toHaveBeenCalledWith('load_card_interactions', { cardId: 'card-abc' });
	});

	it('returns the persisted interactions list', async () => {
		const fixture = sampleInteraction();
		invokeMock.mockResolvedValue([fixture]);
		const result = await loadCardInteractions('card-1');
		expect(result).toHaveLength(1);
		const [first] = result;
		expect(first?.id).toBe('int-1');
		expect(first?.iterations[0]?.tool_calls[0]?.tool_name).toBe('SubmitComposedCard');
	});

	it('returns an empty array for cards with no history', async () => {
		invokeMock.mockResolvedValue([]);
		const result = await loadCardInteractions('card-empty');
		expect(result).toEqual([]);
	});

	it('rewraps backend errors as a JS Error', async () => {
		invokeMock.mockRejectedValue('card_id must be a UUID');
		await expect(loadCardInteractions('not-a-uuid')).rejects.toThrow(/card_id/);
	});
});
