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
SkillList - Displays skills as compact entity rows inside a single card.
Each row carries the category / target-kind badges and the enable switch;
actions (edit, version history, delete) are always visible.
-->

<script lang="ts">
	import type { SkillSummary, SkillCategory } from '$types/skill';
	import { SKILL_CATEGORY_I18N_KEYS } from '$types/skill';
	import { Card, Badge, Button, StatusIndicator, Input, Select, Switch } from '$lib/components/ui';
	import { BookMarked, Search, Pencil, History, Trash2 } from '@lucide/svelte';
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
		/** Version history callback */
		onhistory: (skillId: string) => void;
		/** Delete callback */
		ondelete: (skillId: string) => void;
		/** Toggle enabled callback */
		ontoggle: (skillId: string, enabled: boolean) => void;
	}

	let { skills, loading, onedit, onhistory, ondelete, ontoggle }: Props = $props();

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

	// Category options with "All" option
	let categoryOptions = $derived([
		{ value: '', label: t('skills_all_categories') },
		...(Object.keys(SKILL_CATEGORY_I18N_KEYS) as SkillCategory[]).map((value) => ({
			value,
			label: t(SKILL_CATEGORY_I18N_KEYS[value])
		}))
	]);

	// Filtered skills (uses debouncedQuery for performance)
	let filteredSkills = $derived.by(() => {
		let result = skills;

		if (debouncedQuery.trim()) {
			const query = debouncedQuery.toLowerCase();
			result = result.filter(
				(s) => s.name.toLowerCase().includes(query) || s.description.toLowerCase().includes(query)
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
	 * Builds the meta line: description · content size · last update
	 */
	function metaLine(skill: SkillSummary): string {
		const description = skill.description || t('skills_no_description');
		const size = t('skills_chars_count').replace('{count}', skill.content_length.toLocaleString());
		const updated = `${t('skills_updated')} ${formatDate(skill.updated_at)}`;
		return `${description} · ${size} · ${updated}`;
	}
</script>

<div class="skill-list">
	<!-- Filters -->
	<div class="list-filters">
		<div class="search-box">
			<Search size={14} class="search-icon" />
			<Input
				placeholder={$i18n('skills_search_placeholder')}
				value={searchQuery}
				oninput={handleSearchInput}
			/>
		</div>
		<Select
			value={categoryFilter}
			onchange={(e) => (categoryFilter = e.currentTarget.value as SkillCategory | '')}
			options={categoryOptions}
			ariaLabel={$i18n('skills_form_category_label')}
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
		<div class="entity-list">
			{#each filteredSkills as skill (skill.id)}
				<div class="entity-row" class:is-disabled={!skill.enabled}>
					<BookMarked size={20} class="entity-icon" />
					<div class="entity-main">
						<span class="entity-title">
							<strong>{skill.name}</strong>
							<Badge variant={getCategoryVariant(skill.category)}>
								{$i18n(SKILL_CATEGORY_I18N_KEYS[skill.category])}
							</Badge>
							{#if skill.kind === 'kanban'}
								<Badge variant="success">{$i18n('skills_kind_kanban')}</Badge>
							{:else}
								<Badge variant="neutral">{$i18n('skills_kind_standard')}</Badge>
							{/if}
						</span>
						<span class="entity-meta">{metaLine(skill)}</span>
					</div>
					<Switch
						checked={skill.enabled}
						onchange={(value) => ontoggle(skill.id, value)}
						ariaLabel={$i18n('skills_toggle_arialabel').replace('{name}', skill.name)}
					/>
					<div class="entity-actions">
						<Button
							variant="ghost"
							size="sm"
							onclick={() => onedit(skill.id)}
							ariaLabel="{$i18n('common_edit')}: {skill.name}"
						>
							<Pencil size={14} />
						</Button>
						<Button
							variant="ghost"
							size="sm"
							onclick={() => onhistory(skill.id)}
							ariaLabel="{$i18n('versions_history_button')}: {skill.name}"
						>
							<History size={14} />
						</Button>
						<Button
							variant="ghost"
							size="sm"
							onclick={() => ondelete(skill.id)}
							ariaLabel="{$i18n('common_delete')}: {skill.name}"
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
	.skill-list {
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

	.entity-row.is-disabled .entity-main,
	.entity-row.is-disabled :global(.entity-icon) {
		opacity: 0.6;
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
