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
 * One renderable entry of the execution thread: a top-level block plus, for
 * sub-agent summary blocks, the internal blocks nested inside it.
 */
export interface BlockGroup {
	/** Block rendered at the top level of the thread */
	block: ChatBlock;
	/** For `sub_agent` blocks: internal blocks rendered nested inside it */
	internals: ChatBlock[];
}

/**
 * Group a linear block timeline so each sub-agent's internal blocks
 * (tool_call / thinking carrying its `agent_id`) nest inside that
 * sub-agent's summary block instead of rendering flat in the main thread.
 *
 * An internal block attaches to the NEXT summary with a matching
 * `_sub_agent_id` (the stream emits the summary after the sub-agent's
 * internal sequence), so repeated invocations of the same sub-agent each
 * collect their own slice. An internal block with no later summary falls
 * back to the last PRECEDING summary of the same agent (the live stream
 * upserts the summary in place, so a repeated invocation's internals can
 * arrive after it). Internal blocks with no summary at all (still
 * streaming) and primary-agent blocks stay top-level.
 *
 * @param blocks - Full ordered timeline of blocks (mixed primary + internals)
 * @returns Top-level groups in timeline order.
 */
export function groupBlocksBySubAgent(blocks: ChatBlock[]): BlockGroup[] {
	// Pass 1: attach each internal block index to the index of the next
	// summary carrying the same sub-agent id.
	const attachTo = new Map<number, number>();
	const pendingByAgent = new Map<string, number[]>();
	blocks.forEach((block, index) => {
		if (block.block_type === 'sub_agent') {
			const id = (block.data as SubAgentBlockData)._sub_agent_id;
			if (id) {
				for (const pendingIndex of pendingByAgent.get(id) ?? []) {
					attachTo.set(pendingIndex, index);
				}
				pendingByAgent.delete(id);
			}
			return;
		}
		const agentId = (block.data as ToolCallBlockData | ThinkingBlockData).agent_id;
		if (agentId) {
			const pending = pendingByAgent.get(agentId) ?? [];
			pending.push(index);
			pendingByAgent.set(agentId, pending);
		}
	});

	// Fallback: internals with no later summary attach to the last preceding
	// summary of the same agent (live upsert). Agents with no summary at all
	// keep their blocks top-level.
	for (const [agentId, indices] of pendingByAgent) {
		let summaryIndex = -1;
		blocks.forEach((block, index) => {
			if (
				block.block_type === 'sub_agent' &&
				(block.data as SubAgentBlockData)._sub_agent_id === agentId
			) {
				summaryIndex = index;
			}
		});
		if (summaryIndex === -1) continue;
		for (const blockIndex of indices) {
			attachTo.set(blockIndex, summaryIndex);
		}
	}

	// Pass 2: build groups in timeline order; attached blocks are skipped at
	// the top level and re-emitted inside their summary's `internals`.
	const internalsBySummary = new Map<number, ChatBlock[]>();
	for (const [blockIndex, summaryIndex] of attachTo) {
		const block = blocks[blockIndex];
		if (!block) continue;
		const internals = internalsBySummary.get(summaryIndex) ?? [];
		internals.push(block);
		internalsBySummary.set(summaryIndex, internals);
	}
	const groups: BlockGroup[] = [];
	blocks.forEach((block, index) => {
		if (attachTo.has(index)) return;
		groups.push({ block, internals: internalsBySummary.get(index) ?? [] });
	});
	return groups;
}
