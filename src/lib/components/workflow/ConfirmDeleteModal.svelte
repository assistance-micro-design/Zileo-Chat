<!--
  ConfirmDeleteModal Component
  A modal dialog for confirming workflow deletion with danger styling.

  @example
  <ConfirmDeleteModal
    open={showDeleteModal}
    workflowName="My Workflow"
    onconfirm={handleDeleteConfirm}
    oncancel={() => showDeleteModal = false}
  />
-->
<script lang="ts">
	import { AlertTriangle, X } from 'lucide-svelte';
	import { Button } from '$lib/components/ui';

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

	/**
	 * Handle keyboard events for accessibility
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			oncancel();
		}
	}

	/**
	 * Handle backdrop click to close modal
	 */
	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			oncancel();
		}
	}
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
	<div class="modal-backdrop" role="presentation" onclick={handleBackdropClick} onkeydown={handleKeydown}>
		<div class="modal delete-modal" role="alertdialog" aria-modal="true" aria-labelledby="modal-title" aria-describedby="modal-description">
			<div class="modal-header">
				<div class="modal-title-wrapper">
					<div class="icon-wrapper">
						<AlertTriangle size={24} class="modal-icon" />
					</div>
					<h3 id="modal-title" class="modal-title">Delete Workflow</h3>
				</div>
				<button type="button" class="close-button" onclick={oncancel} aria-label="Close" disabled={isDeleting}>
					<X size={20} />
				</button>
			</div>

			<div class="modal-body">
				<p id="modal-description" class="delete-message">
					Are you sure you want to delete <strong class="workflow-name">"{workflowName}"</strong>?
				</p>
				<p class="delete-warning">
					This action cannot be undone. All messages, tool history, and reasoning steps will be permanently removed.
				</p>
			</div>

			<div class="modal-footer">
				<Button
					variant="ghost"
					onclick={oncancel}
					disabled={isDeleting}
				>
					Cancel
				</Button>
				<Button
					variant="danger"
					onclick={handleConfirm}
					disabled={isDeleting}
				>
					{#if isDeleting}
						Deleting...
					{:else}
						Delete Workflow
					{/if}
				</Button>
			</div>
		</div>
	</div>
{/if}

<style>
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.6);
		backdrop-filter: blur(4px);
		z-index: var(--z-index-modal-backdrop);
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xl);
		animation: fadeIn 0.15s ease-out;
	}

	@keyframes fadeIn {
		from { opacity: 0; }
		to { opacity: 1; }
	}

	.delete-modal {
		background: var(--color-bg-primary);
		border-radius: var(--border-radius-xl);
		box-shadow: var(--shadow-xl);
		width: 100%;
		max-width: 420px;
		overflow: hidden;
		animation: slideUp 0.2s ease-out;
	}

	@keyframes slideUp {
		from {
			opacity: 0;
			transform: translateY(20px) scale(0.98);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}

	.modal-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: linear-gradient(180deg, var(--color-error-light) 0%, var(--color-bg-primary) 100%);
	}

	.modal-title-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.icon-wrapper {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		background: var(--color-error-light);
		border-radius: var(--border-radius-md);
	}

	.icon-wrapper :global(.modal-icon) {
		color: var(--color-error);
	}

	.modal-title {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.close-button {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 36px;
		height: 36px;
		background: transparent;
		border: none;
		border-radius: var(--border-radius-md);
		color: var(--color-text-tertiary);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.close-button:hover:not(:disabled) {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.close-button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.modal-body {
		padding: var(--spacing-lg);
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.delete-message {
		font-size: var(--font-size-base);
		color: var(--color-text-primary);
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
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		border-left: 3px solid var(--color-error);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}
</style>
