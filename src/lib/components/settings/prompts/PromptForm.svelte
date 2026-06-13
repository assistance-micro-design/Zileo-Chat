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

PromptForm - Form component for creating and editing prompts.
Displays in a modal with variable detection and skill-reference insertion.
Version history is reached from the prompt list rows, not from this form.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriInvoke } from '$lib/tauri';
	import { Button, Input, Textarea, Select } from '$lib/components/ui';
	import type { Prompt, PromptCreate, PromptCategory } from '$types/prompt';
	import { PROMPT_CATEGORY_I18N_KEYS } from '$types/prompt';
	import type { SkillSummary } from '$types/skill';
	import { extractVariables, extractSkillReferences } from '$lib/stores/prompts';
	import { i18n, t } from '$lib/i18n';

	/**
	 * Component props
	 */
	interface Props {
		/** Form mode - create or edit */
		mode: 'create' | 'edit';
		/** Existing prompt data for edit mode */
		prompt?: Prompt | null;
		/** Whether the form is currently saving */
		saving?: boolean;
		/** Callback when form is submitted */
		onsave?: (data: PromptCreate) => void;
		/** Callback when form is cancelled */
		oncancel?: () => void;
	}

	let { mode, prompt = null, saving = false, onsave, oncancel }: Props = $props();

	// Form state
	let name = $state('');
	let description = $state('');
	let category = $state<PromptCategory>('custom');
	let content = $state('');

	// Skills state for insertion
	let availableSkills = $state<SkillSummary[]>([]);
	let contentTextarea = $state<HTMLTextAreaElement | null>(null);
	const contentTextareaId = 'prompt-content-textarea';

	onMount(async () => {
		contentTextarea = document.getElementById(contentTextareaId) as HTMLTextAreaElement | null;
		try {
			const skills = await tauriInvoke<SkillSummary[]>('list_skills');
			availableSkills = skills.filter((s) => s.enabled);
		} catch {
			// Non-blocking: skill insertion is an optional feature.
			// If loading fails, the insertion dropdown simply won't appear.
		}
	});

	// Sync form state when prompt prop changes (e.g., switching between edit targets)
	$effect(() => {
		name = prompt?.name ?? '';
		description = prompt?.description ?? '';
		category = prompt?.category ?? 'custom';
		content = prompt?.content ?? '';
	});

	// Derived state
	let detectedVariables = $derived(extractVariables(content));
	let detectedSkills = $derived(extractSkillReferences(content));
	let contentLength = $derived(content.length);
	let isValid = $derived(name.trim().length > 0 && content.trim().length > 0);

	/** Skills available for insertion (not already referenced) */
	let insertableSkills = $derived(availableSkills.filter((s) => !detectedSkills.includes(s.name)));

	// Category options for Select
	let categoryOptions = $derived(
		(Object.keys(PROMPT_CATEGORY_I18N_KEYS) as PromptCategory[]).map((value) => ({
			value,
			label: t(PROMPT_CATEGORY_I18N_KEYS[value])
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

	/**
	 * Insert a skill reference at the cursor position in the content textarea
	 */
	function insertSkillReference(skillName: string): void {
		const ref = `{{skill:${skillName}}}`;
		if (contentTextarea) {
			const textarea = contentTextarea;
			const start = textarea.selectionStart;
			const end = textarea.selectionEnd;
			content = content.substring(0, start) + ref + content.substring(end);
			// Restore focus and cursor position after the inserted text
			requestAnimationFrame(() => {
				textarea.focus();
				const newPos = start + ref.length;
				textarea.setSelectionRange(newPos, newPos);
			});
		} else {
			// Fallback: append at end
			content += (content.length > 0 && !content.endsWith('\n') ? '\n' : '') + ref;
		}
	}
</script>

<form class="prompt-form" onsubmit={handleSubmit}>
	<div class="form-row">
		<Input
			label={$i18n('prompts_form_name_label')}
			value={name}
			oninput={(e) => (name = e.currentTarget.value)}
			placeholder={$i18n('prompts_form_name_placeholder')}
			required
			disabled={saving}
		/>

		<Select
			label={$i18n('prompts_form_category_label')}
			value={category}
			onchange={(e) => (category = e.currentTarget.value as PromptCategory)}
			options={categoryOptions}
			disabled={saving}
		/>
	</div>

	<Textarea
		label={$i18n('prompts_form_description_label')}
		value={description}
		oninput={(e) => (description = e.currentTarget.value)}
		placeholder={$i18n('prompts_form_description_placeholder')}
		rows={2}
		disabled={saving}
	/>

	<div class="form-field">
		<Textarea
			id={contentTextareaId}
			label={$i18n('prompts_form_content_label')}
			value={content}
			oninput={(e) => (content = e.currentTarget.value)}
			placeholder={$i18n('prompts_form_content_placeholder')}
			rows={6}
			required
			disabled={saving}
			help={$i18n('prompts_form_content_help')}
		/>
		<div class="content-toolbar">
			{#if insertableSkills.length > 0}
				<select
					class="skill-select"
					onchange={(e) => {
						const val = e.currentTarget.value;
						if (val) {
							insertSkillReference(val);
							e.currentTarget.value = '';
						}
					}}
					disabled={saving}
					aria-label={$i18n('prompts_insert_skill')}
				>
					<option value="">{$i18n('prompts_insert_skill')}</option>
					{#each insertableSkills as skill (skill.id)}
						<option value={skill.name}>{skill.name}</option>
					{/each}
				</select>
			{/if}
			<span class="char-count">{contentLength.toLocaleString()}/50,000</span>
		</div>

		{#if detectedVariables.length > 0 || detectedSkills.length > 0}
			<div class="detected">
				{#if detectedVariables.length > 0}
					<div class="detected-group">
						<span class="detected-label">
							{$i18n('prompts_detected_variables')}
							<span class="detected-count">{detectedVariables.length}</span>
						</span>
						<div class="chips">
							{#each detectedVariables as variable (variable)}
								<span class="badge badge-primary mono-badge">{variable}</span>
							{/each}
						</div>
					</div>
				{/if}
				{#if detectedSkills.length > 0}
					<div class="detected-group">
						<span class="detected-label">
							{$i18n('prompts_detected_skills')}
							<span class="detected-count">{detectedSkills.length}</span>
						</span>
						<div class="chips">
							{#each detectedSkills as skill (skill)}
								<span class="badge badge-success mono-badge">{skill}</span>
							{/each}
						</div>
					</div>
				{/if}
			</div>
		{/if}
	</div>

	<div class="form-actions">
		<Button type="button" variant="ghost" onclick={handleCancel} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button type="submit" variant="primary" disabled={!isValid || saving}>
			{saving
				? $i18n('prompts_saving')
				: mode === 'create'
					? $i18n('prompts_create')
					: $i18n('prompts_save_changes')}
		</Button>
	</div>
</form>

<style>
	.prompt-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.form-row {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
		align-items: start;
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	/* Toolbar under the content field: skill inserter (left) + char count
	   (right). Decoupled from the detected chips so its alignment never shifts
	   with the number of variables. */
	.content-toolbar {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.char-count {
		margin-left: auto;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-variant-numeric: tabular-nums;
	}

	.mono-badge {
		font-family: var(--font-mono);
		font-weight: var(--font-weight-medium);
	}

	/* Detected variables / skills: a tidy bordered panel that groups each kind
	   under a labelled heading with a count. Capped height keeps the form usable
	   even when a prompt declares many variables. */
	.detected {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		max-height: 11rem;
		overflow-y: auto;
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--surface-2);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
	}

	.detected-group {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
	}

	.detected-label {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-2xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.detected-count {
		min-width: 1.25rem;
		padding: 0 var(--spacing-2xs);
		text-align: center;
		font-size: var(--font-size-2xs);
		font-variant-numeric: tabular-nums;
		color: var(--color-text-secondary);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-full);
	}

	.chips {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-2xs);
	}

	.skill-select {
		font-size: var(--font-size-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-sm);
		background: var(--color-bg-primary);
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.skill-select:hover {
		border-color: var(--color-accent);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-md);
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	@media (max-width: 640px) {
		.form-row {
			grid-template-columns: 1fr;
		}
	}
</style>
