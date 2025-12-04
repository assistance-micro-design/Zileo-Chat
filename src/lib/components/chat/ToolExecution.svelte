<!--
  ToolExecution Component
  Displays the status of a tool being executed by an agent.
  Shows tool name, status indicator, and execution duration.

  @example
  <ToolExecution tool="search_files" status="running" />
  <ToolExecution tool="database_query" status="completed" duration={1234} />
-->
<script lang="ts">
	import type { Status } from '$lib/components/ui/StatusIndicator.svelte';
	import StatusIndicator from '$lib/components/ui/StatusIndicator.svelte';
	import { Wrench, Clock } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Tool execution status
	 */
	export type ToolStatus = 'pending' | 'running' | 'completed' | 'error';

	/**
	 * ToolExecution props
	 */
	interface Props {
		/** Tool name or identifier */
		tool: string;
		/** Current execution status */
		status: ToolStatus;
		/** Execution duration in milliseconds (if completed) */
		duration?: number;
		/** Error message (if failed) */
		error?: string;
	}

	let { tool, status, duration, error }: Props = $props();

	/**
	 * Map tool status to StatusIndicator status
	 */
	const indicatorStatus = $derived(
		status === 'pending' ? 'idle' : status
	) as Status;

	/**
	 * Format duration for display
	 */
	function formatDuration(ms: number): string {
		if (ms < 1000) return `${ms}ms`;
		return `${(ms / 1000).toFixed(2)}s`;
	}

	/**
	 * Get status label for accessibility (i18n)
	 */
	const statusLabel = $derived(
		({
			pending: $i18n('chat_tool_pending'),
			running: $i18n('chat_tool_running'),
			completed: $i18n('chat_tool_completed'),
			error: $i18n('chat_tool_failed')
		})[status]
	);
</script>

<div class="tool-execution" class:error={status === 'error'}>
	<div class="tool-icon">
		<Wrench size={14} />
	</div>
	<div class="tool-info">
		<span class="tool-name">{tool}</span>
		{#if error}
			<span class="tool-error">{error}</span>
		{/if}
	</div>
	<div class="tool-status">
		{#if duration !== undefined && status === 'completed'}
			<span class="tool-duration">
				<Clock size={12} />
				{formatDuration(duration)}
			</span>
		{/if}
		<StatusIndicator status={indicatorStatus} size="sm" />
		<span class="status-label">{statusLabel}</span>
	</div>
</div>

<style>
	.tool-execution {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
	}

	.tool-execution.error {
		background: var(--color-error-light);
	}

	.tool-icon {
		color: var(--color-text-tertiary);
		display: flex;
		align-items: center;
	}

	.tool-info {
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.tool-name {
		font-family: var(--font-mono);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.tool-error {
		color: var(--color-error);
		font-size: var(--font-size-xs);
	}

	.tool-status {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-tertiary);
	}

	.tool-duration {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.status-label {
		min-width: 60px;
	}
</style>
