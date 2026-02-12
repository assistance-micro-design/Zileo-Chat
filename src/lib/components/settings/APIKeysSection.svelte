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
API Keys Section - Extracted from Settings page (OPT-6c)
Manages API key configuration modal for LLM providers.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import type { ProviderType, ProviderSettings } from '$types/llm';
	import { Button, Input, Modal, StatusIndicator } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	/** Props */
	interface Props {
		/** Whether the modal is open */
		open: boolean;
		/** Current provider being configured */
		provider: ProviderType;
		/** Optional display name for the provider (custom providers) */
		providerDisplayName?: string;
		/** Provider settings (for Ollama base_url) */
		providerSettings: ProviderSettings | null;
		/** Whether provider has API key configured */
		hasApiKey: boolean;
		/** Whether this is a custom provider */
		isCustom?: boolean;
		/** Close modal callback */
		onclose: () => void;
		/** Reload LLM data callback (after save/delete) */
		onReload: () => void;
	}

	let { open, provider, providerDisplayName, providerSettings, hasApiKey, isCustom = false, onclose, onReload }: Props = $props();

	/** Whether this provider requires an API key (not ollama) */
	const requiresApiKey = $derived(provider !== 'ollama');

	/** Form state */
	let apiKey = $state('');
	let saving = $state(false);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);

	/**
	 * Resets form state when modal opens/closes
	 */
	$effect(() => {
		if (open) {
			apiKey = '';
			message = null;
		}
	});

	/**
	 * Saves API key for the selected provider
	 */
	async function handleSaveApiKey(): Promise<void> {
		if (!apiKey.trim()) {
			message = { type: 'error', text: 'API key cannot be empty' };
			return;
		}

		// OPT-10: Confirmation before saving API key
		if (!confirm($i18n('api_key_confirm_save'))) {
			return;
		}

		saving = true;
		message = null;

		try {
			// Map ProviderType to LLMProvider format (capitalize first letter)
			const providerName = provider.charAt(0).toUpperCase() + provider.slice(1);
			await invoke('save_api_key', {
				provider: providerName,
				apiKey: apiKey
			});
			apiKey = '';
			onReload();
			message = { type: 'success', text: 'API key saved securely' };
			onclose();
		} catch (err) {
			message = { type: 'error', text: `Failed to save: ${err}` };
		} finally {
			saving = false;
		}
	}

	/**
	 * Deletes API key for the provider
	 */
	async function handleDeleteApiKey(): Promise<void> {
		if (!confirm(`Are you sure you want to delete the API key for ${provider}?`)) {
			return;
		}

		saving = true;
		message = null;

		try {
			const providerName = provider.charAt(0).toUpperCase() + provider.slice(1);
			await invoke('delete_api_key', { provider: providerName });
			onReload();
			message = { type: 'success', text: 'API key deleted' };
		} catch (err) {
			message = { type: 'error', text: `Failed to delete: ${err}` };
		} finally {
			saving = false;
		}
	}
</script>

<Modal
	{open}
	title={provider === 'ollama' ? $i18n('api_key_modal_ollama') : (isCustom ? `${$i18n('llm_provider_configure')} ${providerDisplayName ?? provider}` : $i18n('api_key_modal_mistral'))}
	onclose={() => onclose()}
>
	{#snippet body()}
		<div class="api-key-modal-content">
			{#if provider === 'ollama'}
				<p class="api-key-info">
					{$i18n('api_key_ollama_info')}
				</p>
				<Input
					type="url"
					label={$i18n('api_key_server_url')}
					value={providerSettings?.base_url ?? 'http://localhost:11434'}
					help={$i18n('api_key_server_url_help')}
					disabled
				/>
				<div class="status-row">
					<StatusIndicator status="completed" size="sm" />
					<span class="status-text">{$i18n('api_key_not_required')}</span>
				</div>
			{:else}
				<p class="api-key-info">
					{#if isCustom}
						{$i18n('llm_custom_provider_api_key')}
					{:else}
						{$i18n('api_key_mistral_info')}
					{/if}
				</p>
				<Input
					type="password"
					label={$i18n('api_key_label')}
					placeholder={$i18n('api_key_placeholder')}
					bind:value={apiKey}
					disabled={saving}
					help={$i18n('api_key_help')}
				/>
				{#if hasApiKey}
					<div class="status-row">
						<StatusIndicator status="completed" size="sm" />
						<span class="status-text">{$i18n('api_key_configured')}</span>
					</div>
				{/if}
			{/if}

			{#if message}
				<div class="message-toast" class:success={message.type === 'success'} class:error={message.type === 'error'}>
					{message.text}
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="api-key-modal-actions">
			<Button variant="ghost" onclick={() => onclose()} disabled={saving}>
				{$i18n('common_cancel')}
			</Button>
			{#if requiresApiKey}
				{#if hasApiKey}
					<Button
						variant="danger"
						onclick={handleDeleteApiKey}
						disabled={saving}
					>
						{$i18n('api_key_delete')}
					</Button>
				{/if}
				<Button
					variant="primary"
					onclick={handleSaveApiKey}
					disabled={saving || !apiKey.trim()}
				>
					{saving ? $i18n('common_saving') : $i18n('api_key_save')}
				</Button>
			{:else}
				<Button variant="primary" onclick={() => onclose()}>
					{$i18n('common_done')}
				</Button>
			{/if}
		</div>
	{/snippet}
</Modal>

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

	.status-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-success-light);
		border-radius: var(--border-radius-md);
	}

	.status-text {
		font-size: var(--font-size-sm);
		color: var(--color-success);
	}

	.api-key-modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}

	.message-toast {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.message-toast.success {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.message-toast.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}
</style>
