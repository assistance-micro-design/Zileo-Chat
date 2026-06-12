<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  ToolCallBlock Component
  Collapsible block showing a tool call with input/output details.
-->

<script lang="ts">
	import { Wrench, ChevronDown, CircleCheckBig, CircleX, Server } from '@lucide/svelte';
	import { untrack } from 'svelte';
	import { i18n } from '$lib/i18n';
	import { formatDuration } from '$lib/utils/duration';

	interface Props {
		toolName: string;
		toolType: 'local' | 'mcp';
		serverName?: string;
		inputParams: string;
		outputResult: string;
		success: boolean;
		errorMessage?: string;
		durationMs: number;
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
		toolName,
		toolType,
		serverName,
		inputParams,
		outputResult,
		success,
		errorMessage,
		durationMs,
		collapsed = true,
		sequence,
		agentId,
		agentName,
		primaryAgentId
	}: Props = $props();

	const blockId = $derived(`tool-${sequence ?? 'tmp'}`);

	// A block is "sub-agent" when its agent_id is present AND different from
	// the workflow's primary agent. Missing primaryAgentId (legacy/replay
	// without registry hit) collapses to false so the block renders identical
	// to a primary one (no false-positive indentation).
	const isSubAgent = $derived(!!agentId && !!primaryAgentId && agentId !== primaryAgentId);
	const agentLabel = $derived(agentName ?? agentId?.slice(0, 8) ?? '');

	const formattedDuration = $derived(formatDuration(durationMs));

	const formattedInput = $derived(formatJson(inputParams));
	const formattedOutput = $derived(formatJson(outputResult));

	// Failed calls open on the output tab so the error is visible immediately;
	// successful calls open on the input tab (what the tool was asked to do).
	let activeTab = $state<'input' | 'output'>(untrack(() => success) ? 'input' : 'output');
	let inputTabRef = $state<HTMLButtonElement | null>(null);
	let outputTabRef = $state<HTMLButtonElement | null>(null);

	function selectTab(tab: 'input' | 'output'): void {
		activeTab = tab;
	}

	function handleTabKeydown(event: KeyboardEvent): void {
		if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
			event.preventDefault();
			activeTab = activeTab === 'input' ? 'output' : 'input';
			(activeTab === 'input' ? inputTabRef : outputTabRef)?.focus();
		}
	}

	function formatJson(raw: string): string {
		try {
			const parsed = JSON.parse(raw);
			return JSON.stringify(parsed, null, 2);
		} catch {
			return raw;
		}
	}

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
	class="tool-call-block"
	class:success
	class:error={!success}
	class:mcp-tool={toolType === 'mcp'}
	role="region"
	aria-label={isSubAgent
		? `${$i18n('block_tool_call_sub_agent_label')}: ${agentLabel} — ${toolName} — ${success ? $i18n('chat_tool_success') : $i18n('chat_tool_error')}`
		: `${toolName} - ${success ? $i18n('chat_tool_success') : $i18n('chat_tool_error')}`}
