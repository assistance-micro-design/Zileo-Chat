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
	import { Button, Input, Textarea, Select } from '$lib/components/ui';
	import type { Skill, SkillCreate, SkillCategory } from '$types/skill';
	import { SKILL_CATEGORY_I18N_KEYS } from '$types/skill';
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

	// Form state
	let name = $state('');
	let description = $state('');
	let category = $state<SkillCategory>('custom');
	let content = $state('');

	// Sync form state when skill prop changes (e.g., switching between edit targets)
	$effect(() => {
		name = skill?.name ?? '';
		description = skill?.description ?? '';
		category = skill?.category ?? 'custom';
		content = skill?.content ?? '';
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
			content: content.trim()
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
