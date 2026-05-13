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
	const isSubAgent = $derived(
		!!agentId && !!primaryAgentId && agentId !== primaryAgentId
	);
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
	class:sub-agent={isSubAgent}
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
		<Brain size={source === 'model_thinking' ? 16 : 14} class="thinking-icon" />
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
	.thinking-block {
		border-radius: var(--border-radius-md);
		margin: var(--spacing-xs) 0;
		overflow: hidden;
	}

	.thinking-block.model-thinking {
		background: var(--color-bg-tertiary);
		border-left: 3px solid var(--color-accent);
	}

	.thinking-block.agent-flow {
		background: transparent;
		border-left: 2px solid var(--color-border);
	}

	/* Sub-agent visual treatment (spec 2026-05-12 section 'Décisions validées') */
	.thinking-block.sub-agent {
		margin-left: 16px;
		border-left-style: dashed;
		border-left-color: var(--color-info);
	}

	.agent-tag {
		display: inline-flex;
		align-items: center;
		padding: 2px 6px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
		background: color-mix(in srgb, var(--color-info) 10%, transparent);
		border-radius: 4px;
		flex-shrink: 0;
		max-width: 120px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.thinking-header:hover .agent-tag {
		background: color-mix(in srgb, var(--color-info) 16%, transparent);
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

	.thinking-header :global(.thinking-icon) {
		flex-shrink: 0;
	}

	.model-thinking .thinking-header :global(.thinking-icon) {
		color: var(--color-accent);
	}

	.agent-flow .thinking-header :global(.thinking-icon) {
		color: var(--color-text-tertiary);
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
	}

	.thinking-content {
		font-family: var(--font-family-mono, monospace);
		font-size: var(--font-size-xs);
		line-height: 1.5;
		color: var(--color-text-secondary);
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
	}

	.model-thinking .thinking-content {
		color: var(--color-text-primary);
	}
</style>
