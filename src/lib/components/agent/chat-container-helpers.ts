/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Pure helpers for `ChatContainer.svelte`.
 *
 * Extracted so they can be unit-tested without mounting Svelte components.
 */

import type {
	ChatBlock,
	ToolCallBlockData,
	ThinkingBlockData,
	SubAgentBlockData
} from '$types/chat-block';

/**
 * Count the internal blocks (tool_call / thinking) that originate from a
 * given sub-agent, looking only at blocks that appear BEFORE the sub-agent's
 * own summary block in the timeline (matches the linear stream order — the
 * summary fires after the sub-agent finishes its internal sequence).
 *
 * @param blocks - Full ordered timeline of blocks (mixed primary + internals)
 * @param subAgentId - The sub-agent's agent_id to match against
 *   `block.data.agent_id`. Pass the `_sub_agent_id` carried on the
 *   SubAgentBlockData (= the actual agent_id of the delegated agent).
 * @returns Number of internal blocks attributable to this sub-agent.
 */
export function countInternalBlocks(blocks: ChatBlock[], subAgentId: string | undefined): number {
	if (!subAgentId) return 0;
	let count = 0;
	for (const b of blocks) {
		if (b.block_type === 'sub_agent') {
			const data = b.data as SubAgentBlockData;
			if (data._sub_agent_id === subAgentId) {
				// Reached the summary block of this sub-agent — stop counting
				// (any block past this point belongs to another sub-agent or
				// the primary agent's subsequent turn).
				break;
			}
			continue;
		}
		const data = b.data as ToolCallBlockData | ThinkingBlockData;
		if (data.agent_id === subAgentId) {
			count += 1;
		}
	}
	return count;
}
