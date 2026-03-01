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
SkillSettings - Container component for skill library management.
Provides CRUD operations for skills with list view and form modal.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		skillStore,
		skills,
		skillLoading,
		skillError,
		skillFormMode,
		editingSkill
	} from '$lib/stores/skills';
	import SkillList from './SkillList.svelte';
	import SkillForm from './SkillForm.svelte';
	import { Modal, ErrorBanner, DeleteConfirmModal } from '$lib/components/ui';
	import SettingsSectionHeader from '../SettingsSectionHeader.svelte';
	import type { SkillCreate } from '$types/skill';
	import { i18n } from '$lib/i18n';

	/** Form modal saving state */
	let saving = $state(false);

	/** Delete confirmation modal state */
	let showDeleteConfirm = $state(false);
	let skillToDelete = $state<string | null>(null);
	let deleting = $state(false);

	/**
	 * Loads skills on component mount
	 */
	onMount(() => {
		skillStore.loadSkills();
	});

	/**
	 * Opens the create skill form modal
	 */
	function handleCreate(): void {
		skillStore.openCreateForm();
	}

	/**
	 * Opens the edit form for a specific skill
	 */
	function handleEdit(skillId: string): void {
		skillStore.openEditForm(skillId);
	}

	/**
	 * Opens delete confirmation modal
	 */
	function handleDeleteRequest(skillId: string): void {
		skillToDelete = skillId;
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes skill deletion
	 */
	async function confirmDelete(): Promise<void> {
		if (!skillToDelete) return;

		deleting = true;
		try {
			await skillStore.deleteSkill(skillToDelete);
			showDeleteConfirm = false;
			skillToDelete = null;
		} finally {
			deleting = false;
		}
	}

	/**
	 * Cancels delete operation
	 */
	function cancelDelete(): void {
		showDeleteConfirm = false;
		skillToDelete = null;
	}

	/**
	 * Handles form save (create or update)
	 */
	async function handleSave(data: SkillCreate): Promise<void> {
		saving = true;
		try {
			if ($skillFormMode === 'create') {
				await skillStore.createSkill(data);
			} else if ($editingSkill) {
				await skillStore.updateSkill($editingSkill.id, data);
			}
		} catch {
			// Error state managed by skillStore (displayed via $skillError)
		} finally {
			saving = false;
		}
	}

	/**
	 * Handles skill enabled/disabled toggle
	 */
	async function handleToggleEnabled(skillId: string, enabled: boolean): Promise<void> {
		try {
			await skillStore.toggleEnabled(skillId, enabled);
		} catch {
			// Error state managed by skillStore
		}
	}

	/**
	 * Closes the form modal
	 */
	function handleFormClose(): void {
		skillStore.closeForm();
	}

	/**
	 * Clears the error message
	 */
	function handleDismissError(): void {
		skillStore.clearError();
	}
</script>

<div class="skill-settings">
	<!-- Shared settings section header -->
	<SettingsSectionHeader
		titleKey="skills_title"
		descriptionKey="skills_description"
		helpTitleKey="help_skills_title"
		helpDescriptionKey="help_skills_description"
		helpTutorialKey="help_skills_tutorial"
		createLabelKey="skills_create"
		onCreate={handleCreate}
	/>

	<!-- Shared error banner -->
	{#if $skillError}
		<ErrorBanner
			message={$skillError}
			onDismiss={handleDismissError}
			dismissLabel={$i18n('skills_dismiss')}
		/>
	{/if}

	<!-- Skill list (always visible) -->
	<SkillList
		skills={$skills}
		loading={$skillLoading}
		onedit={handleEdit}
		ondelete={handleDeleteRequest}
		ontoggle={handleToggleEnabled}
	/>
</div>

<!-- Create/Edit Form Modal -->
<Modal
	open={$skillFormMode !== null}
	title={$skillFormMode === 'create' ? $i18n('skills_create') : $i18n('skills_edit')}
	onclose={handleFormClose}
>
	{#snippet body()}
		<SkillForm
			mode={$skillFormMode ?? 'create'}
			skill={$editingSkill}
			{saving}
			onsave={handleSave}
			oncancel={handleFormClose}
		/>
	{/snippet}
</Modal>

<!-- Shared delete confirmation modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="skills_delete_title"
	confirmMessageKey="skills_delete_confirm"
	{deleting}
	deletingLabelKey="skills_deleting"
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
/>

<style>
	.skill-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}
</style>
