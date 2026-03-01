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
SkillList - Displays skills in a grid of cards.
Shows skill summary with actions for edit, delete, and enable/disable toggle.
-->

<script lang="ts">
	import type { SkillSummary, SkillCategory } from '$types/skill';
	import { Card, Badge, Button, StatusIndicator, Input, Select } from '$lib/components/ui';
	import { BookMarked, Edit, Trash2, FileText } from '@lucide/svelte';
	import { i18n, t } from '$lib/i18n';

	/**
	 * Component props
	 */
	interface Props {
		/** List of skills to display */
		skills: SkillSummary[];
		/** Loading state */
		loading: boolean;
		/** Edit callback */
		onedit: (skillId: string) => void;
		/** Delete callback */
		ondelete: (skillId: string) => void;
		/** Toggle enabled callback */
		ontoggle: (skillId: string, enabled: boolean) => void;
	}

	let { skills, loading, onedit, ondelete, ontoggle }: Props = $props();

	// Filter state
	let searchQuery = $state('');
	let debouncedQuery = $state('');
	let categoryFilter = $state<SkillCategory | ''>('');

	let searchTimeout: ReturnType<typeof setTimeout>;
	function handleSearchInput(event: Event & { currentTarget: HTMLInputElement }): void {
		searchQuery = event.currentTarget.value;
		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			debouncedQuery = searchQuery;
		}, 300);
	}

	// Category labels mapping for i18n
	const categoryI18nKeys: Record<SkillCategory, string> = {
		system: 'skills_category_system',
		coding: 'skills_category_coding',
		workflow: 'skills_category_workflow',
		analysis: 'skills_category_analysis',
		custom: 'skills_category_custom'
	};

	// Category options with "All" option
	let categoryOptions = $derived([
		{ value: '', label: t('skills_all_categories') },
		...(['system', 'coding', 'workflow', 'analysis', 'custom'] as SkillCategory[]).map(
			(value) => ({
				value,
				label: t(categoryI18nKeys[value])
			})
		)
	]);

	// Filtered skills (uses debouncedQuery for performance)
	let filteredSkills = $derived.by(() => {
		let result = skills;

		if (debouncedQuery.trim()) {
			const query = debouncedQuery.toLowerCase();
			result = result.filter(
				(s) =>
					s.name.toLowerCase().includes(query) ||
					s.description.toLowerCase().includes(query)
			);
		}

		if (categoryFilter) {
			result = result.filter((s) => s.category === categoryFilter);
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
	function getCategoryVariant(category: SkillCategory): 'primary' | 'warning' {
		return category === 'system' ? 'warning' : 'primary';
	}

	/**
	 * Formats content length for display
	 */
	function formatContentLength(length: number): string {
		if (length >= 1000) {
			return `${(length / 1000).toFixed(1)}k`;
		}
		return String(length);
	}
</script>

<div class="skill-list">
	<!-- Filters -->
	<div class="list-filters">
		<Input
			placeholder={$i18n('skills_search_placeholder')}
			value={searchQuery}
			oninput={handleSearchInput}
		/>
		<Select
			value={categoryFilter}
			onchange={(e) => (categoryFilter = e.currentTarget.value as SkillCategory | '')}
			options={categoryOptions}
		/>
	</div>

	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>{$i18n('skills_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if filteredSkills.length === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<BookMarked size={48} class="empty-icon" />
					{#if skills.length === 0}
						<h3 class="empty-title">{$i18n('skills_no_skills')}</h3>
						<p class="empty-description">
							{$i18n('skills_no_skills_description')}
						</p>
					{:else}
						<h3 class="empty-title">{$i18n('skills_no_match')}</h3>
						<p class="empty-description">
							{$i18n('skills_no_match_description')}
						</p>
					{/if}
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="skill-grid">
			{#each filteredSkills as skill (skill.id)}
				<Card>
					{#snippet body()}
						<div class="skill-card" class:disabled={!skill.enabled}>
							<div class="skill-header">
								<div class="skill-name-row">
									<BookMarked size={20} class="skill-icon" />
									<h4 class="skill-name">{skill.name}</h4>
								</div>
								<Badge variant={getCategoryVariant(skill.category)}>
									{$i18n(categoryI18nKeys[skill.category])}
								</Badge>
							</div>

							<p class="skill-description">
								{skill.description || $i18n('skills_no_description')}
							</p>

							<div class="skill-details">
								<div class="detail-row">
									<span class="detail-label">
										<FileText size={14} />
										{$i18n('skills_content_size')}
									</span>
									<span class="detail-value">
										{$i18n('skills_chars_count').replace('{count}', formatContentLength(skill.content_length))}
									</span>
								</div>
								<div class="detail-row">
									<span class="detail-label">{$i18n('skills_updated')}</span>
									<span class="detail-value">{formatDate(skill.updated_at)}</span>
								</div>
								<div class="detail-row">
									<span class="detail-label">{$i18n('skills_status')}</span>
									<label class="toggle-label">
										<input
											type="checkbox"
											checked={skill.enabled}
											onchange={() => ontoggle(skill.id, !skill.enabled)}
											class="toggle-input"
										/>
										<span class="toggle-text">
											{skill.enabled ? $i18n('skills_enabled') : $i18n('skills_disabled')}
										</span>
									</label>
								</div>
							</div>

							<div class="skill-actions">
								<Button variant="ghost" size="sm" onclick={() => onedit(skill.id)}>
									<Edit size={16} />
									<span>{$i18n('common_edit')}</span>
								</Button>
								<Button
									variant="danger"
									size="sm"
									onclick={() => ondelete(skill.id)}
								>
									<Trash2 size={16} />
									<span>{$i18n('common_delete')}</span>
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
	.skill-list {
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

	.skill-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: var(--spacing-lg);
		contain: layout style;
	}

	.skill-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.skill-card.disabled {
		opacity: 0.6;
	}

	.skill-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
	}

	.skill-name-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.skill-name-row :global(.skill-icon) {
		color: var(--color-accent);
	}

	.skill-name {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.skill-description {
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

	.skill-details {
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

	.toggle-label {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		cursor: pointer;
	}

	.toggle-input {
		accent-color: var(--color-accent);
	}

	.toggle-text {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.skill-actions {
		display: flex;
		gap: var(--spacing-sm);
		justify-content: flex-end;
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	.skill-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	@media (max-width: 768px) {
		.skill-grid {
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
