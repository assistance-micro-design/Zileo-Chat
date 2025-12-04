<!--
  SubAgentActivity Component
  Displays sub-agent activity during workflow execution.
  Shows real-time status, progress, and results for spawned/delegated agents.

  Phase E: Streaming Events for Sub-Agent System

  @example
  <SubAgentActivity
    subAgents={activeSubAgents}
    isStreaming={isStreaming}
    collapsed={false}
  />
-->
<script lang="ts">
	import type { ActiveSubAgent, SubAgentStatus } from '$lib/stores/streaming';
	import {
		Bot,
		Loader2,
		CheckCircle2,
		XCircle,
		ChevronDown,
		ChevronUp,
		Clock,
		Cpu
	} from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * SubAgentActivity props
	 */
	interface Props {
		/** Active sub-agents from streaming store */
		subAgents?: ActiveSubAgent[];
		/** Whether streaming is active */
		isStreaming?: boolean;
		/** Whether panel is collapsed by default */
		collapsed?: boolean;
	}

	let { subAgents = [], isStreaming = false, collapsed = true }: Props = $props();

	/** Internal expanded state */
	let expanded = $state(!collapsed);

	/**
	 * Toggle panel expansion
	 */
	function toggleExpanded(): void {
		expanded = !expanded;
	}

	/**
	 * Get status CSS class
	 */
	function getStatusClass(status: SubAgentStatus): string {
		switch (status) {
			case 'completed':
				return 'status-success';
			case 'error':
				return 'status-error';
			case 'running':
				return 'status-running';
			default:
				return 'status-pending';
		}
	}

	/**
	 * Format duration in human-readable format
	 */
	function formatDuration(ms: number | undefined): string {
		if (ms === undefined) return '-';
		if (ms < 1000) return `${ms}ms`;
		if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`;
		return `${Math.floor(ms / 60000)}m ${Math.floor((ms % 60000) / 1000)}s`;
	}

	/**
	 * Get elapsed time for running sub-agents
	 */
	function getElapsedTime(startedAt: number): string {
		const elapsed = Date.now() - startedAt;
		return formatDuration(elapsed);
	}

	/**
	 * Count of running sub-agents
	 */
	const runningCount = $derived(subAgents.filter((a) => a.status === 'running').length);

	/**
	 * Count of completed sub-agents
	 */
	const completedCount = $derived(subAgents.filter((a) => a.status === 'completed').length);

	/**
	 * Count of failed sub-agents
	 */
	const errorCount = $derived(subAgents.filter((a) => a.status === 'error').length);

	/**
	 * Whether there are any sub-agents to show
	 */
	const hasSubAgents = $derived(subAgents.length > 0);
</script>

{#if hasSubAgents}
	<div class="sub-agent-panel" class:expanded>
		<!-- Header -->
		<button
			class="panel-header"
			onclick={toggleExpanded}
			aria-expanded={expanded}
			aria-controls="sub-agent-list"
		>
			<div class="header-left">
				<Cpu size={16} class="panel-icon" />
				<span class="panel-title">{$i18n('workflow_sub_agents_title')}</span>
				<span class="agent-count">{subAgents.length}</span>
				{#if runningCount > 0}
					<span class="count-badge running">{runningCount}</span>
				{/if}
				{#if completedCount > 0}
					<span class="count-badge success">{completedCount}</span>
				{/if}
				{#if errorCount > 0}
					<span class="count-badge error">{errorCount}</span>
				{/if}
			</div>
			<div class="header-right">
				{#if runningCount > 0 || isStreaming}
					<Loader2 size={14} class="streaming-indicator" />
				{/if}
				{#if expanded}
					<ChevronUp size={16} />
				{:else}
					<ChevronDown size={16} />
				{/if}
			</div>
		</button>

		<!-- Sub-Agent List -->
		{#if expanded}
			<div id="sub-agent-list" class="agent-list" role="list">
				{#each subAgents as agent (agent.id)}
					<div
						class="agent-item {getStatusClass(agent.status)}"
						role="listitem"
					>
						<div class="agent-status">
							{#if agent.status === 'running'}
								<Loader2 size={14} class="spinning" />
							{:else if agent.status === 'completed'}
								<CheckCircle2 size={14} />
							{:else if agent.status === 'error'}
								<XCircle size={14} />
							{:else}
								<Bot size={14} />
							{/if}
						</div>
						<div class="agent-details">
							<div class="agent-name">
								{agent.name}
								<span class="agent-id">{agent.id.slice(0, 8)}</span>
							</div>
							<div class="agent-task">{agent.taskDescription.slice(0, 100)}{agent.taskDescription.length > 100 ? '...' : ''}</div>

							<!-- Progress bar for running agents -->
							{#if agent.status === 'running' && agent.progress > 0}
								<div class="progress-container">
									<div class="progress-bar" style="width: {agent.progress}%"></div>
								</div>
							{/if}

							<!-- Status message -->
							{#if agent.statusMessage}
								<div class="agent-message">{agent.statusMessage}</div>
							{/if}

							<!-- Error message -->
							{#if agent.error}
								<div class="agent-error">{agent.error}</div>
							{/if}

							<!-- Metrics for completed agents -->
							{#if agent.status === 'completed' && agent.metrics}
								<div class="agent-metrics">
									<span class="metric">
										<Clock size={10} />
										{formatDuration(agent.metrics.duration_ms)}
									</span>
									<span class="metric">
										{$i18n('workflow_sub_agents_in_out').replace('{input}', String(agent.metrics.tokens_input)).replace('{output}', String(agent.metrics.tokens_output))}
									</span>
								</div>
							{/if}
						</div>
						<div class="agent-timing">
							{#if agent.duration !== undefined}
								{formatDuration(agent.duration)}
							{:else if agent.status === 'running'}
								{getElapsedTime(agent.startedAt)}
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style>
	.sub-agent-panel {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		overflow: hidden;
		transition: all var(--transition-base, 200ms) ease-out;
	}

	.sub-agent-panel.expanded {
		box-shadow: var(--shadow-sm);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		background: transparent;
		border: none;
		cursor: pointer;
		font: inherit;
		text-align: left;
		color: var(--color-text-primary);
	}

	.panel-header:hover {
		background: var(--color-bg-tertiary);
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.header-left :global(.panel-icon) {
		color: var(--color-accent);
	}

	.panel-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.agent-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.count-badge {
		font-size: var(--font-size-xs);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.count-badge.running {
		color: var(--color-accent);
		background: var(--color-accent-bg, rgba(99, 102, 241, 0.1));
	}

	.count-badge.success {
		color: var(--color-success);
		background: var(--color-success-bg, rgba(34, 197, 94, 0.1));
	}

	.count-badge.error {
		color: var(--color-error);
		background: var(--color-error-bg, rgba(239, 68, 68, 0.1));
	}

	.header-right {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-tertiary);
	}

	.header-right :global(.streaming-indicator) {
		animation: spin 1s linear infinite;
		color: var(--color-accent);
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.agent-list {
		max-height: 300px;
		overflow-y: auto;
		border-top: 1px solid var(--color-border);
		animation: slideDown 200ms ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			max-height: 0;
		}
		to {
			opacity: 1;
			max-height: 300px;
		}
	}

	.agent-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light, rgba(0, 0, 0, 0.05));
		animation: fadeInItem 150ms ease-out;
		transition: background-color var(--transition-fast, 150ms) ease;
	}

	@keyframes fadeInItem {
		from {
			opacity: 0;
			transform: translateX(-8px);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}

	.agent-item:last-child {
		border-bottom: none;
	}

	.agent-status {
		flex-shrink: 0;
		margin-top: 2px;
	}

	.agent-item.status-success .agent-status {
		color: var(--color-success);
	}

	.agent-item.status-error .agent-status {
		color: var(--color-error);
	}

	.agent-item.status-running .agent-status {
		color: var(--color-accent);
	}

	.agent-item.status-pending .agent-status {
		color: var(--color-text-tertiary);
	}

	.agent-item :global(.spinning) {
		animation: spin 1s linear infinite;
	}

	.agent-details {
		flex: 1;
		min-width: 0;
	}

	.agent-name {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.agent-id {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-family: var(--font-mono);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-sm);
	}

	.agent-task {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		margin-top: var(--spacing-xs);
		line-height: 1.4;
	}

	.progress-container {
		height: 4px;
		background: var(--color-bg-tertiary);
		border-radius: var(--radius-full);
		margin-top: var(--spacing-xs);
		overflow: hidden;
	}

	.progress-bar {
		height: 100%;
		background: var(--color-accent);
		border-radius: var(--radius-full);
		transition: width 300ms ease-out;
	}

	.agent-message {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		margin-top: var(--spacing-xs);
		font-style: italic;
	}

	.agent-error {
		font-size: var(--font-size-xs);
		color: var(--color-error);
		margin-top: var(--spacing-xs);
	}

	.agent-metrics {
		display: flex;
		gap: var(--spacing-md);
		margin-top: var(--spacing-xs);
	}

	.metric {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-family: var(--font-mono);
	}

	.agent-timing {
		flex-shrink: 0;
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
		min-width: 50px;
		text-align: right;
	}
</style>
