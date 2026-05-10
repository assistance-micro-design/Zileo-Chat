<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  SubAgentBlock Component
  Collapsible block showing sub-agent execution results.
-->

<script lang="ts">
	import { Users, ChevronDown, CheckCircle, XCircle } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { formatDuration } from '$lib/utils/duration';
	import { truncateThinkingContent } from '$types/thinking';

	interface Props {
		agentName: string;
		status: 'completed' | 'error';
		durationMs?: number;
		tokensInput?: number;
		tokensOutput?: number;
		reportSummary?: string;
		collapsed?: boolean;
		/** Stable block sequence used to derive a deterministic DOM id */
		sequence?: number;
	}

	let {
		agentName,
		status,
		durationMs,
		tokensInput,
		tokensOutput,
		reportSummary,
		collapsed = true,
		sequence
	}: Props = $props();

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
		<Users size={14} class="agent-icon" />
		<span class="agent-name">{agentName}</span>

		<span class="agent-status">
			{#if status === 'completed'}
				<CheckCircle size={14} class="status-success" />
			{:else}
				<XCircle size={14} class="status-error" />
			{/if}
		</span>

		{#if formattedDuration}
			<span class="agent-duration">{formattedDuration}</span>
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
	.sub-agent-block {
		border-radius: var(--border-radius-md);
		margin: var(--spacing-xs) 0;
		background: var(--color-bg-secondary);
		overflow: hidden;
	}

	.sub-agent-block.completed {
		border-left: 3px solid var(--color-info, var(--color-accent));
	}

	.sub-agent-block.error {
		border-left: 3px solid var(--color-danger);
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

	.sub-agent-header :global(.agent-icon) {
		flex-shrink: 0;
		color: var(--color-info, var(--color-accent));
	}

	.agent-name {
		font-weight: var(--font-weight-medium);
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

	.agent-duration {
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
		border-top: 1px solid var(--color-border);
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
