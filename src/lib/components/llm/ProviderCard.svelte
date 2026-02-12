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
  Displays a LLM provider with status, configuration options, and connection testing.
  All providers are always available - agents can use models from multiple providers.

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
	import type { Snippet } from 'svelte';
	import { Card, Badge, Button, StatusIndicator } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import ConnectionTester from './ConnectionTester.svelte';
	import type { ProviderSettings, ProviderType, LLMModel } from '$types/llm';

	/**
	 * ProviderCard props
	 */
	interface Props {
		/** Provider type identifier */
		provider: ProviderType;
		/** Override display name (for custom providers) */
		displayName?: string;
		/** Provider settings (null if not loaded) */
		settings: ProviderSettings | null;
		/** Whether the provider has an API key configured */
		hasApiKey: boolean;
		/** Default model for this provider (if set) */
		defaultModel?: LLMModel | null;
		/** Whether this is a custom (non-builtin) provider */
		isCustom?: boolean;
		/** Icon snippet to render */
		icon?: Snippet;
		/** Callback when configure button is clicked */
		onConfigure: () => void;
		/** Callback when delete button is clicked (custom providers only) */
		onDelete?: () => void;
	}

	let {
		provider,
		displayName,
		settings,
		hasApiKey,
		defaultModel = null,
		isCustom = false,
		icon,
		onConfigure,
		onDelete
	}: Props = $props();

	/**
	 * Gets the display name for the provider.
	 * Uses displayName prop override for custom providers, i18n for builtins.
	 */
	function getProviderDisplayName(p: ProviderType): string {
		if (displayName) return displayName;
		switch (p) {
			case 'mistral':
				return $i18n('llm_provider_mistral');
			case 'ollama':
				return $i18n('llm_provider_ollama');
			default:
				return p;
		}
	}

	/**
	 * Gets the provider type description.
	 */
	function getProviderTypeDescription(p: ProviderType): string {
		switch (p) {
			case 'mistral':
				return $i18n('llm_provider_cloud_api');
			case 'ollama':
				return $i18n('llm_provider_local_server');
			default:
				return $i18n('llm_provider_cloud_api');
		}
	}

	/**
	 * Gets the badge variant based on configuration status
	 */
	function getBadgeVariant(): 'success' | 'warning' {
		return isConfigured ? 'success' : 'warning';
	}

	/**
	 * Gets the translation key for status text
	 */
	function getStatusKey(): string {
		return isConfigured ? 'llm_provider_ready' : 'llm_provider_not_configured';
	}

	/**
	 * Determines if the provider is configured
	 */
	const isConfigured = $derived(hasApiKey || provider === 'ollama');

	/** Provider name for display */
	const providerDisplayName = $derived(getProviderDisplayName(provider));
	const providerTypeDesc = $derived(getProviderTypeDescription(provider));
</script>

<Card>
	{#snippet header()}
		<div class="provider-header">
			<div class="provider-info">
				{#if icon}
					<div class="provider-icon">
						{@render icon()}
					</div>
				{/if}
				<div class="provider-details">
					<h3 class="provider-name">{providerDisplayName}</h3>
					<p class="provider-type">{providerTypeDesc}</p>
				</div>
			</div>
			<Badge variant={getBadgeVariant()}>
				{$i18n(getStatusKey())}
			</Badge>
		</div>
	{/snippet}

	{#snippet body()}
		<div class="provider-body">
			<div class="status-list">
				{#if isConfigured}
					<div class="status-row">
						<StatusIndicator status="completed" size="sm" />
						<span class="status-text">
							{provider === 'ollama' ? $i18n('llm_provider_server_available') : $i18n('llm_provider_api_key_configured')}
						</span>
					</div>
				{:else}
					<div class="status-row">
						<StatusIndicator status="error" size="sm" />
						<span class="status-text">{$i18n('llm_provider_not_configured')}</span>
					</div>
				{/if}

				{#if defaultModel}
					<div class="info-row">
						<span class="info-label">{$i18n('llm_provider_default_model')}</span>
						<span class="info-value">{defaultModel.name}</span>
					</div>
				{/if}

				{#if settings?.base_url && (provider === 'ollama' || isCustom)}
					<div class="info-row">
						<span class="info-label">{$i18n('llm_provider_server_url')}</span>
						<span class="info-value url">{settings.base_url}</span>
					</div>
				{/if}
			</div>

			<ConnectionTester {provider} disabled={!isConfigured} />
		</div>
	{/snippet}

	{#snippet footer()}
		<div class="provider-actions">
			{#if isCustom && onDelete}
				<Button variant="danger" size="sm" onclick={onDelete}>
					{$i18n('common_delete')}
				</Button>
			{/if}
			<Button variant="primary" size="sm" onclick={onConfigure}>
				{$i18n('llm_provider_configure')}
			</Button>
		</div>
	{/snippet}
</Card>

<style>
	.provider-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-md);
	}

	.provider-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.provider-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 40px;
		height: 40px;
		border-radius: var(--radius-md);
		background-color: var(--color-bg-secondary);
	}

	.provider-details {
		display: flex;
		flex-direction: column;
	}

	.provider-name {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.provider-type {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.provider-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.status-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.status-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.status-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.info-row {
		display: flex;
		align-items: baseline;
		gap: var(--spacing-xs);
		font-size: var(--font-size-sm);
	}

	.info-label {
		color: var(--color-text-secondary);
	}

	.info-value {
		color: var(--color-text-primary);
		font-weight: var(--font-weight-medium);
	}

	.info-value.url {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
	}

	.provider-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}
</style>
