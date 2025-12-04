<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

MemoryForm - Form for adding/editing memory entries.
Provides fields for memory type, content, and metadata.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button, Input, Select, Textarea } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type { Memory, MemoryType } from '$types/memory';
	import { i18n, t } from '$lib/i18n';

	/** Props */
	interface Props {
		/** Form mode: 'add' or 'edit' */
		mode: 'add' | 'edit';
		/** Memory to edit (for edit mode) */
		memory?: Memory;
		/** Callback when form is saved */
		onsave?: () => void;
		/** Callback when form is cancelled */
		oncancel?: () => void;
	}

	let { mode, memory, onsave, oncancel }: Props = $props();

	/** Form state */
	let formData = $state({
		type: 'knowledge' as MemoryType,
		content: '',
		tags: '',
		priority: 0.5
	});

	/** UI state */
	let saving = $state(false);
	let error = $state<string | null>(null);

	/** Memory type options (reactive to locale) */
	const typeOptions = $derived<SelectOption[]>([
		{ value: 'user_pref', label: t('memory_type_user_pref') },
		{ value: 'context', label: t('memory_type_context') },
		{ value: 'knowledge', label: t('memory_type_knowledge') },
		{ value: 'decision', label: t('memory_type_decision') }
	]);

	/**
	 * Initialize form data when memory prop changes
	 */
	$effect(() => {
		if (memory && mode === 'edit') {
			const tags = memory.metadata?.tags;
			formData = {
				type: (memory.type as MemoryType) || 'knowledge',
				content: memory.content || '',
				tags: Array.isArray(tags) ? tags.join(', ') : '',
				priority: (memory.metadata?.priority as number) || 0.5
			};
		} else {
			formData = {
				type: 'knowledge',
				content: '',
				tags: '',
				priority: 0.5
			};
		}
	});

	/**
	 * Handles type change
	 */
	function handleTypeChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		formData.type = event.currentTarget.value as MemoryType;
	}

	/**
	 * Handles form submission
	 */
	async function handleSubmit(): Promise<void> {
		error = null;

		// Validate content
		if (!formData.content.trim()) {
			error = t('memory_form_error_required');
			return;
		}

		if (formData.content.length > 50000) {
			error = t('memory_form_error_max_length');
			return;
		}

		saving = true;

		try {
			// Build metadata
			const metadata: Record<string, unknown> = {};
			if (formData.tags.trim()) {
				metadata.tags = formData.tags.split(',').map((tag) => tag.trim()).filter(Boolean);
			}
			if (formData.priority !== 0.5) {
				metadata.priority = formData.priority;
			}

			if (mode === 'add') {
				// Add new memory
				await invoke<string>('add_memory', {
					memoryType: formData.type,
					content: formData.content.trim(),
					metadata: Object.keys(metadata).length > 0 ? metadata : null
				});
			} else if (memory) {
				// Update existing memory
				await invoke<Memory>('update_memory', {
					memoryId: memory.id,
					content: formData.content.trim(),
					metadata: Object.keys(metadata).length > 0 ? metadata : null
				});
			}

			onsave?.();
		} catch (err) {
			error = t('memory_failed_save').replace('{error}', String(err));
		} finally {
			saving = false;
		}
	}

	/**
	 * Handles cancel
	 */
	function handleCancel(): void {
		oncancel?.();
	}
</script>

<form class="memory-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
	{#if error}
		<div class="error-message">
			{error}
		</div>
	{/if}

	<Select
		label={$i18n('memory_form_type_label')}
		options={typeOptions}
		value={formData.type}
		onchange={handleTypeChange}
		help={$i18n('memory_form_type_help')}
	/>

	<Textarea
		label={$i18n('memory_form_content_label')}
		placeholder={$i18n('memory_form_content_placeholder')}
		value={formData.content}
		oninput={(e: Event & { currentTarget: HTMLTextAreaElement }) => formData.content = e.currentTarget.value}
		rows={6}
		help={$i18n('memory_form_content_help')}
	/>

	<Input
		type="text"
		label={$i18n('memory_form_tags_label')}
		placeholder={$i18n('memory_form_tags_placeholder')}
		value={formData.tags}
		oninput={(e: Event & { currentTarget: HTMLInputElement }) => formData.tags = e.currentTarget.value}
		help={$i18n('memory_form_tags_help')}
	/>

	<div class="slider-field">
		<span class="slider-label">
			{$i18n('memory_form_priority_label').replace('{value}', formData.priority.toFixed(1))}
		</span>
		<input
			type="range"
			min="0"
			max="1"
			step="0.1"
			bind:value={formData.priority}
			class="slider"
			aria-label={$i18n('memory_form_priority_label').replace('{value}', '')}
		/>
		<span class="slider-help">{$i18n('memory_form_priority_help')}</span>
	</div>

	<div class="form-actions">
		<Button
			type="button"
			variant="ghost"
			onclick={handleCancel}
			disabled={saving}
		>
			{$i18n('common_cancel')}
		</Button>
		<Button
			type="submit"
			variant="primary"
			disabled={saving || !formData.content.trim()}
		>
			{saving ? $i18n('common_saving') : mode === 'add' ? $i18n('memory_modal_add') : $i18n('memory_form_save_changes')}
		</Button>
	</div>
</form>

<style>
	.memory-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.error-message {
		padding: var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.slider-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.slider-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.slider {
		width: 100%;
		height: 8px;
		border-radius: 4px;
		background: var(--color-bg-tertiary);
		outline: none;
		cursor: pointer;
	}

	.slider::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: var(--color-accent);
		cursor: pointer;
	}

	.slider::-moz-range-thumb {
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: var(--color-accent);
		cursor: pointer;
		border: none;
	}

	.slider-help {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		margin-top: var(--spacing-md);
	}
</style>
