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
  CustomProviderForm Component
  Form for adding a new OpenAI-compatible custom provider.
  Auto-generates a URL-safe name from the display name.
-->

<script lang="ts">
	import { Input, Button } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import { createCustomProvider } from '$lib/stores/llm';
	import { getErrorMessage } from '$lib/utils/error';

	/** Props */
	interface Props {
		/** Called when provider is created successfully */
		oncreated: () => void;
		/** Called when form is cancelled */
		oncancel: () => void;
	}

	let { oncreated, oncancel }: Props = $props();

	/** Form fields */
	let displayName = $state('');
	let baseUrl = $state('');
	let apiKey = $state('');
	let saving = $state(false);
	let error = $state<string | null>(null);

	/** Auto-generated URL-safe name from display name */
	const name = $derived(
		displayName
			.toLowerCase()
			.replace(/[^a-z0-9]+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 64)
	);

	/** Form validation */
	const isValid = $derived(
		name.length > 0 && displayName.trim().length > 0 && baseUrl.trim().length > 0 && apiKey.trim().length > 0
	);

	/**
	 * Handles form submission
	 */
	async function handleSubmit(): Promise<void> {
		if (!isValid) return;

		saving = true;
		error = null;

		try {
			await createCustomProvider(name, displayName.trim(), baseUrl.trim(), apiKey.trim());
			oncreated();
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			saving = false;
		}
	}
</script>

<form class="custom-provider-form" onsubmit={(e) => { e.preventDefault(); handleSubmit(); }}>
	<Input
		label={$i18n('llm_custom_provider_display_name')}
		placeholder="RouterLab"
		bind:value={displayName}
		disabled={saving}
		required
	/>

	{#if name}
		<div class="name-preview">
			<span class="name-label">{$i18n('llm_custom_provider_name')}:</span>
			<code class="name-value">{name}</code>
		</div>
	{/if}

	<Input
		label={$i18n('llm_custom_provider_base_url')}
		type="url"
		placeholder="https://api.routerlab.ch/v1"
		bind:value={baseUrl}
		disabled={saving}
		help="OpenAI-compatible API endpoint (without /chat/completions)"
		required
	/>

	<Input
		label={$i18n('llm_custom_provider_api_key')}
		type="password"
		placeholder="sk-..."
		bind:value={apiKey}
		disabled={saving}
		required
	/>

	{#if error}
		<div class="form-error">{error}</div>
	{/if}

	<div class="form-actions">
		<Button variant="ghost" onclick={oncancel} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button
			variant="primary"
			type="submit"
			disabled={saving || !isValid}
		>
			{saving ? $i18n('common_saving') : $i18n('common_save')}
		</Button>
	</div>
</form>

<style>
	.custom-provider-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.name-preview {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.name-label {
		white-space: nowrap;
	}

	.name-value {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
		padding: 2px 6px;
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-sm);
	}

	.form-error {
		padding: var(--spacing-sm) var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-sm);
	}
</style>