>
	<button
		class="tool-header"
		onclick={toggle}
		onkeydown={handleKeydown}
		aria-expanded={!collapsed}
		aria-controls={blockId}
		type="button"
	>
		<span class="blk-icon" aria-hidden="true">
			<Wrench size={13} />
		</span>
		{#if isSubAgent}
			<span class="agent-tag" title={agentLabel}>{agentLabel}</span>
		{/if}
		<span class="tool-name">{toolName}</span>

		{#if toolType === 'mcp' && serverName}
			<span class="mcp-badge">
				<Server size={10} />
				{serverName}
			</span>
		{/if}

		<span class="tool-status">
			{#if success}
				<CircleCheckBig size={14} class="status-success" />
			{:else}
				<CircleX size={14} class="status-error" />
			{/if}
		</span>

		<span class="tool-duration">{formattedDuration}</span>

		<ChevronDown size={14} class="chevron {collapsed ? '' : 'expanded'}" />
	</button>

	{#if !collapsed}
		<div class="tool-body" id={blockId}>
			<div class="io-tabs" role="tablist" aria-label={toolName}>
				<button
					bind:this={inputTabRef}
					type="button"
					class="io-tab"
					class:active={activeTab === 'input'}
					role="tab"
					aria-selected={activeTab === 'input'}
					aria-controls="{blockId}-panel"
					tabindex={activeTab === 'input' ? 0 : -1}
					onclick={() => selectTab('input')}
					onkeydown={handleTabKeydown}
				>
					{$i18n('chat_tool_input')}
				</button>
				<button
					bind:this={outputTabRef}
					type="button"
					class="io-tab"
					class:active={activeTab === 'output'}
					role="tab"
					aria-selected={activeTab === 'output'}
					aria-controls="{blockId}-panel"
					tabindex={activeTab === 'output' ? 0 : -1}
					onclick={() => selectTab('output')}
					onkeydown={handleTabKeydown}
				>
					{$i18n('chat_tool_output')}
				</button>
			</div>

			<div id="{blockId}-panel" role="tabpanel">
				{#if activeTab === 'input'}
					<pre class="tool-json">{formattedInput}</pre>
				{:else}
					<pre class="tool-json" class:error-text={!success}>{formattedOutput}</pre>
				{/if}
			</div>

			{#if errorMessage}
				<div class="tool-error">
					{errorMessage}
				</div>
			{/if}
		</div>
	{/if}
</div>

<style>
	/* The block publishes its channel (blue for local tools, magenta for MCP)
	   so the execution-thread rail in ChatContainer can tint its node. */
	.tool-call-block {
		--blk-channel: var(--channel-tool);
		--blk-channel-soft: var(--channel-tool-soft);
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-xs);
		margin: var(--spacing-xs) 0;
		overflow: hidden;
	}

	.tool-call-block.mcp-tool {
		--blk-channel: var(--channel-mcp);
		--blk-channel-soft: var(--channel-mcp-soft);
	}

	/* Failures keep a loud red rib: the status icon alone is too quiet for
	   an error that may need user action. Successes rely on the check icon. */
	.tool-call-block.error {
		border-left: 3px solid var(--color-error);
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

	.tool-header:hover .agent-tag {
		background: color-mix(in srgb, var(--channel-agent) 16%, transparent);
	}

	.tool-header {
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

	.tool-header:hover {
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

	.tool-name {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		flex-shrink: 0;
	}

	.mcp-badge {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		padding: 1px var(--spacing-xs);
		background: var(--blk-channel-soft);
		border: 1px solid currentColor;
		border-radius: var(--border-radius-full);
		font-size: var(--font-size-xs);
		color: var(--blk-channel);
	}

	.tool-status {
		display: flex;
		align-items: center;
		margin-left: auto;
	}

	.tool-status :global(.status-success) {
		color: var(--color-success);
	}

	.tool-status :global(.status-error) {
		color: var(--color-danger);
	}

	.tool-duration {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		flex-shrink: 0;
	}

	.tool-header :global(.chevron) {
		flex-shrink: 0;
		transition: transform 0.2s ease;
		color: var(--color-text-tertiary);
	}

	.tool-header :global(.chevron.expanded) {
		transform: rotate(180deg);
	}

	.tool-body {
		padding: var(--spacing-xs) var(--spacing-sm) var(--spacing-sm);
		border-top: 1px solid var(--color-border-light);
	}

	.io-tabs {
		display: flex;
		gap: 2px;
		margin-bottom: var(--spacing-xs);
	}

	.io-tab {
		padding: 0.2rem 0.7rem;
		font-size: var(--font-size-2xs);
		font-family: var(--font-family);
		font-weight: var(--font-weight-semibold);
		text-transform: uppercase;
		letter-spacing: 0.07em;
		color: var(--color-text-tertiary);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-sm) var(--border-radius-sm) 0 0;
		cursor: pointer;
	}

	.io-tab.active {
		background: var(--surface-2);
		color: var(--blk-channel, var(--color-text-primary));
	}

	.tool-json {
		font-family: var(--font-family-mono, monospace);
		font-size: var(--font-size-xs);
		line-height: 1.6;
		color: var(--color-text-primary);
		background: var(--surface-2);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-md);
		padding: 0.7rem 0.85rem;
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
		max-height: 300px;
		overflow-y: auto;
	}

	.tool-json.error-text {
		color: var(--color-error);
		border-color: var(--color-error-border);
	}

	.tool-error {
		font-size: var(--font-size-xs);
		color: var(--color-danger);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-danger-bg, rgba(239, 68, 68, 0.1));
		border-radius: var(--border-radius-sm);
	}
</style>
