<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  SubAgentBlock Component
  Collapsible block showing sub-agent execution results.
-->

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Users, ChevronDown, CircleCheckBig, CircleX, ArrowRightLeft } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { formatDuration } from '$lib/utils/duration';

	interface Props {
		agentName: string;
		status: 'completed' | 'error';
		durationMs?: number;
		tokensInput?: number;
		tokensOutput?: number;
		/**
		 * USD cost computed with the sub-agent's OWN pricing. Sourced from the
		 * live `sub_agent_complete` chunk on stream and from
		 * `sub_agent_execution.cost_usd` on replay. Absent when the model has
		 * no pricing row; the cost row is hidden in that case.
		 */
		costUsd?: number;
		/**
		 * Cached prompt tokens (cache reads). Live source:
		 * `sub_agent_complete.metrics.cached_tokens`. Replay source:
		 * `merge_into_chat_blocks` projection of `sub_agent_execution.cached_tokens`.
		 * `null`/absent hides the row.
		 */
		cachedTokens?: number | null;
		/** Cache-write prompt tokens. Same source contract as `cachedTokens`. */
		cacheWriteTokens?: number | null;
		/** Thinking/reasoning tokens (reasoning models). Same contract as `cachedTokens`. */
		thinkingTokens?: number | null;
		reportSummary?: string;
		collapsed?: boolean;
		/** Stable block sequence used to derive a deterministic DOM id */
		sequence?: number;
		/**
		 * Number of internal blocks (tool_call/thinking) attributable to this
		 * sub-agent. Displayed as a count in the header so the user can gauge
		 * the sub-agent's activity without expanding the block.
		 */
		internalBlockCount?: number;
		/**
		 * Internal execution thread (nested ThinkingBlock/ToolCallBlock
		 * components) rendered inside the expanded body, hidden on collapse.
		 */
		children?: Snippet;
	}

	let {
		agentName,
		status,
		durationMs,
		tokensInput,
		tokensOutput,
		costUsd,
		cachedTokens,
		cacheWriteTokens,
		thinkingTokens,
		reportSummary,
		collapsed = true,
		sequence,
		internalBlockCount = 0,
		children
	}: Props = $props();

	const hasCacheRow = $derived(
		(cachedTokens != null && cachedTokens > 0) ||
			(cacheWriteTokens != null && cacheWriteTokens > 0) ||
			(thinkingTokens != null && thinkingTokens > 0)
	);

	const formattedCost = $derived(
		costUsd && costUsd > 0 ? `$${costUsd < 0.01 ? costUsd.toFixed(4) : costUsd.toFixed(2)}` : null
	);

	const blockId = $derived(`subagent-${sequence ?? 'tmp'}`);

	const formattedDuration = $derived(durationMs ? formatDuration(durationMs) : null);

	function toggle(): void {
		collapsed = !collapsed;
	}

	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Enter' || event.key === ' ') {
			event.preventDefault();
			toggle();
		}
	}
</script>

<div
	class="sub-agent-block"
	class:completed={status === 'completed'}
	class:error={status === 'error'}
	role="region"
	aria-label="{agentName} - {status}"
