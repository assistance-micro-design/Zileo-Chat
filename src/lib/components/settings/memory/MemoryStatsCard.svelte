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

MemoryStatsCard - Displays memory statistics with category breakdown.
Extracted from MemorySettings.svelte.
-->

<script lang="ts">
	import { Card, Badge, ProgressBar } from '$lib/components/ui';
	import type { MemoryStats, MemoryTokenStats } from '$types/embedding';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Memory statistics */
		stats: MemoryStats | null;
		/** Token statistics */
		tokenStats: MemoryTokenStats | null;
	}

	let { stats, tokenStats }: Props = $props();

	/**
	 * Format large numbers for display
	 */
	function formatNumber(n: number): string {
		if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`;
		if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
		return n.toString();
	}

	/**
	 * Get badge variant based on memory type
	 */
	function getTypeVariant(type: string): 'primary' | 'success' | 'warning' | 'error' {
		switch (type) {
			case 'knowledge':
				return 'warning';
			case 'context':
				return 'success';
			case 'decision':
				return 'error';
			case 'user_pref':
				return 'primary';
			default:
				return 'primary';
		}
	}
</script>

{#if stats || tokenStats}
	<Card>
		{#snippet header()}
			<h3 class="card-title">{$i18n('memory_stats_title')}</h3>
		{/snippet}
		{#snippet body()}
			<div class="unified-stats">
				<!-- Summary Row -->
				<div class="summary-stats">
					<div class="summary-item">
						<span class="summary-value"
							>{formatNumber(stats?.total ?? tokenStats?.total_memories ?? 0)}</span
						>
						<span class="summary-label">{$i18n('memory_total_memories')}</span>
					</div>
					<div class="summary-item">
						<span class="summary-value">{formatNumber(tokenStats?.total_chars ?? 0)}</span>
						<span class="summary-label">{$i18n('memory_total_characters')}</span>
					</div>
					<div class="summary-item">
						<span class="summary-value"
							>{formatNumber(tokenStats?.total_estimated_tokens ?? 0)}</span
						>
						<span class="summary-label">{$i18n('memory_est_tokens')}</span>
					</div>
					<div class="summary-item">
						<span class="summary-value">{stats?.with_embeddings ?? 0}/{stats?.total ?? 0}</span>
						<span class="summary-label">{$i18n('memory_with_embeddings')}</span>
					</div>
				</div>

				<!-- Category Breakdown -->
				{#if tokenStats && tokenStats.categories.length > 0}
					<div class="categories-section">
						<h4 class="section-title">{$i18n('memory_by_category')}</h4>
						<div class="categories-list">
							{#each tokenStats.categories as cat (cat.memory_type)}
								<div class="category-item">
									<div class="category-header">
										<Badge variant={getTypeVariant(cat.memory_type)}>{cat.memory_type}</Badge>
										<span class="category-count">{cat.count} {$i18n('memory_memories_count')}</span>
										<span class="embedding-status"
											>{cat.with_embeddings}/{cat.count} {$i18n('memory_embedded')}</span
										>
									</div>
									<div class="category-details">
										<span class="token-count"
											>{formatNumber(cat.estimated_tokens)} {$i18n('memory_tokens')}</span
										>
										<span class="char-count"
											>({formatNumber(cat.total_chars)} {$i18n('memory_chars')})</span
										>
									</div>
									<ProgressBar
										value={tokenStats.total_chars > 0
											? (cat.total_chars / tokenStats.total_chars) * 100
											: 0}
										showLabel={false}
									/>
								</div>
							{/each}
						</div>
					</div>
				{:else if stats && Object.keys(stats.by_type).length > 0}
					<div class="categories-section">
						<h4 class="section-title">{$i18n('memory_by_type')}</h4>
						<div class="type-list">
							{#each Object.entries(stats.by_type) as [type, count] (type)}
								<div class="type-item">
									<Badge variant={getTypeVariant(type)}>{type}</Badge>
									<span class="type-count">{count}</span>
								</div>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/snippet}
	</Card>
{/if}

<style>
	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.unified-stats {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.summary-stats {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.summary-item {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
	}

	.summary-value {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-bold);
		color: var(--color-accent);
	}

	.summary-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.categories-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.section-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-secondary);
	}

	.categories-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.category-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
	}

	.category-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.category-count {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.embedding-status {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-left: auto;
	}

	.category-details {
		display: flex;
		gap: var(--spacing-sm);
		font-size: var(--font-size-sm);
	}

	.token-count {
		color: var(--color-text-primary);
	}

	.char-count {
		color: var(--color-text-secondary);
	}

	.type-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
	}

	.type-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-sm);
	}

	.type-count {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	@media (max-width: 768px) {
		.summary-stats {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 480px) {
		.summary-stats {
			grid-template-columns: 1fr;
		}

		.category-header {
			flex-direction: column;
			align-items: flex-start;
		}

		.embedding-status {
			margin-left: 0;
		}
	}
</style>
