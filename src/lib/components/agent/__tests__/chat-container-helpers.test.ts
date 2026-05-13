/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Tests for chat-container-helpers (pure functions).
 */

import { describe, it, expect } from 'vitest';
import { countInternalBlocks } from '../chat-container-helpers';
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

describe('countInternalBlocks', () => {
	it('returns 0 when sub-agent id is undefined', () => {
		expect(countInternalBlocks([], undefined)).toBe(0);
	});

	it('returns 0 for no internal blocks', () => {
		const blocks: ChatBlock[] = [tool('agent_primary', 0), subAgent('agent_sub', 1)];
		expect(countInternalBlocks(blocks, 'agent_sub')).toBe(0);
	});

	it('counts matching agent_id only', () => {
		const blocks: ChatBlock[] = [
			tool('agent_sub_001', 0),
			thinking('agent_sub_001', 1),
			tool('agent_primary', 2),
			subAgent('agent_sub_001', 3)
		];
		expect(countInternalBlocks(blocks, 'agent_sub_001')).toBe(2);
	});

	it('stops counting once the sub-agent summary block is reached', () => {
		// Blocks attributed to agent_sub_001 AFTER its own summary block
		// must NOT be counted — they belong to a later turn or another
		// branch of execution.
		const blocks: ChatBlock[] = [
			tool('agent_sub_001', 0),
			subAgent('agent_sub_001', 1),
			tool('agent_sub_001', 2)
		];
		expect(countInternalBlocks(blocks, 'agent_sub_001')).toBe(1);
	});

	it('ignores blocks from a different sub-agent', () => {
		const blocks: ChatBlock[] = [
			tool('agent_sub_other', 0),
			tool('agent_sub_001', 1),
			subAgent('agent_sub_001', 2)
		];
		expect(countInternalBlocks(blocks, 'agent_sub_001')).toBe(1);
	});
});
