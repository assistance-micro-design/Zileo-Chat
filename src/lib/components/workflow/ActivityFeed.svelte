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
  ActivityFeed Component
  Displays a filterable feed of workflow activities in the right sidebar.
  Shows tool executions, sub-agent activity, and reasoning steps with status filters.

  Phase E: Unified Activity Timeline
  OPT-FA-13: Memoized Activity Filtering - uses store-level filtering for single source of truth
  OPT-MSG-5: Virtual scroll for large activity lists (20+ items) using @humanspeak/svelte-virtual-list
             - 90% DOM node reduction for 100+ activities
             - Dynamic height handling (no fixed item height required)
             - Falls back to standard {#each} for small lists

  @example
  <ActivityFeed
    activities={$filteredActivities}
    allActivities={$allActivities}
    isStreaming={true}
    filter="all"
    onFilterChange={(f) => activityStore.setFilter(f)}
    collapsed={false}
  />
-->
<script lang="ts">
	import type { Component } from 'svelte';
	import type { WorkflowActivityEvent, ActivityFilter } from '$types/activity';
	import { ACTIVITY_FILTERS } from '$types/activity';
	import { countActivitiesByType } from '$lib/utils/activity';
	import ActivityItem from './ActivityItem.svelte';
	import { Activity, Wrench, Bot, Brain, ListTodo, Loader2 } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import SvelteVirtualList from '@humanspeak/svelte-virtual-list';

	/**
	 * Icon component mapping for filters
	 */
	const FILTER_ICONS: Record<string, Component<{ size?: number; class?: string }>> = {
		Activity: Activity,
		Wrench: Wrench,
		Bot: Bot,
		Brain: Brain,
		ListTodo: ListTodo
	};

	/**
	 * ActivityFeed props
	 *
	 * OPT-FA-13: Memoized activity filtering
	 * - `activities`: Pre-filtered activities for display (from store's filteredActivities)
	 * - `allActivities`: Unfiltered activities for counts (from store's allActivities)
	 */
	interface Props {
		/** Pre-filtered activity events for display */
		activities?: WorkflowActivityEvent[];
		/** All activity events for counts (unfiltered) */
		allActivities?: WorkflowActivityEvent[];
		/** Whether streaming is currently active */
		isStreaming?: boolean;
		/** Current activity filter */
		filter?: ActivityFilter;
		/** Filter change callback */
		onFilterChange?: (filter: ActivityFilter) => void;
		/** Whether sidebar is collapsed */
		collapsed?: boolean;
	}

	let {
		activities = [],
		allActivities = [],
		isStreaming = false,
		filter = 'all',
		onFilterChange,
		collapsed = false
	}: Props = $props();

	/**
	 * Handle filter change
	 */
	function handleFilterChange(newFilter: ActivityFilter): void {
		onFilterChange?.(newFilter);
	}

	/**
	 * Activity counts by type (uses unfiltered allActivities for accurate counts)
	 * OPT-FA-13: Centralized filtering means counts always reflect total activities
	 */
	const counts = $derived(countActivitiesByType(allActivities));

	/**
	 * Whether to show empty state (uses pre-filtered activities)
	 */
	const showEmptyState = $derived(activities.length === 0);

	/**
	 * Minimum items threshold for virtual scrolling
	 * Below this threshold, regular rendering is more efficient
	 * OPT-MSG-5: Virtual scroll for large activity lists
	 */
	const VIRTUAL_SCROLL_THRESHOLD = 20;

	/**
	 * Whether to use virtual scrolling based on activity count
	 */
	const useVirtualScroll = $derived(activities.length >= VIRTUAL_SCROLL_THRESHOLD);
</script>

<div class="activity-feed" class:collapsed>
	<!-- Filter Tabs -->
	<div class="filter-tabs" role="tablist" aria-label="Activity filters">
		{#each ACTIVITY_FILTERS as f}
			{@const IconComponent = FILTER_ICONS[f.icon]}
			<button
				role="tab"
				class="filter-tab"
				class:active={filter === f.id}
				aria-selected={filter === f.id}
				aria-controls="activity-list"
				aria-label={f.label}
				title="{f.label} ({counts[f.id]})"
				onclick={() => handleFilterChange(f.id)}
			>
				<IconComponent size={16} />
			</button>
		{/each}
	</div>

	<!-- Activity List -->
	{#if !collapsed}
		<div id="activity-list" class="activity-list" role="list">
			{#if showEmptyState}
				<div class="empty-state">
					{#if isStreaming}
						<Loader2 class="spinning" size={20} />
						<span>{$i18n('workflow_activity_waiting')}</span>
					{:else}
						<Activity size={20} />
						<span>{$i18n('workflow_activity_none')}</span>
					{/if}
				</div>
			{:else if useVirtualScroll}
				<!-- OPT-MSG-5: Virtual scroll for 20+ activities (90% DOM reduction) -->
				<div class="virtual-list-container">
					<SvelteVirtualList
						items={activities}
						defaultEstimatedItemHeight={72}
						bufferSize={10}
						containerClass="virtual-list-outer"
						viewportClass="virtual-list-viewport"
						contentClass="virtual-list-content"
						itemsClass="virtual-list-items"
					>
						{#snippet renderItem(activity)}
							<ActivityItem {activity} />
						{/snippet}
					</SvelteVirtualList>
				</div>
				{#if isStreaming}
					<div class="streaming-indicator">
						<Loader2 class="spinning" size={14} />
						<span>{$i18n('workflow_activity_processing')}</span>
					</div>
				{/if}
			{:else}
				<!-- Standard rendering for small lists (<20 items) -->
				<div class="standard-list-container">
					{#each activities as activity (activity.id)}
						<ActivityItem {activity} />
					{/each}
				</div>

				{#if isStreaming}
					<div class="streaming-indicator">
						<Loader2 class="spinning" size={14} />
						<span>{$i18n('workflow_activity_processing')}</span>
					</div>
				{/if}
			{/if}
		</div>
	{/if}
</div>

<style>
	.activity-feed {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-height: 0;
		overflow: hidden;
		background: var(--color-bg-secondary);
		border-left: 1px solid var(--color-border);
	}

	.activity-feed.collapsed {
		width: 48px;
	}

	/* Filter Tabs */
	.filter-tabs {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-primary);
		flex-shrink: 0;
	}

	.collapsed .filter-tabs {
		flex-direction: column;
	}

	.filter-tab {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		padding: 0;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		cursor: pointer;
		color: var(--color-text-secondary);
		transition: all var(--transition-fast, 150ms) ease;
	}

	.filter-tab:hover {
		background: var(--color-bg-tertiary);
		border-color: var(--color-border-hover, var(--color-border));
		color: var(--color-text-primary);
	}

	.filter-tab.active {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: var(--color-text-inverse, white);
	}

	/* Activity List */
	.activity-list {
		flex: 1;
		overflow: hidden;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}

	/* Standard list container for non-virtual rendering */
	.standard-list-container {
		flex: 1;
		min-height: 0;
		overflow-y: auto;
		overflow-x: hidden;
	}

	/* Apply custom scrollbar to standard list */
	.standard-list-container::-webkit-scrollbar {
		width: 6px;
	}

	.standard-list-container::-webkit-scrollbar-track {
		background: transparent;
	}

	.standard-list-container::-webkit-scrollbar-thumb {
		background: var(--color-border);
		border-radius: var(--radius-full);
	}

	.standard-list-container::-webkit-scrollbar-thumb:hover {
		background: var(--color-text-tertiary);
	}

	/* OPT-MSG-5: Virtual List Container - takes full available space */
	.virtual-list-container {
		flex: 1;
		min-height: 0;
		overflow: hidden;
	}

	/* Virtual list component styling - replicate library's positioning logic */
	.virtual-list-container :global(.virtual-list-outer) {
		position: relative;
		width: 100%;
		height: 100%;
		overflow: hidden;
	}

	.virtual-list-container :global(.virtual-list-viewport) {
		position: absolute;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		overflow-y: scroll;
		overflow-x: hidden;
		-webkit-overflow-scrolling: touch;
	}

	.virtual-list-container :global(.virtual-list-content) {
		position: relative;
		width: 100%;
		min-height: 100%;
	}

	.virtual-list-container :global(.virtual-list-items) {
		position: absolute;
		width: 100%;
		left: 0;
		top: 0;
	}

	/* Apply custom scrollbar to virtual list viewport */
	.virtual-list-container :global(.virtual-list-viewport)::-webkit-scrollbar {
		width: 6px;
	}

	.virtual-list-container :global(.virtual-list-viewport)::-webkit-scrollbar-track {
		background: transparent;
	}

	.virtual-list-container :global(.virtual-list-viewport)::-webkit-scrollbar-thumb {
		background: var(--color-border);
		border-radius: var(--radius-full);
	}

	.virtual-list-container :global(.virtual-list-viewport)::-webkit-scrollbar-thumb:hover {
		background: var(--color-text-tertiary);
	}

	/* Empty State */
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-xl);
		color: var(--color-text-tertiary);
		text-align: center;
	}

	.empty-state span {
		font-size: var(--font-size-sm);
	}

	.empty-state :global(.spinning) {
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

	/* Streaming Indicator */
	.streaming-indicator {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
		border-top: 1px solid var(--color-border-light, rgba(0, 0, 0, 0.05));
		background: var(--color-bg-tertiary);
		position: sticky;
		bottom: 0;
		animation: pulse 2s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.7;
		}
	}

	.streaming-indicator :global(.spinning) {
		animation: spin 1s linear infinite;
		color: var(--color-accent);
	}
</style>
