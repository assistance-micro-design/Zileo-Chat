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

AgentSettings - Container component for agent management.
Provides CRUD operations for agents with list view and form modal.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import {
		agentStore,
		agents,
		isLoading,
		error,
		formMode,
		editingAgent
	} from '$lib/stores/agents';
	import AgentList from './AgentList.svelte';
	import AgentForm from './AgentForm.svelte';
	import { Button, Modal, HelpButton } from '$lib/components/ui';
	import { Plus } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Component props
	 */
	interface Props {
		/**
		 * Refresh trigger - increment this value to force a reload of agents.
		 * Used after import operations to ensure UI updates with new data.
		 */
		refreshTrigger?: number;
	}

	let { refreshTrigger = 0 }: Props = $props();

	/** Delete confirmation modal state */
	let showDeleteConfirm = $state(false);
	let agentToDelete = $state<string | null>(null);
	let deleting = $state(false);

	/**
	 * Loads agents on component mount
	 */
	onMount(() => {
		agentStore.loadAgents();
	});

	/**
	 * Watch for external refresh triggers (e.g., after import).
	 * This ensures the agent list updates when refreshTrigger changes.
	 */
	$effect(() => {
		// Track refreshTrigger changes
		const trigger = refreshTrigger;
		// Skip initial mount (onMount already handles that)
		if (trigger > 0) {
			agentStore.loadAgents();
		}
	});

	/**
	 * Opens the create agent form
	 */
	function handleCreate(): void {
		agentStore.openCreateForm();
	}

	/**
	 * Opens the edit form for a specific agent
	 */
	function handleEdit(agentId: string): void {
		agentStore.openEditForm(agentId);
	}

	/**
	 * Opens delete confirmation modal
	 */
	function handleDeleteRequest(agentId: string): void {
		agentToDelete = agentId;
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes agent deletion
	 */
	async function confirmDelete(): Promise<void> {
		if (!agentToDelete) return;

		deleting = true;
		try {
			await agentStore.deleteAgent(agentToDelete);
			showDeleteConfirm = false;
			agentToDelete = null;
		} finally {
			deleting = false;
		}
	}

	/**
	 * Cancels delete operation
	 */
	function cancelDelete(): void {
		showDeleteConfirm = false;
		agentToDelete = null;
	}

	/**
	 * Closes the form modal
	 */
	function handleFormClose(): void {
		agentStore.closeForm();
	}

	/**
	 * Clears the error message
	 */
	function handleDismissError(): void {
		agentStore.clearError();
	}
</script>

<div class="agent-settings">
	<!-- Header with title and create button -->
	<header class="settings-header">
		<div class="header-content">
			<div class="header-title-row">
				<h3 class="header-title">{$i18n('agents_config_title')}</h3>
				<HelpButton
					titleKey="help_agents_title"
					descriptionKey="help_agents_description"
					tutorialKey="help_agents_tutorial"
				/>
			</div>
			<p class="header-description">
				{$i18n('agents_config_description')}
			</p>
		</div>
		<Button variant="primary" size="sm" onclick={handleCreate}>
			<Plus size={16} />
			<span>{$i18n('agents_create')}</span>
		</Button>
	</header>

	<!-- Error display -->
	{#if $error}
		<div class="error-banner">
			<span class="error-text">{$error}</span>
			<button type="button" class="dismiss-btn" onclick={handleDismissError}>
				{$i18n('common_close')}
			</button>
		</div>
	{/if}

	<!-- Agent list or form -->
	{#if $formMode}
		<AgentForm
			mode={$formMode}
			agent={$editingAgent}
			oncancel={handleFormClose}
		/>
	{:else}
		<AgentList
			agents={$agents}
			loading={$isLoading}
			onedit={handleEdit}
			ondelete={handleDeleteRequest}
		/>
	{/if}
</div>

<!-- Delete confirmation modal -->
<Modal
	open={showDeleteConfirm}
	title={$i18n('agents_delete_title')}
	onclose={cancelDelete}
>
	{#snippet body()}
		<p class="confirm-text">
			{$i18n('agents_delete_confirm')}
		</p>
	{/snippet}
	{#snippet footer()}
		<div class="modal-actions">
			<Button variant="ghost" onclick={cancelDelete} disabled={deleting}>
				{$i18n('common_cancel')}
			</Button>
			<Button variant="danger" onclick={confirmDelete} disabled={deleting}>
				{deleting ? $i18n('agents_deleting') : $i18n('common_delete')}
			</Button>
		</div>
	{/snippet}
</Modal>

<style>
	.agent-settings {
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

	.header-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.header-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
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
