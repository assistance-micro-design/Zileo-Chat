<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  ThinkingBlock Component
  Collapsible block showing model thinking or agent flow reasoning.
-->

<script lang="ts">
	import { Brain, ChevronDown } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { truncateThinkingContent } from '$types/thinking';

	interface Props {
		content: string;
		source: 'model_thinking' | 'agent_flow';
		collapsed?: boolean;
		/** Stable block sequence used to derive a deterministic DOM id */
		sequence?: number;
		/** ID of the agent that produced this block (orchestrator or sub-agent) */
		agentId?: string;
		/** Display name of the agent that produced this block (best-effort) */
		agentName?: string;
		/** Workflow's primary agent id — used to compute `isSubAgent` */
		primaryAgentId?: string;
	}

	let {
		content,
		source,
		collapsed = true,
		sequence,
		agentId,
		agentName,
		primaryAgentId
	}: Props = $props();

	const blockId = $derived(`thinking-${sequence ?? 'tmp'}`);

	const preview = $derived(truncateThinkingContent(content, 80));

	// A block is "sub-agent" when its agent_id is present AND different from
	// the workflow's primary agent. Falsy primaryAgentId collapses to false
	// (legacy/replay without registry hit) so layout stays unchanged.
	const isSubAgent = $derived(!!agentId && !!primaryAgentId && agentId !== primaryAgentId);
	const agentLabel = $derived(agentName ?? agentId?.slice(0, 8) ?? '');

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
	class="thinking-block"
	class:model-thinking={source === 'model_thinking'}
	class:agent-flow={source === 'agent_flow'}
	role="region"
	aria-label={isSubAgent
		? `${$i18n('block_thinking_sub_agent_label')}: ${agentLabel}`
		: $i18n('chat_thinking_block_label')}
>
	<button
		class="thinking-header"
		onclick={toggle}
		onkeydown={handleKeydown}
		aria-expanded={!collapsed}
		aria-controls={blockId}
		type="button"
	>
		<span class="blk-icon" aria-hidden="true">
			<Brain size={source === 'model_thinking' ? 15 : 13} />
		</span>
		{#if isSubAgent}
			<span class="agent-tag" title={agentLabel}>{agentLabel}</span>
		{/if}
		<span class="thinking-title">
			{source === 'model_thinking' ? $i18n('chat_thinking_model') : $i18n('chat_thinking_agent')}
		</span>
		{#if collapsed}
			<span class="thinking-preview">{preview}</span>
		{/if}
		<ChevronDown size={14} class="chevron {collapsed ? '' : 'expanded'}" />
	</button>

	{#if !collapsed}
		<div class="thinking-body" id={blockId}>
			<pre class="thinking-content">{content}</pre>
		</div>
	{/if}
</div>

<style>
	/* The thinking channel (violet) is published on the block root so the
	   execution-thread rail in ChatContainer can tint this block's node. */
	.thinking-block {
		--blk-channel: var(--channel-thinking);
		--blk-channel-soft: var(--channel-thinking-soft);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-xs);
		margin: var(--spacing-xs) 0;
		overflow: hidden;
	}

	/* Agent-flow reasoning is quieter: no card chrome, just the channel tint */
	.thinking-block.agent-flow {
		background: transparent;
		border: none;
		box-shadow: none;
	}

	.agent-tag {
		display: inline-flex;
		align-items: center;
		padding: 2px 6px;
		font-size: var(--font-size-xs);
		color: var(--channel-agent);
		background: var(--channel-agent-soft);
		border-radius: 4px;
		flex-shrink: 0;
		max-width: 120px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.thinking-header:hover .agent-tag {
		background: color-mix(in srgb, var(--channel-agent) 16%, transparent);
	}

	.thinking-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		width: 100%;
		padding: var(--spacing-xs) var(--spacing-sm);
		background: none;
		border: none;
		cursor: pointer;
		color: var(--color-text-secondary);
		font-size: var(--font-size-xs);
		text-align: left;
		transition: background-color 0.15s ease;
	}

	.thinking-header:hover {
		background: var(--color-bg-hover);
	}

	.model-thinking .thinking-header {
		color: var(--color-text-primary);
		font-size: var(--font-size-sm);
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

	.agent-flow .blk-icon {
		width: 22px;
		height: 22px;
	}

	.thinking-title {
		font-weight: var(--font-weight-medium);
		flex-shrink: 0;
	}

	.thinking-preview {
		flex: 1;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	.thinking-header :global(.chevron) {
		flex-shrink: 0;
		transition: transform 0.2s ease;
	}

	.thinking-header :global(.chevron.expanded) {
		transform: rotate(180deg);
	}

	.thinking-body {
		padding: var(--spacing-xs) var(--spacing-sm) var(--spacing-sm);
		border-top: 1px solid var(--color-border-light);
		background: linear-gradient(180deg, var(--blk-channel-soft), transparent 38%);
	}

	.thinking-content {
		font-family: var(--font-family-mono, monospace);
		font-size: var(--font-size-xs);
		line-height: 1.65;
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
	}

	.model-thinking .thinking-content {
		color: var(--color-text-primary);
	}
</style>
