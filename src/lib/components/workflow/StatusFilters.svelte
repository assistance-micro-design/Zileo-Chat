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
  StatusFilters Component
  Horizontal chip buttons to filter workflows by status.
  Displays count per status and highlights the active filter.
-->
<script lang="ts">
	import type { StatusFilter } from '$types/sidebar';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Currently active filter */
		activeFilter: StatusFilter;
		/** Workflow count per status */
		counts: Record<StatusFilter, number>;
		/** Callback when filter changes */
		onfilterchange: (filter: StatusFilter) => void;
	}

	let { activeFilter, counts, onfilterchange }: Props = $props();

	const FILTERS: { key: StatusFilter; labelKey: string }[] = [
		{ key: 'all', labelKey: 'sidebar_filter_all' },
		{ key: 'running', labelKey: 'sidebar_filter_running' },
		{ key: 'completed', labelKey: 'sidebar_filter_completed' },
		{ key: 'error', labelKey: 'sidebar_filter_error' },
		{ key: 'idle', labelKey: 'sidebar_filter_idle' }
	];
</script>

<div class="status-filters" role="radiogroup" aria-label={$i18n('sidebar_filter_all')}>
	{#each FILTERS as filter (filter.key)}
		{@const isActive = activeFilter === filter.key}
		{@const count = counts[filter.key]}
		{#if count > 0 || filter.key === 'all'}
			<button
				type="button"
				class={['status-chip', filter.key, isActive && 'active']}
				role="radio"
				aria-checked={isActive}
				onclick={() => onfilterchange(filter.key)}
			>
				{$i18n(filter.labelKey)}
				<span class="chip-count">{count}</span>
			</button>
		{/if}
	{/each}
</div>

<style>
	.status-filters {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
	}

	.status-chip {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px var(--spacing-sm);
		font-size: var(--font-size-xs);
		font-family: var(--font-family);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
		background: var(--color-bg-tertiary);
		border: 1px solid transparent;
		border-radius: var(--border-radius-full);
		cursor: pointer;
		transition:
			background var(--transition-fast),
			color var(--transition-fast),
			border-color var(--transition-fast);
		white-space: nowrap;
		line-height: 1.4;
	}

	.status-chip:hover {
		background: var(--color-bg-secondary);
		border-color: var(--color-border);
	}

	.status-chip.active {
		color: var(--color-accent-text);
		background: var(--color-accent);
		border-color: var(--color-accent);
	}

	.status-chip.active:hover {
		opacity: 0.9;
	}

	.status-chip.running:not(.active) {
		color: var(--color-success);
	}

	.status-chip.error:not(.active) {
		color: var(--color-error);
	}

	.chip-count {
		font-size: 10px;
		opacity: 0.8;
	}

	.status-chip.active .chip-count {
		opacity: 0.9;
	}
</style>
