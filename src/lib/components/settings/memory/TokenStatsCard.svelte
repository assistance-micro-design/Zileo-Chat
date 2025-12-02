<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

TokenStatsCard - Displays token/character statistics per memory category.
-->

<script lang="ts">
	import { Card, Badge, ProgressBar } from '$lib/components/ui';
	import type { MemoryTokenStats } from '$types/embedding';

	interface Props {
		stats: MemoryTokenStats;
	}

	let { stats }: Props = $props();

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
				return 'primary';
			case 'context':
				return 'success';
			case 'decision':
				return 'warning';
			case 'user_pref':
				return 'error';
			default:
				return 'primary';
		}
	}
</script>

<Card>
	{#snippet header()}
		<h3 class="card-title">Token Usage by Category</h3>
	{/snippet}
	{#snippet body()}
		<div class="stats-container">
			<!-- Summary Stats -->
			<div class="summary-stats">
				<div class="summary-item">
					<span class="summary-label">Total Memories</span>
					<span class="summary-value">{formatNumber(stats.total_memories)}</span>
				</div>
				<div class="summary-item">
					<span class="summary-label">Total Characters</span>
					<span class="summary-value">{formatNumber(stats.total_chars)}</span>
				</div>
				<div class="summary-item">
					<span class="summary-label">Est. Tokens</span>
					<span class="summary-value">{formatNumber(stats.total_estimated_tokens)}</span>
				</div>
			</div>

			<!-- Category Breakdown -->
			{#if stats.categories.length > 0}
				<div class="categories-section">
					<h4 class="section-title">By Category</h4>
					<div class="categories-list">
						{#each stats.categories as cat}
							<div class="category-item">
								<div class="category-header">
									<Badge variant={getTypeVariant(cat.memory_type)}
										>{cat.memory_type}</Badge
									>
									<span class="category-count">{cat.count} memories</span>
								</div>
								<div class="category-details">
									<span class="token-count">{formatNumber(cat.estimated_tokens)} tokens</span>
									<span class="char-count">({formatNumber(cat.total_chars)} chars)</span>
								</div>
								<ProgressBar
									value={stats.total_chars > 0 ? (cat.total_chars / stats.total_chars) * 100 : 0}
									showLabel={false}
								/>
								<div class="embedding-info">
									{cat.with_embeddings}/{cat.count} with embeddings
								</div>
							</div>
						{/each}
					</div>
				</div>
			{:else}
				<div class="empty-state">
					<p>No memories stored yet</p>
				</div>
			{/if}
		</div>
	{/snippet}
</Card>

<style>
	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.stats-container {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.summary-stats {
		display: flex;
		gap: var(--spacing-xl);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.summary-item {
		display: flex;
		flex-direction: column;
	}

	.summary-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.summary-value {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
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
		gap: var(--spacing-md);
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
	}

	.category-count {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
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

	.embedding-info {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.empty-state {
		text-align: center;
		padding: var(--spacing-lg);
		color: var(--color-text-secondary);
	}

	@media (max-width: 768px) {
		.summary-stats {
			flex-direction: column;
			gap: var(--spacing-md);
		}
	}
</style>
