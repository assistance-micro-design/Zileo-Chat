<!--
  ActivityItem Component
  Displays a single activity event in the workflow activity feed.
  Shows event type, status, duration, and expandable details.

  Phase E: Unified Activity Timeline

  @example
  <ActivityItem
    activity={activityEvent}
    expanded={false}
  />
-->
<script lang="ts">
	import type { WorkflowActivityEvent, ActivityStatus } from '$types/activity';
	import {
		Wrench,
		Brain,
		Bot,
		ShieldCheck,
		Activity,
		Loader2,
		CheckCircle2,
		XCircle,
		Clock
	} from 'lucide-svelte';

	/**
	 * ActivityItem props
	 */
	interface Props {
		/** Activity event to display */
		activity: WorkflowActivityEvent;
		/** Whether to show expanded details */
		expanded?: boolean;
	}

	let { activity, expanded = false }: Props = $props();

	/**
	 * Get status CSS class for styling
	 */
	function getStatusClass(status: ActivityStatus): string {
		switch (status) {
			case 'completed':
				return 'status-completed';
			case 'error':
				return 'status-error';
			case 'running':
				return 'status-running';
			default:
				return 'status-pending';
		}
	}

	/**
	 * Get icon component based on activity type
	 */
	function getIconComponent(type: string) {
		if (type.startsWith('tool_')) return Wrench;
		if (type === 'reasoning') return Brain;
		if (type.startsWith('sub_agent_')) return Bot;
		if (type === 'validation') return ShieldCheck;
		return Activity;
	}

	/**
	 * Get status icon component
	 */
	function getStatusIcon(status: ActivityStatus) {
		switch (status) {
			case 'completed':
				return CheckCircle2;
			case 'error':
				return XCircle;
			case 'running':
				return Loader2;
			default:
				return Clock;
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
	 * Derived icon component based on activity type
	 */
	const IconComponent = $derived(getIconComponent(activity.type));

	/**
	 * Derived status icon based on activity status
	 */
	const StatusIcon = $derived(getStatusIcon(activity.status));

	/**
	 * Derived status class
	 */
	const statusClass = $derived(getStatusClass(activity.status));

	/**
	 * Derived formatted duration
	 */
	const formattedDuration = $derived(formatDuration(activity.duration));
</script>

<div class="activity-item {statusClass}" role="listitem">
	<div class="item-icon">
		<IconComponent size={14} class="type-icon" />
	</div>
	<div class="item-content">
		<div class="item-title">
			{activity.title}
			<span class="status-icon-wrapper" class:spinning={activity.status === 'running'}>
				<StatusIcon size={12} class="status-icon" />
			</span>
		</div>
		{#if activity.description && expanded}
			<div class="item-description">{activity.description}</div>
		{/if}
		{#if activity.metadata?.error}
			<div class="item-error">{activity.metadata.error}</div>
		{/if}
	</div>
	<div class="item-meta">
		<span class="item-duration">{formattedDuration}</span>
	</div>
</div>

<style>
	.activity-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
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

	.activity-item:last-child {
		border-bottom: none;
	}

	.activity-item:hover {
		background: var(--color-bg-tertiary);
	}

	.item-icon {
		flex-shrink: 0;
		margin-top: 2px;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.activity-item.status-pending .item-icon :global(.type-icon) {
		color: var(--color-text-tertiary);
	}

	.activity-item.status-running .item-icon :global(.type-icon) {
		color: var(--color-accent);
	}

	.activity-item.status-completed .item-icon :global(.type-icon) {
		color: var(--color-success);
	}

	.activity-item.status-error .item-icon :global(.type-icon) {
		color: var(--color-error);
	}

	.item-content {
		flex: 1;
		min-width: 0;
	}

	.item-title {
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		line-height: 1.4;
	}

	.status-icon-wrapper {
		flex-shrink: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		opacity: 0.6;
	}

	.status-icon-wrapper.spinning {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	.activity-item.status-pending .status-icon-wrapper :global(.status-icon) {
		color: var(--color-text-tertiary);
	}

	.activity-item.status-running .status-icon-wrapper :global(.status-icon) {
		color: var(--color-accent);
	}

	.activity-item.status-completed .status-icon-wrapper :global(.status-icon) {
		color: var(--color-success);
	}

	.activity-item.status-error .status-icon-wrapper :global(.status-icon) {
		color: var(--color-error);
	}

	.item-description {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		margin-top: var(--spacing-xs);
		line-height: 1.4;
	}

	.item-error {
		font-size: var(--font-size-xs);
		color: var(--color-error);
		margin-top: var(--spacing-xs);
		line-height: 1.4;
	}

	.item-meta {
		flex-shrink: 0;
		display: flex;
		align-items: flex-start;
		margin-top: 2px;
	}

	.item-duration {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
		min-width: 40px;
		text-align: right;
	}
</style>
