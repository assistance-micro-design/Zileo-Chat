<!--
  Modal Component
  A dialog modal with backdrop, header, body, and footer.
  Includes keyboard accessibility (Escape to close).

  @example
  <Modal open={showModal} title="Confirm Action" onclose={() => showModal = false}>
    {#snippet body()}
      <p>Are you sure you want to proceed?</p>
    {/snippet}
    {#snippet footer()}
      <Button variant="ghost" onclick={() => showModal = false}>Cancel</Button>
      <Button onclick={handleConfirm}>Confirm</Button>
    {/snippet}
  </Modal>
-->
<script lang="ts">
	import { X } from 'lucide-svelte';
	import type { Snippet } from 'svelte';

	/**
	 * Modal props
	 */
	interface Props {
		/** Whether the modal is open */
		open: boolean;
		/** Modal title */
		title: string;
		/** Close handler */
		onclose: () => void;
		/** Modal body content */
		body?: Snippet;
		/** Modal footer content */
		footer?: Snippet;
	}

	let { open, title, onclose, body, footer }: Props = $props();

	/**
	 * Handle keyboard events for accessibility
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			onclose();
		}
	}

	/**
	 * Handle backdrop click to close modal
	 */
	function handleBackdropClick(event: MouseEvent): void {
		if (event.target === event.currentTarget) {
			onclose();
		}
	}
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
	<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
	<div class="modal-backdrop" role="presentation" onclick={handleBackdropClick}>
		<div class="modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
			<div class="modal-header">
				<h3 id="modal-title" class="modal-title">{title}</h3>
				<button type="button" class="btn btn-ghost btn-icon" onclick={onclose} aria-label="Close">
					<X size={20} />
				</button>
			</div>

			{#if body}
				<div class="modal-body">
					{@render body()}
				</div>
			{/if}

			{#if footer}
				<div class="modal-footer">
					{@render footer()}
				</div>
			{/if}
		</div>
	</div>
{/if}
