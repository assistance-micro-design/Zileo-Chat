<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

PromptForm - Form component for creating and editing prompts.
Displays in a modal with variable detection and preview.
-->

<script lang="ts">
	import { Button, Input, Textarea, Select, Badge } from '$lib/components/ui';
	import type { Prompt, PromptCreate, PromptCategory } from '$types/prompt';
	import { PROMPT_CATEGORY_LABELS } from '$types/prompt';
	import { extractVariables } from '$lib/stores/prompts';

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

	// Category options for Select
	const categoryOptions = Object.entries(PROMPT_CATEGORY_LABELS).map(([value, label]) => ({
		value: value as PromptCategory,
		label
	}));

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
			label="Name"
			value={name}
			oninput={(e) => (name = e.currentTarget.value)}
			placeholder="Enter prompt name"
			required
			disabled={saving}
		/>
		<span class="char-count">{name.length}/128</span>
	</div>

	<div class="form-field">
		<Textarea
			label="Description"
			value={description}
			oninput={(e) => (description = e.currentTarget.value)}
			placeholder="Brief description of this prompt"
			rows={2}
			disabled={saving}
		/>
		<span class="char-count">{description.length}/1000</span>
	</div>

	<div class="form-field">
		<Select
			label="Category"
			value={category}
			onchange={(e) => (category = e.currentTarget.value as PromptCategory)}
			options={categoryOptions}
			disabled={saving}
		/>
	</div>

	<div class="form-field">
		<Textarea
			label="Content"
			value={content}
			oninput={(e) => (content = e.currentTarget.value)}
			placeholder="Enter prompt content. Use &#123;&#123;variable_name&#125;&#125; for placeholders."
			rows={8}
			required
			disabled={saving}
		/>
		<span class="char-count">{contentLength.toLocaleString()}/50,000</span>
	</div>

	{#if detectedVariables.length > 0}
		<div class="variables-section">
			<span class="variables-label">Detected Variables:</span>
			<div class="variables-list">
				{#each detectedVariables as variable}
					<Badge variant="primary">{variable}</Badge>
				{/each}
			</div>
		</div>
	{/if}

	<div class="form-actions">
		<Button type="button" variant="ghost" onclick={handleCancel} disabled={saving}>
			Cancel
		</Button>
		<Button type="submit" variant="primary" disabled={!isValid || saving}>
			{saving ? 'Saving...' : mode === 'create' ? 'Create Prompt' : 'Save Changes'}
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
