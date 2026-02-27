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

DeleteConfirmModal - Reusable delete confirmation dialog.
Extracted from AgentSettings and PromptSettings.
-->

<script lang="ts">
	import { Button, Modal } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Whether the modal is open */
		open: boolean;
		/** i18n key for the modal title */
		titleKey: string;
		/** i18n key for the confirmation message */
		confirmMessageKey: string;
		/** Whether a delete operation is in progress */
		deleting: boolean;
		/** i18n key for the deleting state label (defaults to common_delete) */
		deletingLabelKey?: string;
		/** Name of the item being deleted (displayed in bold after confirm message) */
		itemName?: string;
		/** i18n key for a warning message displayed below the confirm text */
		warningMessageKey?: string;
		/** Callback when delete is confirmed */
		onConfirm: () => void;
		/** Callback when delete is cancelled */
		onCancel: () => void;
	}

	let {
		open,
		titleKey,
		confirmMessageKey,
		deleting,
		deletingLabelKey,
		itemName,
		warningMessageKey,
		onConfirm,
		onCancel
	}: Props = $props();
</script>

<Modal
	{open}
	title={$i18n(titleKey)}
	onclose={onCancel}
>
	{#snippet body()}
		<p class="confirm-text">
			{$i18n(confirmMessageKey)}{#if itemName} <strong class="item-name">"{itemName}"</strong>?{/if}
		</p>
		{#if warningMessageKey}
			<p class="delete-warning">
				{$i18n(warningMessageKey)}
			</p>
		{/if}
	{/snippet}
	{#snippet footer()}
		<div class="modal-actions">
			<Button variant="ghost" onclick={onCancel} disabled={deleting}>
				{$i18n('common_cancel')}
			</Button>
			<Button variant="danger" onclick={onConfirm} disabled={deleting}>
				{deleting ? $i18n(deletingLabelKey ?? 'common_delete') : $i18n('common_delete')}
			</Button>
		</div>
	{/snippet}
</Modal>

<style>
	.confirm-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		line-height: var(--line-height-relaxed);
	}

	.item-name {
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

	.modal-actions {
		display: flex;
		gap: var(--spacing-sm);
		justify-content: flex-end;
	}
</style>
