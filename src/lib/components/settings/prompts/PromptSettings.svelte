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
	import { Modal, ErrorBanner, DeleteConfirmModal } from '$lib/components/ui';
	import SettingsSectionHeader from '../SettingsSectionHeader.svelte';
	import type { PromptCreate } from '$types/prompt';
	import { i18n } from '$lib/i18n';
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
		} catch {
			// Error state managed by promptStore (displayed via $promptError)
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
	<!-- Shared settings section header -->
	<SettingsSectionHeader
		titleKey="prompts_title"
		descriptionKey="prompts_description"
		helpTitleKey="help_prompts_title"
		helpDescriptionKey="help_prompts_description"
		helpTutorialKey="help_prompts_tutorial"
		createLabelKey="prompts_create"
		onCreate={handleCreate}
	/>

	<!-- Shared error banner -->
	{#if $promptError}
		<ErrorBanner
			message={$promptError}
			onDismiss={handleDismissError}
			dismissLabel={$i18n('prompts_dismiss')}
		/>
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
	title={$promptFormMode === 'create' ? $i18n('prompts_create') : $i18n('prompts_edit')}
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

<!-- Shared delete confirmation modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="prompts_delete_title"
	confirmMessageKey="prompts_delete_confirm"
	{deleting}
	deletingLabelKey="prompts_deleting"
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
/>

<style>
	.prompt-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}
</style>
