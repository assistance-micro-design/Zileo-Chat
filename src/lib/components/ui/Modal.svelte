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
	import { i18n } from '$lib/i18n';
	import { X } from '@lucide/svelte';
	import type { Snippet } from 'svelte';
	import { focusTrap } from '$lib/actions/focusTrap';

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
		/**
		 * When true, the modal fills the viewport (minus a small margin) instead
		 * of the default 600px / 90vh box. Used for content-heavy modals such as
		 * the Kanban card review chat.
		 */
		fullscreen?: boolean;
		/**
		 * When true, the modal widens to 860px for multi-column forms (e.g. the
		 * model editor's 3-column rows). Ignored when fullscreen is set.
		 */
		wide?: boolean;
		/**
		 * When true, the backdrop stacks above popover-level overlays. Set this
		 * for a modal rendered from within another already-open modal that was
		 * itself elevated (e.g. the delete confirmation inside
		 * VersionsHistoryModal); otherwise the nested dialog renders behind its
		 * parent and stays invisible.
		 */
		elevated?: boolean;
		/**
		 * Whether clicking the backdrop closes the modal (default true). Set
		 * false for fail-safe modals that must not be dismissed without an
		 * explicit decision (e.g. the human-in-the-loop ValidationModal).
		 */
		closeOnBackdrop?: boolean;
		/** Whether pressing Escape closes the modal (default true). */
		closeOnEscape?: boolean;
		/** Whether the header shows the close (X) button (default true). */
		showCloseButton?: boolean;
	}

	let {
		open,
		title,
		onclose,
		body,
		footer,
		fullscreen = false,
		wide = false,
		elevated = false,
		closeOnBackdrop = true,
		closeOnEscape = true,
		showCloseButton = true
	}: Props = $props();

	/**
	 * Handle keyboard events for accessibility
	 */
	function handleKeydown(event: KeyboardEvent): void {
		if (closeOnEscape && event.key === 'Escape') {
			onclose();
		}
	}

	/**
	 * Handle backdrop click to close modal
	 */
	function handleBackdropClick(event: MouseEvent): void {
		if (closeOnBackdrop && event.target === event.currentTarget) {
			onclose();
		}
	}
</script>

<svelte:window onkeydown={open ? handleKeydown : undefined} />

{#if open}
	<div
		class={['modal-backdrop', elevated && 'modal-backdrop-elevated']}
		role="presentation"
		onclick={handleBackdropClick}
		onkeydown={handleKeydown}
	>
		<div
			class={['modal', fullscreen && 'modal-fullscreen', !fullscreen && wide && 'modal-wide']}
			role="dialog"
			aria-modal="true"
			aria-labelledby="modal-title"
			tabindex="-1"
			{@attach focusTrap}
		>
			<div class="modal-header">
				<h3 id="modal-title" class="modal-title">{title}</h3>
				{#if showCloseButton}
					<button
						type="button"
						class="btn btn-ghost btn-icon"
						onclick={onclose}
						aria-label={$i18n('ui_modal_close')}
					>
						<X size={20} />
					</button>
				{/if}
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
