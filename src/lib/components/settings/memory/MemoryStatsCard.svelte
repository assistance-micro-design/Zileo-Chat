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

MemoryStatsCard - Memory statistics as metric chips with category breakdown.
-->

<script lang="ts">
	import { Card, Badge, ProgressBar } from '$lib/components/ui';
	import type { MemoryStats, MemoryTokenStats } from '$types/embedding';
	import { i18n, t } from '$lib/i18n';
	import { getTypeVariant } from './MemoryList.helpers';
	import type { MemoryType } from '$types/memory';

	interface Props {
		/** Memory statistics */
		stats: MemoryStats | null;
		/** Token statistics */
		tokenStats: MemoryTokenStats | null;
	}

	let { stats, tokenStats }: Props = $props();

	/** Localized labels for memory types (badges show readable names) */
	const typeLabels = $derived<Record<string, string>>({
		user_pref: t('memory_type_user_pref'),
		context: t('memory_type_context'),
		knowledge: t('memory_type_knowledge'),
		decision: t('memory_type_decision')
	});

	function typeLabel(type: string): string {
		return typeLabels[type] ?? type;
	}
</script>

{#if stats || tokenStats}
	<Card title={$i18n('memory_stats_title')}>
		{#snippet body()}
			<div class="unified-stats">
				<!-- Summary chips -->
				<div class="metric-grid">
					<div class="metric-chip">
						<span>{$i18n('memory_total_memories')}</span>
						<strong>{(stats?.total ?? tokenStats?.total_memories ?? 0).toLocaleString()}</strong>
					</div>
					<div class="metric-chip">
						<span>{$i18n('memory_with_embeddings')}</span>
						<strong>{(stats?.with_embeddings ?? 0).toLocaleString()}</strong>
					</div>
					<div class="metric-chip">
						<span>{$i18n('memory_total_characters')}</span>
						<strong>{(tokenStats?.total_chars ?? 0).toLocaleString()}</strong>
					</div>
					<div class="metric-chip">
						<span>{$i18n('memory_est_tokens')}</span>
						<strong>~{(tokenStats?.total_estimated_tokens ?? 0).toLocaleString()}</strong>
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
										<Badge variant={getTypeVariant(cat.memory_type as MemoryType)}>
											{typeLabel(cat.memory_type)}
										</Badge>
										<span class="category-count">{cat.count} {$i18n('memory_memories_count')}</span>
										<span class="embedding-status"
											>{cat.with_embeddings}/{cat.count} {$i18n('memory_embedded')}</span
										>
									</div>
									<div class="category-details">
										<span class="token-count"
											>{cat.estimated_tokens.toLocaleString()} {$i18n('memory_tokens')}</span
										>
										<span class="char-count"
											>({cat.total_chars.toLocaleString()} {$i18n('memory_chars')})</span
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
									<Badge variant={getTypeVariant(type as MemoryType)}>{typeLabel(type)}</Badge>
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
	.unified-stats {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.metric-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-sm);
	}

	.metric-chip {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-xs);
		padding: 0.3rem 0.6rem;
		font-size: var(--font-size-xs);
		font-variant-numeric: tabular-nums;
		color: var(--color-text-secondary);
		background: var(--surface-2);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-full);
	}

	.metric-chip strong {
		color: var(--color-text-primary);
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

	@media (max-width: 480px) {
		.metric-grid {
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