>
	<button
		class="sub-agent-header"
		onclick={toggle}
		onkeydown={handleKeydown}
		aria-expanded={!collapsed}
		aria-controls={blockId}
		type="button"
	>
		<span class="blk-icon" aria-hidden="true">
			<Users size={13} />
		</span>
		<span class="agent-name">{agentName}</span>

		{#if internalBlockCount > 0}
			<span class="internal-count">
				{internalBlockCount === 1
					? $i18n('sub_agent_block_internal_actions_count_one').replace('{count}', '1')
					: $i18n('sub_agent_block_internal_actions_count_other').replace(
							'{count}',
							String(internalBlockCount)
						)}
			</span>
		{/if}

		<span class="agent-status">
			{#if status === 'completed'}
				<CircleCheckBig size={14} class="status-success" />
			{:else}
				<CircleX size={14} class="status-error" />
			{/if}
		</span>

		{#if formattedDuration}
			<span class="agent-duration">{formattedDuration}</span>
		{/if}

		{#if formattedCost}
			<span class="agent-cost">{formattedCost}</span>
		{/if}

		<ChevronDown size={14} class="chevron {collapsed ? '' : 'expanded'}" />
	</button>

	{#if !collapsed}
		<div class="sub-agent-body" id={blockId}>
			{#if tokensInput || tokensOutput || hasCacheRow}
				<div class="metric-chips">
					{#if tokensInput}
						<span class="metric-chip">
							<ArrowRightLeft size={12} aria-hidden="true" />
							{$i18n('chat_tokens_in')}
							{tokensInput.toLocaleString()}
						</span>
					{/if}
					{#if tokensOutput}
						<span class="metric-chip">
							<ArrowRightLeft size={12} aria-hidden="true" />
							{$i18n('chat_tokens_out')}
							{tokensOutput.toLocaleString()}
						</span>
					{/if}
					{#if cachedTokens != null && cachedTokens > 0}
						<span class="metric-chip"
							>{$i18n('chat_tokens_cache')} {cachedTokens.toLocaleString()}</span
						>
					{/if}
					{#if cacheWriteTokens != null && cacheWriteTokens > 0}
						<span class="metric-chip"
							>{$i18n('chat_tokens_cache_write')} {cacheWriteTokens.toLocaleString()}</span
						>
					{/if}
					{#if thinkingTokens != null && thinkingTokens > 0}
						<span class="metric-chip"
							>{$i18n('chat_tokens_thinking')} {thinkingTokens.toLocaleString()}</span
						>
					{/if}
				</div>
			{/if}

			{#if reportSummary}
				<div class="agent-report">
					{reportSummary}
				</div>
			{/if}

			{#if children && internalBlockCount > 0}
				<div class="sub-agent-thread">
					{@render children()}
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	/* The sub-agent channel (brand orange) is published on the block root so
	   the execution-thread rail in ChatContainer can tint this block's node. */
	.sub-agent-block {
		--blk-channel: var(--channel-agent);
		--blk-channel-soft: var(--channel-agent-soft);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-xs);
		margin: var(--spacing-xs) 0;
		overflow: hidden;
	}

	/* Failures keep a loud red rib; success relies on the check icon. */
	.sub-agent-block.error {
		border-left: 3px solid var(--color-error);
	}

	.sub-agent-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		width: 100%;
		padding: var(--spacing-xs) var(--spacing-sm);
		background: none;
		border: none;
		cursor: pointer;
		color: var(--color-text-primary);
		font-size: var(--font-size-sm);
		text-align: left;
		transition: background-color 0.15s ease;
	}

	.sub-agent-header:hover {
		background: var(--color-bg-hover);
	}

	/* Tinted icon pill carrying the block's channel color */
	.blk-icon {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 26px;
		height: 26px;
		border-radius: 8px;
		background: var(--blk-channel-soft);
		color: var(--blk-channel);
		flex-shrink: 0;
	}

	.agent-name {
		font-weight: var(--font-weight-medium);
		flex-shrink: 0;
	}

	.internal-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		flex-shrink: 0;
	}

	.agent-status {
		display: flex;
		align-items: center;
		margin-left: auto;
	}

	.agent-status :global(.status-success) {
		color: var(--color-success);
	}

	.agent-status :global(.status-error) {
		color: var(--color-danger);
	}

	.agent-duration,
	.agent-cost {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		flex-shrink: 0;
	}

	.sub-agent-header :global(.chevron) {
		flex-shrink: 0;
		transition: transform 0.2s ease;
		color: var(--color-text-tertiary);
	}

	.sub-agent-header :global(.chevron.expanded) {
		transform: rotate(180deg);
	}

	.sub-agent-body {
		padding: var(--spacing-xs) var(--spacing-sm) var(--spacing-sm);
		border-top: 1px solid var(--color-border-light);
	}

	/* Pill chips matching the execution-thread metric chips */
	.metric-chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
		margin-bottom: var(--spacing-sm);
	}

	.metric-chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 0.14rem 0.5rem;
		font-size: var(--font-size-2xs);
		font-variant-numeric: tabular-nums;
		color: var(--color-text-secondary);
		background: var(--surface-2);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-full);
	}

	.agent-report {
		font-size: var(--font-size-sm);
		line-height: 1.5;
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		word-break: break-word;
	}

	/*
	  Internal execution thread: same rail-and-nodes pattern as the main
	  thread in ChatContainer, but on the sub-agent (orange) channel fading
	  out. Each nested block publishes --blk-channel on its root, which the
	  node pseudo-element reads.
	*/
	.sub-agent-thread {
		position: relative;
		margin-top: var(--spacing-sm);
		padding-left: 26px;
	}

	.sub-agent-thread::before {
		content: '';
		position: absolute;
		left: 9px;
		top: 6px;
		bottom: 6px;
		width: 2px;
		border-radius: 2px;
		background: linear-gradient(180deg, var(--channel-agent), transparent);
		opacity: 0.5;
	}

	.sub-agent-thread > :global(*) {
		position: relative;
	}

	.sub-agent-thread > :global(*)::before {
		content: '';
		position: absolute;
		left: -21px;
		top: 14px;
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--blk-channel, var(--color-text-tertiary));
		box-shadow:
			0 0 0 3px var(--blk-channel-soft, transparent),
			0 0 10px var(--blk-channel, transparent);
	}
</style>
