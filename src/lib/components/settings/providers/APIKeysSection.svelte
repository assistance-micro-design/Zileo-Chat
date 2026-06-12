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
API Keys Section - Extracted from Settings page
API key configuration modal for cloud builtin providers (Mistral). Ollama
edits its server URL directly on its provider card, and custom providers
manage their key from the edit-provider form.
-->

<script lang="ts">
	import { tauriInvoke } from '$lib/tauri';
	import type { ProviderType } from '$types/llm';
	import { Button, PasswordInput, Modal, DeleteConfirmModal } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { toastStore } from '$lib/stores/toast';
	import type { ToastType } from '$types/background-workflow';

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	/** Props */
	interface Props {
		/** Whether the modal is open */
		open: boolean;
		/** Current provider being configured */
		provider: ProviderType;
		/** Display name used in the modal title and delete confirmation */
		providerDisplayName?: string;
		/** Whether provider has API key configured */
		hasApiKey: boolean;
		/** Close modal callback */
		onclose: () => void;
		/** Reload LLM data callback (after save/delete) */
		onReload: () => void;
	}

	let { open, provider, providerDisplayName, hasApiKey, onclose, onReload }: Props = $props();

	/** Form state */
	let apiKey = $state('');
	let saving = $state(false);

	/** Save confirmation state */
	let showSaveConfirm = $state(false);
	let saveConfirming = $state(false);

	/** Delete confirmation state */
	let showDeleteConfirm = $state(false);
	let deleteConfirming = $state(false);

	/**
	 * Resets form state when modal opens/closes
	 */
	$effect(() => {
		if (open) {
			apiKey = '';
		}
	});

	/**
	 * Requests save confirmation for API key
	 */
	function handleSaveApiKeyRequest(): void {
		if (!apiKey.trim()) {
			notify('error', $i18n('settings_api_key_empty'));
			return;
		}
		showSaveConfirm = true;
	}

	/**
	 * Confirms and executes API key save
	 */
	async function confirmSaveApiKey(): Promise<void> {
		saveConfirming = true;

		try {
			// Pass the provider id verbatim (lowercase, e.g. "mistral"). The
			// backend canonicalizes built-in providers to their keystore key
			// (e.g. "Mistral"), so the casing sent here does not have to match
			// the read sites.
			await tauriInvoke('save_api_key', {
				provider: provider,
				apiKey: apiKey
			});
			apiKey = '';
			onReload();
			notify('success', $i18n('settings_api_key_saved'));
			showSaveConfirm = false;
			onclose();
		} catch (err) {
			notify('error', $i18n('settings_api_key_save_failed', { error: getErrorMessage(err) }));
		} finally {
			saveConfirming = false;
		}
	}

	/**
	 * Cancels save confirmation
	 */
	function cancelSaveApiKey(): void {
		showSaveConfirm = false;
	}

	/**
	 * Requests delete confirmation for API key
	 */
	function handleDeleteApiKeyRequest(): void {
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes API key deletion
	 */
	async function confirmDeleteApiKey(): Promise<void> {
		deleteConfirming = true;

		try {
			// Same id as the save path — see confirmSaveApiKey.
			await tauriInvoke('delete_api_key', { provider: provider });
			onReload();
			notify('success', $i18n('settings_api_key_deleted'));
			showDeleteConfirm = false;
		} catch (err) {
			notify('error', $i18n('settings_api_key_delete_failed', { error: getErrorMessage(err) }));
		} finally {
			deleteConfirming = false;
		}
	}

	/**
	 * Cancels delete confirmation
	 */
	function cancelDeleteApiKey(): void {
		showDeleteConfirm = false;
	}
</script>

<Modal {open} title={$i18n('api_key_modal_mistral')} onclose={() => onclose()}>
	{#snippet body()}
		<div class="api-key-modal-content">
			<p class="api-key-info">{$i18n('api_key_mistral_info')}</p>
			<PasswordInput
				label={$i18n('api_key_label')}
				placeholder={$i18n('api_key_placeholder')}
				bind:value={apiKey}
				disabled={saving}
				help={$i18n('api_key_help')}
			/>
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="api-key-modal-actions">
			{#if hasApiKey}
				<div class="delete-action">
					<Button variant="danger-soft" onclick={handleDeleteApiKeyRequest} disabled={saving}>
						{$i18n('api_key_delete')}
					</Button>
				</div>
			{/if}
			<Button variant="ghost" onclick={() => onclose()} disabled={saving}>
				{$i18n('common_cancel')}
			</Button>
			<Button
				variant="primary"
				onclick={handleSaveApiKeyRequest}
				disabled={saving || !apiKey.trim()}
			>
				{saving ? $i18n('common_saving') : $i18n('api_key_save')}
			</Button>
		</div>
	{/snippet}
</Modal>

<!-- Save API Key Confirmation Modal -->
<DeleteConfirmModal
	open={showSaveConfirm}
	titleKey="api_key_save_title"
	confirmMessageKey="api_key_confirm_save"
	deleting={saveConfirming}
	deletingLabelKey="api_key_saving"
	variant="primary"
	confirmLabelKey="api_key_save_confirm_label"
	onConfirm={confirmSaveApiKey}
	onCancel={cancelSaveApiKey}
/>

<!-- Delete API Key Confirmation Modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="api_key_delete_title"
	confirmMessageKey="api_key_delete_confirm_msg"
	deleting={deleteConfirming}
	itemName={providerDisplayName ?? provider}
	onConfirm={confirmDeleteApiKey}
	onCancel={cancelDeleteApiKey}
/>

<style>
	.api-key-modal-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.api-key-info {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		line-height: var(--line-height-relaxed);
		margin: 0;
	}

	.api-key-modal-actions {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: var(--spacing-sm);
		width: 100%;
	}

	.delete-action {
		margin-right: auto;
	}
</style>
