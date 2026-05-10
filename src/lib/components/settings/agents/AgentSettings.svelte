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
	import { tauriInvoke } from '$lib/tauri';
	import { agentStore, agents, isLoading, error, formMode, editingAgent } from '$lib/stores/agents';
	import type { ProviderInfo } from '$types/custom-provider';
	import AgentList from './AgentList.svelte';
	import AgentForm from './AgentForm.svelte';
	import { ErrorBanner, DeleteConfirmModal } from '$lib/components/ui';
	import SettingsSectionHeader from '../SettingsSectionHeader.svelte';

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

	/** Provider ID -> display name mapping for AgentList */
	let providerNames = $state<Record<string, string>>({});

	/**
	 * Loads agents and provider names on component mount
	 */
	onMount(async () => {
		agentStore.loadAgents();
		try {
			const providers = await tauriInvoke<ProviderInfo[]>('list_providers');
			providerNames = Object.fromEntries(providers.map((p) => [p.id, p.displayName]));
		} catch {
			// Non-blocking: provider names are cosmetic, fallback to raw ID
		}
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
	<!-- Shared settings section header -->
	<SettingsSectionHeader
		titleKey="agents_config_title"
		descriptionKey="agents_config_description"
		helpTitleKey="help_agents_title"
		helpDescriptionKey="help_agents_description"
		helpTutorialKey="help_agents_tutorial"
		createLabelKey="agents_create"
		onCreate={handleCreate}
	/>

	<!-- Shared error banner -->
	{#if $error}
		<ErrorBanner message={$error} onDismiss={handleDismissError} />
	{/if}

	<!-- Agent list or form -->
	{#if $formMode}
		<AgentForm mode={$formMode} agent={$editingAgent} oncancel={handleFormClose} />
	{:else}
		<AgentList
			agents={$agents}
			loading={$isLoading}
			{providerNames}
			onedit={handleEdit}
			ondelete={handleDeleteRequest}
		/>
	{/if}
</div>

<!-- Shared delete confirmation modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="agents_delete_title"
	confirmMessageKey="agents_delete_confirm"
	{deleting}
	deletingLabelKey="agents_deleting"
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
/>

<style>
	.agent-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}
</style>
