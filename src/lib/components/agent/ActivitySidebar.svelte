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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

ActivitySidebar Component - Phase C Component Extraction
Right sidebar for displaying workflow activity feed with filtering.
-->

<script lang="ts">
	import { Activity } from 'lucide-svelte';
	import RightSidebar from '$lib/components/layout/RightSidebar.svelte';
	import ActivityFeed from '$lib/components/workflow/ActivityFeed.svelte';
	import { HelpButton } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { WorkflowActivityEvent, ActivityFilter } from '$types/activity';

	interface Props {
		collapsed?: boolean;
		activities: WorkflowActivityEvent[];
		isStreaming?: boolean;
		filter?: ActivityFilter;
		onfilterchange?: (filter: ActivityFilter) => void;
	}

	let {
		collapsed = $bindable(false),
		activities,
		isStreaming = false,
		filter = $bindable<ActivityFilter>('all'),
		onfilterchange
	}: Props = $props();

	function handleFilterChange(newFilter: ActivityFilter) {
		filter = newFilter;
		onfilterchange?.(newFilter);
	}
</script>

<RightSidebar bind:collapsed={collapsed}>
	{#snippet header(isCollapsed)}
		<div class="activity-header" class:collapsed={isCollapsed}>
			<Activity size={20} class="header-icon" />
			{#if !isCollapsed}
				<span class="activity-title">{$i18n('activity_title')}</span>
				<HelpButton
					titleKey="help_activity_sidebar_title"
					descriptionKey="help_activity_sidebar_description"
					tutorialKey="help_activity_sidebar_tutorial"
				/>
				{#if isStreaming}
					<span class="streaming-indicator"></span>
				{/if}
			{/if}
		</div>
	{/snippet}

	{#snippet content(isCollapsed)}
		<ActivityFeed
			{activities}
			{isStreaming}
			filter={filter}
			onFilterChange={handleFilterChange}
			collapsed={isCollapsed}
		/>
	{/snippet}
</RightSidebar>

<style>
	.activity-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
	}

	.activity-header.collapsed {
		justify-content: center;
		padding: var(--spacing-sm);
	}

	.activity-header :global(.header-icon) {
		color: var(--color-accent);
		flex-shrink: 0;
	}

	.activity-title {
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.streaming-indicator {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background: var(--color-success);
		animation: pulse 1.5s infinite;
		flex-shrink: 0;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.5;
		}
	}
</style>
