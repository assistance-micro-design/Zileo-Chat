<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

PromptList - Displays prompts in a grid of cards.
Shows prompt summary with actions for edit and delete.
-->

<script lang="ts">
	import type { PromptSummary, PromptCategory } from '$types/prompt';
	import { PROMPT_CATEGORY_LABELS } from '$types/prompt';
	import { Card, Badge, Button, StatusIndicator, Input, Select } from '$lib/components/ui';
	import { FileText, Edit, Trash2, Variable } from 'lucide-svelte';

	/**
	 * Component props
	 */
	interface Props {
		/** List of prompts to display */
		prompts: PromptSummary[];
		/** Loading state */
		loading: boolean;
		/** Edit callback */
		onedit: (promptId: string) => void;
		/** Delete callback */
		ondelete: (promptId: string) => void;
	}

	let { prompts, loading, onedit, ondelete }: Props = $props();

	// Filter state
	let searchQuery = $state('');
	let categoryFilter = $state<PromptCategory | ''>('');

	// Category options with "All" option
	const categoryOptions = [
		{ value: '', label: 'All Categories' },
		...Object.entries(PROMPT_CATEGORY_LABELS).map(([value, label]) => ({
			value: value as PromptCategory,
			label
		}))
	];

	// Filtered prompts
	let filteredPrompts = $derived.by(() => {
		let result = prompts;

		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			result = result.filter(
				(p) =>
					p.name.toLowerCase().includes(query) ||
					p.description.toLowerCase().includes(query)
			);
		}

		if (categoryFilter) {
			result = result.filter((p) => p.category === categoryFilter);
		}

		return result;
	});

	/**
	 * Formats a date string for display
	 */
	function formatDate(dateStr: string): string {
		return new Date(dateStr).toLocaleDateString(undefined, {
			year: 'numeric',
			month: 'short',
			day: 'numeric'
		});
	}

	/**
	 * Gets badge variant for category type
	 */
	function getCategoryVariant(category: PromptCategory): 'primary' | 'warning' {
		return category === 'system' ? 'warning' : 'primary';
	}
</script>

<div class="prompt-list">
	<!-- Filters -->
	<div class="list-filters">
		<Input
			placeholder="Search prompts..."
			value={searchQuery}
			oninput={(e) => (searchQuery = e.currentTarget.value)}
		/>
		<Select
			value={categoryFilter}
			onchange={(e) => (categoryFilter = e.currentTarget.value as PromptCategory | '')}
			options={categoryOptions}
		/>
	</div>

	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>Loading prompts...</span>
				</div>
			{/snippet}
		</Card>
	{:else if filteredPrompts.length === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<FileText size={48} class="empty-icon" />
					{#if prompts.length === 0}
						<h3 class="empty-title">No Prompts Yet</h3>
						<p class="empty-description">
							Create your first prompt template to start building reusable prompts
							with variable placeholders.
						</p>
					{:else}
						<h3 class="empty-title">No Matching Prompts</h3>
						<p class="empty-description">
							Try adjusting your search query or category filter.
						</p>
					{/if}
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="prompt-grid">
			{#each filteredPrompts as prompt (prompt.id)}
				<Card>
					{#snippet body()}
						<div class="prompt-card">
							<div class="prompt-header">
								<div class="prompt-name-row">
									<FileText size={20} class="prompt-icon" />
									<h4 class="prompt-name">{prompt.name}</h4>
								</div>
								<Badge variant={getCategoryVariant(prompt.category)}>
									{PROMPT_CATEGORY_LABELS[prompt.category]}
								</Badge>
							</div>

							<p class="prompt-description">
								{prompt.description || 'No description provided'}
							</p>

							<div class="prompt-details">
								<div class="detail-row">
									<span class="detail-label">
										<Variable size={14} />
										Variables
									</span>
									<span class="detail-value">
										{prompt.variables_count} placeholder{prompt.variables_count !== 1 ? 's' : ''}
									</span>
								</div>
								<div class="detail-row">
									<span class="detail-label">Updated</span>
									<span class="detail-value">{formatDate(prompt.updated_at)}</span>
								</div>
							</div>

							<div class="prompt-actions">
								<Button variant="ghost" size="sm" onclick={() => onedit(prompt.id)}>
									<Edit size={16} />
									<span>Edit</span>
								</Button>
								<Button variant="danger" size="sm" onclick={() => ondelete(prompt.id)}>
									<Trash2 size={16} />
									<span>Delete</span>
								</Button>
							</div>
						</div>
					{/snippet}
				</Card>
			{/each}
		</div>
	{/if}
</div>

<style>
	.prompt-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.list-filters {
		display: flex;
		gap: var(--spacing-md);
		max-width: 500px;
	}

	.list-filters :global(> *:first-child) {
		flex: 2;
	}

	.list-filters :global(> *:last-child) {
		flex: 1;
		min-width: 150px;
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: var(--spacing-2xl);
		gap: var(--spacing-md);
	}

	.empty-state :global(.empty-icon) {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.empty-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.empty-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		max-width: 400px;
		margin: 0;
		line-height: var(--line-height-relaxed);
	}

	.prompt-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: var(--spacing-lg);
	}

	.prompt-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.prompt-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
	}

	.prompt-name-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.prompt-name-row :global(.prompt-icon) {
		color: var(--color-accent);
	}

	.prompt-name {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.prompt-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
		line-height: var(--line-height-relaxed);
	}

	.prompt-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.detail-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: var(--font-size-sm);
	}

	.detail-label {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		color: var(--color-text-secondary);
	}

	.detail-value {
		font-weight: var(--font-weight-medium);
	}

	.prompt-actions {
		display: flex;
		gap: var(--spacing-sm);
		justify-content: flex-end;
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	.prompt-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	@media (max-width: 768px) {
		.prompt-grid {
			grid-template-columns: 1fr;
		}

		.list-filters {
			flex-direction: column;
			max-width: none;
		}

		.list-filters :global(> *) {
			flex: 1 !important;
		}
	}
</style>
