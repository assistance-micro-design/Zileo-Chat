<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

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
	import { getActivityIcon } from '$lib/utils/activity-icons';
	import { formatDuration } from '$lib/utils/duration';
	import { formatTokenCount, formatAbsoluteTimestamp } from '$lib/utils/activity';
	import ActivityItemDetails from './ActivityItemDetails.svelte';
	import ReasoningDetailsPanel from './ReasoningDetailsPanel.svelte';
	import ToolDetailsPanel from './ToolDetailsPanel.svelte';
	import {
		Loader2,
		CheckCircle2,
		XCircle,
		Clock,
		ChevronDown,
		ChevronRight
	} from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

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

	/** Local state for task collapse */
	let isTaskExpanded = $state(false);

	/** Local state for reasoning collapse */
	let isReasoningExpanded = $state(false);

	/** Local state for tool details collapse */
	let isToolExpanded = $state(false);

	/** Check if this is a task with expandable details */
	const isTaskWithDetails = $derived(
		activity.type.startsWith('task_') &&
			(activity.description || activity.metadata?.agentAssigned || activity.metadata?.priority)
	);

	/** Check if this is a reasoning step with full content */
	const isReasoningWithContent = $derived(
		activity.type === 'reasoning' && !!activity.metadata?.content
	);

	/** Check if this is a tool with lazy-loadable details */
	const isToolWithDetails = $derived(
		activity.type.startsWith('tool_') && !!activity.metadata?.executionId
	);

	/** Whether this item is expandable at all */
	const isExpandable = $derived(isTaskWithDetails || isReasoningWithContent || isToolWithDetails);

	/** Whether the item is currently expanded */
	const isExpanded = $derived(
		(isTaskWithDetails && isTaskExpanded) ||
		(isReasoningWithContent && isReasoningExpanded) ||
		(isToolWithDetails && isToolExpanded)
	);

	/** Toggle expand based on item type */
	function handleToggle(): void {
		if (isTaskWithDetails) isTaskExpanded = !isTaskExpanded;
		else if (isReasoningWithContent) isReasoningExpanded = !isReasoningExpanded;
		else if (isToolWithDetails) isToolExpanded = !isToolExpanded;
	}

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
	 * Derived icon component based on activity type
	 */
	const IconComponent = $derived(getActivityIcon(activity.type));

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
		{#if isExpandable}
			<button
				class="expand-btn"
				onclick={handleToggle}
				aria-expanded={isExpanded}
				aria-label={isExpanded ? $i18n('workflow_activity_collapse') : $i18n('workflow_activity_expand')}
			>
				{#if isExpanded}
					<ChevronDown size={14} />
				{:else}
					<ChevronRight size={14} />
				{/if}
			</button>
		{:else}
			<IconComponent size={14} class="type-icon" />
		{/if}
	</div>
	<div class="item-content">
		<div class="item-title">
			{#if isExpandable}
				<IconComponent size={14} class="type-icon" />
			{/if}
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
		<!-- Task details collapse -->
		{#if isTaskWithDetails && isTaskExpanded}
			<ActivityItemDetails {activity} />
		{/if}
		<!-- Reasoning details collapse -->
		{#if isReasoningWithContent && isReasoningExpanded && activity.metadata?.content}
			<ReasoningDetailsPanel content={activity.metadata.content} />
		{/if}
		<!-- Tool details collapse (lazy-loaded) -->
		{#if isToolWithDetails && isToolExpanded && activity.metadata?.executionId}
			<ToolDetailsPanel executionId={activity.metadata.executionId} />
		{/if}
	</div>
	<div class="item-meta">
		{#if activity.metadata?.tokens}
			{@const totalTokens = activity.metadata.tokens.input + activity.metadata.tokens.output}
			{#if totalTokens > 0}
				<span class="token-badge">{formatTokenCount(totalTokens)} tok</span>
			{/if}
		{/if}
		<span class="item-duration" title={formatAbsoluteTimestamp(activity.timestamp)}>{formattedDuration}</span>
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

	.activity-item.task_create .item-icon :global(.type-icon) {
		color: var(--color-accent);
	}

	.activity-item.task_update .item-icon :global(.type-icon) {
		color: var(--color-warning);
	}

	.activity-item.task_complete .item-icon :global(.type-icon) {
		color: var(--color-success);
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
		gap: var(--spacing-xs);
		margin-top: 2px;
	}

	.token-badge {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-family: var(--font-mono);
		white-space: nowrap;
	}

	.item-duration {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
		min-width: 40px;
		text-align: right;
	}

	/* Task expand button */
	.expand-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 0;
		border: none;
		background: transparent;
		cursor: pointer;
		color: var(--color-text-secondary);
		border-radius: var(--radius-sm);
		transition: all var(--transition-fast, 150ms) ease;
	}

	.expand-btn:hover {
		color: var(--color-accent);
		background: var(--color-bg-tertiary);
	}

</style>
