/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Tests for chat-container-helpers (pure functions).
 */

import { describe, it, expect } from 'vitest';
import { groupBlocksBySubAgent } from '../chat-container-helpers';
import type { ChatBlock } from '$types/chat-block';

function thinking(agent_id: string, sequence: number): ChatBlock {
	return {
		block_type: 'thinking',
		sequence,
		data: { content: 't', source: 'agent_flow', agent_id }
	};
}

function tool(agent_id: string, sequence: number): ChatBlock {
	return {
		block_type: 'tool_call',
		sequence,
		data: {
			tool_name: 'Tool',
			tool_type: 'local',
			input_params: '{}',
			output_result: '{}',
			success: true,
			duration_ms: 1,
			agent_id
		}
	};
}

function subAgent(sub_agent_id: string, sequence: number): ChatBlock {
	return {
		block_type: 'sub_agent',
		sequence,
		data: {
			agent_name: 'X',
			status: 'completed',
			_sub_agent_id: sub_agent_id
		}
	};
}

describe('groupBlocksBySubAgent', () => {
	it('returns an empty list for no blocks', () => {
		expect(groupBlocksBySubAgent([])).toEqual([]);
	});

	it('keeps every block top-level when there is no sub-agent summary', () => {
		const blocks: ChatBlock[] = [tool('agent_primary', 0), thinking('agent_primary', 1)];
		const groups = groupBlocksBySubAgent(blocks);
		expect(groups.map((g) => g.block)).toEqual(blocks);
		expect(groups.every((g) => g.internals.length === 0)).toBe(true);
	});

	it('nests internal blocks inside their sub-agent summary', () => {
		const inner1 = tool('agent_sub_001', 0);
		const inner2 = thinking('agent_sub_001', 1);
		const primary = tool('agent_primary', 2);
		const summary = subAgent('agent_sub_001', 3);
		const groups = groupBlocksBySubAgent([inner1, inner2, primary, summary]);
		expect(groups.map((g) => g.block)).toEqual([primary, summary]);
		expect(groups[1]!.internals).toEqual([inner1, inner2]);
	});

	it('keeps internal blocks top-level while their summary has not arrived yet (live streaming)', () => {
		const inner = tool('agent_sub_001', 0);
		const groups = groupBlocksBySubAgent([inner]);
		expect(groups.map((g) => g.block)).toEqual([inner]);
		expect(groups[0]!.internals).toEqual([]);
	});

	it('attaches each invocation of the same sub-agent to its own summary', () => {
		const run1 = tool('agent_sub_001', 0);
		const summary1 = subAgent('agent_sub_001', 1);
		const run2 = tool('agent_sub_001', 2);
		const summary2 = subAgent('agent_sub_001', 3);
		const groups = groupBlocksBySubAgent([run1, summary1, run2, summary2]);
		expect(groups.map((g) => g.block)).toEqual([summary1, summary2]);
		expect(groups[0]!.internals).toEqual([run1]);
		expect(groups[1]!.internals).toEqual([run2]);
	});

	it('falls back to the preceding summary when no later summary exists (live upsert)', () => {
		// During live streaming the sub_agent_complete chunk UPSERTS the
		// summary in place, so internals of a repeated invocation arrive
		// AFTER the (already rendered) summary. They must nest into that
		// preceding summary instead of rendering flat.
		const run1 = tool('agent_sub_001', 0);
		const summary = subAgent('agent_sub_001', 1);
		const lateInner = tool('agent_sub_001', 2);
		const groups = groupBlocksBySubAgent([run1, summary, lateInner]);
		expect(groups.map((g) => g.block)).toEqual([summary]);
		expect(groups[0]!.internals).toEqual([run1, lateInner]);
	});

	it('prefers the next summary over a preceding one', () => {
		// An internal block between two summaries of the same agent belongs
		// to the NEXT one (the stream emits the summary after its slice).
		const summary1 = subAgent('agent_sub_001', 0);
		const inner = tool('agent_sub_001', 1);
		const summary2 = subAgent('agent_sub_001', 2);
		const groups = groupBlocksBySubAgent([summary1, inner, summary2]);
		expect(groups.map((g) => g.block)).toEqual([summary1, summary2]);
		expect(groups[0]!.internals).toEqual([]);
		expect(groups[1]!.internals).toEqual([inner]);
	});

	it('does not attach blocks from a different sub-agent', () => {
		const other = tool('agent_sub_other', 0);
		const inner = tool('agent_sub_001', 1);
		const summary = subAgent('agent_sub_001', 2);
		const groups = groupBlocksBySubAgent([other, inner, summary]);
		expect(groups.map((g) => g.block)).toEqual([other, summary]);
		expect(groups[1]!.internals).toEqual([inner]);
	});

	it('preserves the timeline order of nested internals', () => {
		const a = thinking('agent_sub_001', 0);
		const b = tool('agent_sub_001', 1);
		const c = tool('agent_sub_001', 2);
		const summary = subAgent('agent_sub_001', 3);
		const groups = groupBlocksBySubAgent([a, b, c, summary]);
		expect(groups[0]!.internals).toEqual([a, b, c]);
	});

	it('handles a summary without _sub_agent_id (legacy rows) as a plain block', () => {
		const inner = tool('agent_sub_001', 0);
		const legacySummary: ChatBlock = {
			block_type: 'sub_agent',
			sequence: 1,
			data: { agent_name: 'X', status: 'completed' }
		};
		const groups = groupBlocksBySubAgent([inner, legacySummary]);
		expect(groups.map((g) => g.block)).toEqual([inner, legacySummary]);
		expect(groups[1]!.internals).toEqual([]);
	});
});
