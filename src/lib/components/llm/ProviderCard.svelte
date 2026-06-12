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
  ProviderCard Component
  Displays a builtin LLM provider: configuration status badge, short
  description, connection testing, and either the API key entry point
  (cloud providers) or the editable server URL (Ollama). Custom providers
  are rendered as compact entity rows by LLMSection instead.

  @example
  <ProviderCard
    provider="mistral"
    settings={mistralSettings}
    hasApiKey={true}
    onConfigure={() => openConfig('mistral')}
  >
    {#snippet icon()}
      <MistralIcon />
    {/snippet}
  </ProviderCard>
-->
<script lang="ts">
	import { untrack } from 'svelte';
	import type { Snippet } from 'svelte';
	import { Card, Badge, Button, Input } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import ConnectionTester from './ConnectionTester.svelte';
	import { updateProviderSettings } from '$lib/stores/llm';
	import { toastStore } from '$lib/stores/toast';
	import { getErrorMessage } from '$lib/utils/error';
	import { Check, KeyRound } from '@lucide/svelte';
	import type { ProviderSettings, ProviderType } from '$types/llm';
	import type { ToastType } from '$types/background-workflow';

	/** Default Ollama endpoint used when no settings row exists yet. */
	const DEFAULT_OLLAMA_URL = 'http://localhost:11434';

	/**
	 * ProviderCard props
	 */
	interface Props {
		/** Provider type identifier */
		provider: ProviderType;
		/** Provider settings (null if not loaded) */
		settings: ProviderSettings | null;
		/** Whether the provider has an API key configured */
		hasApiKey: boolean;
		/** Icon snippet to render */
		icon?: Snippet;
		/** Callback when the API key configuration button is clicked (cloud providers) */
		onConfigure?: () => void;
	}

	let { provider, settings, hasApiKey, icon, onConfigure }: Props = $props();

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	/**
	 * Determines if the provider is configured
	 */
	const isConfigured = $derived(hasApiKey || provider === 'ollama');

	/** Provider name for display */
	const providerDisplayName = $derived(
		provider === 'mistral'
			? $i18n('llm_provider_mistral')
			: provider === 'ollama'
				? $i18n('llm_provider_ollama')
				: provider
	);

	/** Status badge label */
	const statusText = $derived(
		!isConfigured
			? $i18n('llm_provider_not_configured')
			: provider === 'ollama'
				? $i18n('llm_provider_server_available')
				: $i18n('llm_provider_api_key_configured')
	);

	// Local editable copy of the Ollama base URL. The card is mounted after
	// the parent finished loading, so the prop seed is stable; untrack avoids
	// the state_referenced_locally warning (the field is intentionally
	// uncontrolled after mount).
	let urlValue = $state(untrack(() => settings?.base_url ?? DEFAULT_OLLAMA_URL));
	let lastSavedUrl = untrack(() => settings?.base_url ?? DEFAULT_OLLAMA_URL);
	let urlSaving = $state(false);

	/**
	 * Persists the edited server URL on blur when it changed. Invalid values
	 * are rejected with a toast and the field reverts to the saved URL.
	 */
	async function saveServerUrl(): Promise<void> {
		const trimmed = urlValue.trim();
		if (trimmed === lastSavedUrl) {
			urlValue = lastSavedUrl;
			return;
		}
		if (!/^https?:\/\//.test(trimmed)) {
			notify('error', $i18n('llm_provider_url_invalid'));
			urlValue = lastSavedUrl;
			return;
		}
		urlSaving = true;
		try {
			await updateProviderSettings(provider, undefined, trimmed);
			lastSavedUrl = trimmed;
			urlValue = trimmed;
			notify('success', $i18n('llm_provider_url_saved'));
		} catch (err) {
			urlValue = lastSavedUrl;
			notify('error', $i18n('llm_provider_url_save_failed', { error: getErrorMessage(err) }));
		} finally {
			urlSaving = false;
		}
	}
</script>

<Card hover>
	{#snippet header()}
		<div class="provider-header">
			<div class="provider-info">
				{#if icon}
					<span class="provider-icon">
						{@render icon()}
					</span>
				{/if}
				<span class="card-title">{providerDisplayName}</span>
			</div>
			<Badge variant={isConfigured ? 'success' : 'warning'}>
				{#if isConfigured}
					<Check size={12} aria-hidden="true" />
				{/if}
				{statusText}
			</Badge>
		</div>
	{/snippet}

	{#snippet body()}
		<div class="provider-body">
			<p class="provider-description">
				{provider === 'ollama'
					? $i18n('llm_provider_local_no_key')
					: $i18n('llm_provider_cloud_api')}
			</p>
			{#if provider === 'ollama'}
				<Input
					type="url"
					label={$i18n('api_key_server_url')}
					bind:value={urlValue}
					onblur={() => saveServerUrl()}
					disabled={urlSaving}
				/>
			{/if}
		</div>
	{/snippet}

	{#snippet footer()}
		<div class="provider-actions">
			<ConnectionTester {provider} disabled={!isConfigured} />
			{#if onConfigure}
				<Button variant="outline" size="sm" onclick={onConfigure}>
					<KeyRound size={14} aria-hidden="true" />
					<span>{$i18n('llm_provider_configure')}</span>
				</Button>
			{/if}
		</div>
	{/snippet}
</Card>

<style>
	.provider-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-md);
		width: 100%;
	}

	.provider-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.provider-icon {
		display: flex;
		align-items: center;
		color: var(--color-accent-deep);
	}

	.provider-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.provider-description {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.provider-body :global(.form-group) {
		margin-bottom: 0;
	}

	.provider-actions {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-sm);
	}

	.provider-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}
</style>
