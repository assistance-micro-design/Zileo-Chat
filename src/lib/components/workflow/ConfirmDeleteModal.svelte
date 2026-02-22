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
  ConfirmDeleteModal Component
  A modal dialog for confirming workflow deletion using the standard Modal UI.

  @example
  <ConfirmDeleteModal
    open={showDeleteModal}
    workflowName="My Workflow"
    onconfirm={handleDeleteConfirm}
    oncancel={() => showDeleteModal = false}
  />
-->
<script lang="ts">
	import Modal from '$lib/components/ui/Modal.svelte';
	import { Button } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	/**
	 * ConfirmDeleteModal props
	 */
	interface Props {
		/** Whether the modal is open */
		open: boolean;
		/** Name of the workflow to delete */
		workflowName: string;
		/** Confirm deletion handler */
		onconfirm: () => void;
		/** Cancel handler */
		oncancel: () => void;
	}

	let { open, workflowName, onconfirm, oncancel }: Props = $props();

	let isDeleting = $state(false);

	/** Reset state when modal opens */
	$effect(() => {
		if (open) {
			isDeleting = false;
		}
	});

	/**
	 * Handle confirm click
	 */
	function handleConfirm(): void {
		isDeleting = true;
		onconfirm();
	}
</script>

<Modal open={open} title={$i18n('workflow_delete_title')} onclose={oncancel}>
	{#snippet body()}
		<p class="confirm-text">
			{$i18n('workflow_delete_confirm')} <strong class="workflow-name">"{workflowName}"</strong>?
		</p>
		<p class="delete-warning">
			{$i18n('workflow_delete_warning')}
		</p>
	{/snippet}
	{#snippet footer()}
		<Button variant="ghost" onclick={oncancel} disabled={isDeleting}>
			{$i18n('common_cancel')}
		</Button>
		<Button variant="danger" onclick={handleConfirm} disabled={isDeleting}>
			{isDeleting ? $i18n('workflow_deleting') : $i18n('workflow_delete_button')}
		</Button>
	{/snippet}
</Modal>

<style>
	.confirm-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		line-height: 1.5;
	}

	.workflow-name {
		color: var(--color-error);
		font-weight: var(--font-weight-semibold);
	}

	.delete-warning {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		margin-top: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		border-left: 3px solid var(--color-error);
	}
</style>
