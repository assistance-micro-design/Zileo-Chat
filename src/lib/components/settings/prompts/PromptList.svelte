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

PromptList - Displays prompts as compact entity rows inside a single card.
Row actions (edit, version history, delete) are always visible.
-->

<script lang="ts">
	import type { PromptSummary, PromptCategory } from '$types/prompt';
	import { PROMPT_CATEGORY_I18N_KEYS } from '$types/prompt';
	import { Card, Badge, Button, StatusIndicator, Input, Select } from '$lib/components/ui';
	import { FileText, BookOpen, Search, Pencil, History, Trash2 } from '@lucide/svelte';
	import { i18n, t } from '$lib/i18n';

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
		/** Version history callback */
		onhistory: (promptId: string) => void;
		/** Delete callback */
		ondelete: (promptId: string) => void;
	}

	let { prompts, loading, onedit, onhistory, ondelete }: Props = $props();

	// Filter state
	let searchQuery = $state('');
	let debouncedQuery = $state('');
	let categoryFilter = $state<PromptCategory | ''>('');

	let searchTimeout: ReturnType<typeof setTimeout>;
	function handleSearchInput(event: Event & { currentTarget: HTMLInputElement }): void {
		searchQuery = event.currentTarget.value;
		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			debouncedQuery = searchQuery;
		}, 300);
	}

	// Category options with "All" option
	let categoryOptions = $derived([
		{ value: '', label: t('prompts_all_categories') },
		...(Object.keys(PROMPT_CATEGORY_I18N_KEYS) as PromptCategory[]).map((value) => ({
			value,
			label: t(PROMPT_CATEGORY_I18N_KEYS[value])
		}))
	]);

	// Filtered prompts (uses debouncedQuery for performance)
	let filteredPrompts = $derived.by(() => {
		let result = prompts;

		if (debouncedQuery.trim()) {
			const query = debouncedQuery.toLowerCase();
			result = result.filter(
				(p) => p.name.toLowerCase().includes(query) || p.description.toLowerCase().includes(query)
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

	/**
	 * Builds the meta line: description · variable count · last update
	 */
	function metaLine(prompt: PromptSummary): string {
		const description = prompt.description || t('prompts_no_description');
		const variables = (
			prompt.variables_count !== 1
				? t('prompts_placeholder_count_plural')
				: t('prompts_placeholder_count')
		).replace('{count}', String(prompt.variables_count));
		const updated = `${t('prompts_updated')} ${formatDate(prompt.updated_at)}`;
		return `${description} · ${variables} · ${updated}`;
	}
</script>

<div class="prompt-list">
	<!-- Filters -->
	<div class="list-filters">
		<div class="search-box">
			<Search size={14} class="search-icon" />
			<Input
				placeholder={$i18n('prompts_search_placeholder')}
				value={searchQuery}
				oninput={handleSearchInput}
			/>
		</div>
		<Select
			value={categoryFilter}
			onchange={(e) => (categoryFilter = e.currentTarget.value as PromptCategory | '')}
			options={categoryOptions}
			ariaLabel={$i18n('prompts_form_category_label')}
		/>
	</div>

	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>{$i18n('prompts_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if filteredPrompts.length === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<FileText size={48} class="empty-icon" />
					{#if prompts.length === 0}
						<h3 class="empty-title">{$i18n('prompts_no_prompts')}</h3>
						<p class="empty-description">
							{$i18n('prompts_no_prompts_description')}
						</p>
					{:else}
						<h3 class="empty-title">{$i18n('prompts_no_match')}</h3>
						<p class="empty-description">
							{$i18n('prompts_no_match_description')}
						</p>
					{/if}
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="entity-list">
			{#each filteredPrompts as prompt (prompt.id)}
				<div class="entity-row">
					<BookOpen size={20} class="entity-icon" />
					<div class="entity-main">
						<span class="entity-title">
							<strong>{prompt.name}</strong>
							<Badge variant={getCategoryVariant(prompt.category)}>
								{$i18n(PROMPT_CATEGORY_I18N_KEYS[prompt.category])}
							</Badge>
						</span>
						<span class="entity-meta">{metaLine(prompt)}</span>
					</div>
					<div class="entity-actions">
						<Button
							variant="ghost"
							size="sm"
							onclick={() => onedit(prompt.id)}
							ariaLabel="{$i18n('common_edit')}: {prompt.name}"
						>
							<Pencil size={14} />
						</Button>
						<Button
							variant="ghost"
							size="sm"
							onclick={() => onhistory(prompt.id)}
							ariaLabel="{$i18n('versions_history_button')}: {prompt.name}"
						>
							<History size={14} />
						</Button>
						<Button
							variant="ghost"
							size="sm"
							onclick={() => ondelete(prompt.id)}
							ariaLabel="{$i18n('common_delete')}: {prompt.name}"
						>
							<Trash2 size={14} />
						</Button>
					</div>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.prompt-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.list-filters {
		display: flex;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.search-box {
		position: relative;
		flex: 1;
		min-width: 240px;
	}

	.search-box :global(.search-icon) {
		position: absolute;
		left: 10px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-text-tertiary);
		z-index: 1;
		pointer-events: none;
	}

	.search-box :global(input) {
		padding-left: 2rem;
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

	.entity-list {
		background: var(--surface-1);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		box-shadow: var(--shadow-sm);
		overflow: hidden;
	}

	.entity-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light);
	}

	.entity-row:last-child {
		border-bottom: none;
	}

	.entity-row :global(.entity-icon) {
		color: var(--color-accent-deep);
		flex-shrink: 0;
	}

	.entity-main {
		flex: 1;
		min-width: 0;
		display: flex;
		flex-direction: column;
		gap: 1px;
	}

	.entity-title {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		flex-wrap: wrap;
	}

	.entity-title strong {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
	}

	.entity-meta {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.entity-actions {
		display: flex;
		gap: var(--spacing-xs);
		flex-shrink: 0;
	}

	@media (max-width: 768px) {
		.list-filters {
			flex-direction: column;
		}
	}
</style>
