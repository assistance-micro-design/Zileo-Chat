<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

PromptForm - Form component for creating and editing prompts.
Displays in a modal with variable detection and preview.
-->

<script lang="ts">
	import { Button, Input, Textarea, Select, Badge } from '$lib/components/ui';
	import type { Prompt, PromptCreate, PromptCategory } from '$types/prompt';
	import { extractVariables } from '$lib/stores/prompts';
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
	let name = $state(prompt?.name ?? '');
	let description = $state(prompt?.description ?? '');
	let category = $state<PromptCategory>(prompt?.category ?? 'custom');
	let content = $state(prompt?.content ?? '');

	// Derived state
	let detectedVariables = $derived(extractVariables(content));
	let contentLength = $derived(content.length);
	let isValid = $derived(name.trim().length > 0 && content.trim().length > 0);

	// Category labels mapping for i18n
	const categoryI18nKeys: Record<PromptCategory, string> = {
		system: 'prompts_category_system',
		user: 'prompts_category_user',
		analysis: 'prompts_category_analysis',
		generation: 'prompts_category_generation',
		coding: 'prompts_category_coding',
		custom: 'prompts_category_custom'
	};

	// Category options for Select
	let categoryOptions = $derived(
		(['system', 'user', 'analysis', 'generation', 'coding', 'custom'] as PromptCategory[]).map((value) => ({
			value,
			label: t(categoryI18nKeys[value])
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

	// Reset form when prompt changes (for edit mode)
	$effect(() => {
		if (prompt) {
			name = prompt.name;
			description = prompt.description;
			category = prompt.category;
			content = prompt.content;
		}
	});
</script>

<form class="prompt-form" onsubmit={handleSubmit}>
	<div class="form-field">
		<Input
			label={$i18n('prompts_form_name_label')}
			value={name}
			oninput={(e) => (name = e.currentTarget.value)}
			placeholder={$i18n('prompts_form_name_placeholder')}
			required
			disabled={saving}
		/>
		<span class="char-count">{name.length}/128</span>
	</div>

	<div class="form-field">
		<Textarea
			label={$i18n('prompts_form_description_label')}
			value={description}
			oninput={(e) => (description = e.currentTarget.value)}
			placeholder={$i18n('prompts_form_description_placeholder')}
			rows={2}
			disabled={saving}
		/>
		<span class="char-count">{description.length}/1000</span>
	</div>

	<div class="form-field">
		<Select
			label={$i18n('prompts_form_category_label')}
			value={category}
			onchange={(e) => (category = e.currentTarget.value as PromptCategory)}
			options={categoryOptions}
			disabled={saving}
		/>
	</div>

	<div class="form-field">
		<Textarea
			label={$i18n('prompts_form_content_label')}
			value={content}
			oninput={(e) => (content = e.currentTarget.value)}
			placeholder={$i18n('prompts_form_content_placeholder')}
			rows={8}
			required
			disabled={saving}
		/>
		<span class="char-count">{contentLength.toLocaleString()}/50,000</span>
	</div>

	{#if detectedVariables.length > 0}
		<div class="variables-section">
			<span class="variables-label">{$i18n('prompts_detected_variables')}</span>
			<div class="variables-list">
				{#each detectedVariables as variable}
					<Badge variant="primary">{variable}</Badge>
				{/each}
			</div>
		</div>
	{/if}

	<div class="form-actions">
		<Button type="button" variant="ghost" onclick={handleCancel} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button type="submit" variant="primary" disabled={!isValid || saving}>
			{saving ? $i18n('prompts_saving') : mode === 'create' ? $i18n('prompts_create') : $i18n('prompts_save_changes')}
		</Button>
	</div>
</form>

<style>
	.prompt-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.form-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.char-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		text-align: right;
	}

	.variables-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.variables-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.variables-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
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
