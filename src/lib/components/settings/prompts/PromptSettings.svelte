<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

PromptSettings - Container component for prompt library management.
Provides CRUD operations for prompts with list view and form modal.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		promptStore,
		prompts,
		promptLoading,
		promptError,
		promptFormMode,
		editingPrompt
	} from '$lib/stores/prompts';
	import PromptList from './PromptList.svelte';
	import PromptForm from './PromptForm.svelte';
	import { Button, Modal } from '$lib/components/ui';
	import { Plus } from 'lucide-svelte';
	import type { PromptCreate } from '$types/prompt';

	/** Form modal saving state */
	let saving = $state(false);

	/** Delete confirmation modal state */
	let showDeleteConfirm = $state(false);
	let promptToDelete = $state<string | null>(null);
	let deleting = $state(false);

	/**
	 * Loads prompts on component mount
	 */
	onMount(() => {
		promptStore.loadPrompts();
	});

	/**
	 * Opens the create prompt form modal
	 */
	function handleCreate(): void {
		promptStore.openCreateForm();
	}

	/**
	 * Opens the edit form for a specific prompt
	 */
	function handleEdit(promptId: string): void {
		promptStore.openEditForm(promptId);
	}

	/**
	 * Opens delete confirmation modal
	 */
	function handleDeleteRequest(promptId: string): void {
		promptToDelete = promptId;
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes prompt deletion
	 */
	async function confirmDelete(): Promise<void> {
		if (!promptToDelete) return;

		deleting = true;
		try {
			await promptStore.deletePrompt(promptToDelete);
			showDeleteConfirm = false;
			promptToDelete = null;
		} finally {
			deleting = false;
		}
	}

	/**
	 * Cancels delete operation
	 */
	function cancelDelete(): void {
		showDeleteConfirm = false;
		promptToDelete = null;
	}

	/**
	 * Handles form save (create or update)
	 */
	async function handleSave(data: PromptCreate): Promise<void> {
		saving = true;
		try {
			if ($promptFormMode === 'create') {
				await promptStore.createPrompt(data);
			} else if ($editingPrompt) {
				await promptStore.updatePrompt($editingPrompt.id, data);
			}
		} catch (e) {
			console.error('Failed to save prompt:', e);
		} finally {
			saving = false;
		}
	}

	/**
	 * Closes the form modal
	 */
	function handleFormClose(): void {
		promptStore.closeForm();
	}

	/**
	 * Clears the error message
	 */
	function handleDismissError(): void {
		promptStore.clearError();
	}
</script>

<div class="prompt-settings">
	<!-- Header with title and create button -->
	<header class="settings-header">
		<div class="header-content">
			<h3 class="header-title">Prompt Library</h3>
			<p class="header-description">
				Create and manage reusable prompt templates with variable placeholders.
			</p>
		</div>
		<Button variant="primary" size="sm" onclick={handleCreate}>
			<Plus size={16} />
			<span>Create Prompt</span>
		</Button>
	</header>

	<!-- Error display -->
	{#if $promptError}
		<div class="error-banner">
			<span class="error-text">{$promptError}</span>
			<button type="button" class="dismiss-btn" onclick={handleDismissError}>
				Dismiss
			</button>
		</div>
	{/if}

	<!-- Prompt list (always visible) -->
	<PromptList
		prompts={$prompts}
		loading={$promptLoading}
		onedit={handleEdit}
		ondelete={handleDeleteRequest}
	/>
</div>

<!-- Create/Edit Form Modal -->
<Modal
	open={$promptFormMode !== null}
	title={$promptFormMode === 'create' ? 'Create Prompt' : 'Edit Prompt'}
	onclose={handleFormClose}
>
	{#snippet body()}
		<PromptForm
			mode={$promptFormMode ?? 'create'}
			prompt={$editingPrompt}
			{saving}
			onsave={handleSave}
			oncancel={handleFormClose}
		/>
	{/snippet}
</Modal>

<!-- Delete confirmation modal -->
<Modal
	open={showDeleteConfirm}
	title="Delete Prompt"
	onclose={cancelDelete}
>
	{#snippet body()}
		<p class="confirm-text">
			Are you sure you want to delete this prompt? This action cannot be undone.
		</p>
	{/snippet}
	{#snippet footer()}
		<div class="modal-actions">
			<Button variant="ghost" onclick={cancelDelete} disabled={deleting}>
				Cancel
			</Button>
			<Button variant="danger" onclick={confirmDelete} disabled={deleting}>
				{deleting ? 'Deleting...' : 'Delete'}
			</Button>
		</div>
	{/snippet}
</Modal>

<style>
	.prompt-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.settings-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--spacing-lg);
	}

	.header-content {
		flex: 1;
	}

	.header-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0 0 var(--spacing-xs) 0;
	}

	.header-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.settings-header :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.error-banner {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
	}

	.error-text {
		font-size: var(--font-size-sm);
	}

	.dismiss-btn {
		background: transparent;
		border: none;
		color: var(--color-error);
		cursor: pointer;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-radius: var(--border-radius-sm);
	}

	.dismiss-btn:hover {
		background: rgba(0, 0, 0, 0.1);
	}

	.confirm-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		line-height: var(--line-height-relaxed);
	}

	.modal-actions {
		display: flex;
		gap: var(--spacing-sm);
		justify-content: flex-end;
	}
</style>
