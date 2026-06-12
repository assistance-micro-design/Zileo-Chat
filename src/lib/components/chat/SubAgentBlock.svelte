<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  SubAgentBlock Component
  Collapsible block showing sub-agent execution results.
-->

<script lang="ts">
	import { Users, ChevronDown, CircleCheckBig, CircleX } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { formatDuration } from '$lib/utils/duration';
	import { truncateThinkingContent } from '$types/thinking';

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
		 * sub-agent. Displayed as a count in the collapsed header so the user
		 * can preview the sub-agent's activity without expanding the block.
		 */
		internalBlockCount?: number;
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
		internalBlockCount = 0
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

	const preview = $derived(reportSummary ? truncateThinkingContent(reportSummary, 100) : null);

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

		{#if collapsed && internalBlockCount > 0}
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
			{#if tokensInput || tokensOutput}
				<div class="agent-tokens">
					{#if tokensInput}
						<span class="token-label"
							>{$i18n('chat_tokens_in')}: {tokensInput.toLocaleString()}</span
						>
					{/if}
					{#if tokensOutput}
						<span class="token-label"
							>{$i18n('chat_tokens_out')}: {tokensOutput.toLocaleString()}</span
						>
					{/if}
				</div>
			{/if}

			{#if hasCacheRow}
				<div class="agent-tokens agent-cache-row">
					{#if cachedTokens != null && cachedTokens > 0}
						<span class="token-label">cache: {cachedTokens.toLocaleString()}</span>
					{/if}
					{#if cacheWriteTokens != null && cacheWriteTokens > 0}
						<span class="token-label">+write: {cacheWriteTokens.toLocaleString()}</span>
					{/if}
					{#if thinkingTokens != null && thinkingTokens > 0}
						<span class="token-label">thinking: {thinkingTokens.toLocaleString()}</span>
					{/if}
				</div>
			{/if}

			{#if reportSummary}
				<div class="agent-report">
					{reportSummary}
				</div>
			{/if}
		</div>
	{:else if preview}
		<div class="sub-agent-preview">
			{preview}
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
		color: var(--blk-channel);
		padding: 2px 6px;
		background: var(--blk-channel-soft);
		border-radius: 4px;
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

	.agent-tokens {
		display: flex;
		gap: var(--spacing-md);
		margin-bottom: var(--spacing-xs);
	}

	.token-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.agent-report {
		font-size: var(--font-size-sm);
		line-height: 1.5;
		color: var(--color-text-primary);
		white-space: pre-wrap;
		word-break: break-word;
	}

	.sub-agent-preview {
		padding: 0 var(--spacing-sm) var(--spacing-xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-style: italic;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
