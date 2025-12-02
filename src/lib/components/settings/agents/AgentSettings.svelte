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
	import { Button, Modal } from '$lib/components/ui';
	import { Plus } from 'lucide-svelte';

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
			<h3 class="header-title">Agent Configuration</h3>
			<p class="header-description">
				Create and manage AI agents with custom tools and MCP server access.
			</p>
		</div>
		<Button variant="primary" size="sm" onclick={handleCreate}>
			<Plus size={16} />
			<span>Create Agent</span>
		</Button>
	</header>

	<!-- Error display -->
	{#if $error}
		<div class="error-banner">
			<span class="error-text">{$error}</span>
			<button type="button" class="dismiss-btn" onclick={handleDismissError}>
				Dismiss
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
	title="Delete Agent"
	onclose={cancelDelete}
>
	{#snippet body()}
		<p class="confirm-text">
			Are you sure you want to delete this agent? This action cannot be undone.
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
