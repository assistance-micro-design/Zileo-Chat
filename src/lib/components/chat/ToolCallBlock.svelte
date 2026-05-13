<!--
  Copyright 2025 Assistance Micro Design
  SPDX-License-Identifier: Apache-2.0

  ToolCallBlock Component
  Collapsible block showing a tool call with input/output details.
-->

<script lang="ts">
	import { Wrench, ChevronDown, CheckCircle, XCircle, Server } from '@lucide/svelte';
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
	const isSubAgent = $derived(
		!!agentId && !!primaryAgentId && agentId !== primaryAgentId
	);
	const agentLabel = $derived(agentName ?? agentId?.slice(0, 8) ?? '');

	const formattedDuration = $derived(formatDuration(durationMs));

	const formattedInput = $derived(formatJson(inputParams));
	const formattedOutput = $derived(formatJson(outputResult));

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
	class:sub-agent={isSubAgent}
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
		<Wrench size={14} class="tool-icon" />
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
				<CheckCircle size={14} class="status-success" />
			{:else}
				<XCircle size={14} class="status-error" />
			{/if}
		</span>

		<span class="tool-duration">{formattedDuration}</span>

		<ChevronDown size={14} class="chevron {collapsed ? '' : 'expanded'}" />
	</button>

	{#if !collapsed}
		<div class="tool-body" id={blockId}>
			<div class="tool-section">
				<span class="section-label">{$i18n('chat_tool_input')}</span>
				<pre class="tool-json">{formattedInput}</pre>
			</div>

			<div class="tool-section">
				<span class="section-label">{$i18n('chat_tool_output')}</span>
				<pre class="tool-json" class:error-text={!success}>{formattedOutput}</pre>
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
	.tool-call-block {
		border-radius: var(--border-radius-md);
		margin: var(--spacing-xs) 0;
		background: var(--color-bg-secondary);
		overflow: hidden;
	}

	.tool-call-block.success {
		border-left: 3px solid var(--color-success);
	}

	.tool-call-block.error {
		border-left: 3px solid var(--color-danger);
	}

	/* Sub-agent visual treatment (spec 2026-05-12 section 'Décisions validées') */
	.tool-call-block.sub-agent {
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

	.tool-header:hover .agent-tag {
		background: color-mix(in srgb, var(--color-info) 16%, transparent);
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

	.tool-header :global(.tool-icon) {
		flex-shrink: 0;
		color: var(--color-text-secondary);
	}

	.tool-name {
		font-weight: var(--font-weight-medium);
		flex-shrink: 0;
	}

	.mcp-badge {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		padding: 1px var(--spacing-xs);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
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
		border-top: 1px solid var(--color-border);
	}

	.tool-section {
		margin-bottom: var(--spacing-sm);
	}

	.tool-section:last-child {
		margin-bottom: 0;
	}

	.section-label {
		display: block;
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-xs);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.tool-json {
		font-family: var(--font-family-mono, monospace);
		font-size: var(--font-size-xs);
		line-height: 1.5;
		color: var(--color-text-primary);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
		white-space: pre-wrap;
		word-break: break-word;
		margin: 0;
		max-height: 300px;
		overflow-y: auto;
	}

	.tool-json.error-text {
		color: var(--color-danger);
	}

	.tool-error {
		font-size: var(--font-size-xs);
		color: var(--color-danger);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-danger-bg, rgba(239, 68, 68, 0.1));
		border-radius: var(--border-radius-sm);
	}
</style>
