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
SkillForm - Form component for creating and editing skills.
Displays in a modal with markdown content editor.
-->

<script lang="ts">
	import { tauriInvoke } from '$lib/tauri';
	import { Button, Input, Textarea, Select, Badge } from '$lib/components/ui';
	import VersionsHistoryModal from '$lib/components/settings/versions/VersionsHistoryModal.svelte';
	import type { Skill, SkillCreate, SkillCategory } from '$types/skill';
	import { SKILL_CATEGORY_I18N_KEYS } from '$types/skill';
	import type { AgentKind } from '$types/agent';
	import { i18n, t } from '$lib/i18n';

	/**
	 * Component props
	 */
	interface Props {
		/** Form mode - create or edit */
		mode: 'create' | 'edit';
		/** Existing skill data for edit mode */
		skill?: Skill | null;
		/** Whether the form is currently saving */
		saving?: boolean;
		/** Callback when form is submitted */
		onsave?: (data: SkillCreate) => void;
		/** Callback when form is cancelled */
		oncancel?: () => void;
	}

	let { mode, skill = null, saving = false, onsave, oncancel }: Props = $props();

	let showVersions = $state(false);
	/** Number of historical versions for this skill; null while loading. */
	let versionCount = $state<number | null>(null);

	async function loadVersionCount(skillId: string): Promise<void> {
		try {
			const list = await tauriInvoke<Array<{ id: string }>>('list_skill_versions', { skillId });
			versionCount = list.length;
		} catch {
			versionCount = null;
		}
	}

	// Form state
	let name = $state('');
	let description = $state('');
	let category = $state<SkillCategory>('custom');
	let content = $state('');
	// 'standard' = no specialization (undefined on the wire), 'kanban' = Kanban-only skill.
	// Immutable after creation: the field is disabled in edit mode.
	let kind = $state<'standard' | AgentKind>('standard');

	// Sync form state when skill prop changes (e.g., switching between edit targets)
	$effect(() => {
		name = skill?.name ?? '';
		description = skill?.description ?? '';
		category = skill?.category ?? 'custom';
		content = skill?.content ?? '';
		kind = skill?.kind ?? 'standard';
	});

	// Refresh the version count whenever the editing target changes or the
	// history modal closes (a restore creates a new snapshot, bumping count).
	$effect(() => {
		if (mode === 'edit' && skill && !showVersions) {
			void loadVersionCount(skill.id);
		} else if (mode === 'create') {
			versionCount = null;
		}
	});

	// Derived state
	let contentLength = $derived(content.length);
	let nameValid = $derived(/^[a-zA-Z0-9_-]*$/.test(name.trim()) && name.trim().length > 0);
	let isValid = $derived(nameValid && description.trim().length > 0 && content.trim().length > 0);

	// Category options for Select
	let categoryOptions = $derived(
		(Object.keys(SKILL_CATEGORY_I18N_KEYS) as SkillCategory[]).map((value) => ({
			value,
			label: t(SKILL_CATEGORY_I18N_KEYS[value])
		}))
	);

	let kindOptions = $derived([
		{ value: 'standard', label: t('skills_kind_standard') },
		{ value: 'kanban', label: t('skills_kind_kanban') }
	]);

	/**
	 * Handles form submission
	 */
	function handleSubmit(e: Event): void {
		e.preventDefault();
		if (!isValid || saving) return;

		onsave?.({
			name: name.trim(),
			description: description.trim(),
			category,
			content: content.trim(),
			kind: kind === 'kanban' ? 'kanban' : undefined
		});
	}

	/**
	 * Handles form cancellation
	 */
	function handleCancel(): void {
		oncancel?.();
	}
</script>

<form class="skill-form" onsubmit={handleSubmit}>
	<div class="form-field">
		<Input
			label={$i18n('skills_form_name_label')}
			value={name}
			oninput={(e) => (name = e.currentTarget.value)}
			placeholder={$i18n('skills_form_name_placeholder')}
			required
			disabled={saving}
		/>
		<div class="field-info">
			<span class="char-count">{name.length}/128</span>
			{#if name.trim().length > 0 && !nameValid}
				<span class="validation-error">{$i18n('skills_form_name_invalid')}</span>
			{/if}
		</div>
	</div>

	<div class="form-field">
		<Textarea
			label={$i18n('skills_form_description_label')}
			value={description}
			oninput={(e) => (description = e.currentTarget.value)}
			placeholder={$i18n('skills_form_description_placeholder')}
			rows={2}
			required
			disabled={saving}
		/>
		<span class="char-count">{description.length}/500</span>
	</div>

	<div class="form-field">
		<Select
			label={$i18n('skills_form_category_label')}
			value={category}
			onchange={(e) => (category = e.currentTarget.value as SkillCategory)}
			options={categoryOptions}
			disabled={saving}
		/>
	</div>

	<div class="form-field">
		<Select
			label={$i18n('skills_form_kind_label')}
			value={kind}
			onchange={(e) => (kind = e.currentTarget.value === 'kanban' ? 'kanban' : 'standard')}
			options={kindOptions}
			disabled={saving || mode === 'edit'}
		/>
		<span class="field-help">
			{mode === 'edit' ? $i18n('skills_form_kind_locked') : $i18n('skills_form_kind_help')}
		</span>
	</div>

	<div class="form-field">
		<Textarea
			label={$i18n('skills_form_content_label')}
			value={content}
			oninput={(e) => (content = e.currentTarget.value)}
			placeholder={$i18n('skills_form_content_placeholder')}
			rows={12}
			required
			disabled={saving}
		/>
		<span class="char-count">{contentLength.toLocaleString()}/50,000</span>
	</div>

	<div class="form-actions">
		{#if mode === 'edit' && skill}
			<Button
				type="button"
				variant="ghost"
				onclick={() => (showVersions = true)}
				disabled={saving || versionCount === 0}
				ariaLabel={$i18n('versions_history_button')}
			>
				{$i18n('versions_history_button')}
				{#if versionCount !== null && versionCount > 0}
					<Badge variant="primary">{versionCount}</Badge>
				{/if}
			</Button>
		{/if}
		<Button type="button" variant="ghost" onclick={handleCancel} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button type="submit" variant="primary" disabled={!isValid || saving}>
			{saving
				? $i18n('skills_saving')
				: mode === 'create'
					? $i18n('skills_create')
					: $i18n('skills_save_changes')}
		</Button>
	</div>
</form>

{#if showVersions && skill}
	<VersionsHistoryModal kind="skill" resourceId={skill.id} onclose={() => (showVersions = false)} />
{/if}

<style>
	.skill-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.field-info {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.char-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		text-align: right;
	}

	.field-help {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.validation-error {
		font-size: var(--font-size-xs);
		color: var(--color-error);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-md);
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}
</style>
