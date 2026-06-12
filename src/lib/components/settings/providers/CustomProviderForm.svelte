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
  Form for adding or editing an OpenAI-compatible custom provider.
  In create mode it auto-generates a URL-safe name from the display name.
  In edit mode the name is fixed (it is the provider's stable id), the API
  key is optional (left blank keeps the stored key unchanged) and the footer
  offers a connection test against the saved configuration.
-->

<script lang="ts">
	import { untrack } from 'svelte';
	import { Input, PasswordInput, Button, Switch, Spinner } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import { createCustomProvider, updateCustomProvider, testConnection } from '$lib/stores/llm';
	import type { ProviderInfo } from '$types/custom-provider';
	import { getErrorMessage } from '$lib/utils/error';
	import { toastStore } from '$lib/stores/toast';
	import type { ToastType } from '$types/background-workflow';
	import { Zap } from '@lucide/svelte';

	/** Props */
	interface Props {
		/**
		 * Existing provider to edit. When omitted the form is in create mode.
		 * In edit mode the name (id) is immutable and the API key is optional.
		 */
		provider?: ProviderInfo;
		/** Called when provider is created, receives the new provider entity and optional warning */
		oncreated?: (provider: ProviderInfo, warning?: string) => void;
		/** Called when an existing provider is updated, receives the updated entity and optional warning */
		onupdated?: (provider: ProviderInfo, warning?: string) => void;
		/** Called when form is cancelled */
		oncancel: () => void;
	}

	let { provider, oncreated, onupdated, oncancel }: Props = $props();

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	// The modal mounts a fresh form per open, so the `provider` prop is stable
	// for the component's lifetime. `untrack` captures the seed values once
	// without the reactive-read warning (the form is intentionally uncontrolled
	// after mount).
	/** Whether the form edits an existing provider (vs creating a new one). */
	const isEdit = untrack(() => provider !== undefined);

	/** Form fields (seeded from the provider in edit mode). */
	let displayName = $state(untrack(() => provider?.displayName ?? ''));
	let baseUrl = $state(untrack(() => provider?.baseUrl ?? ''));
	let apiKey = $state('');
	/**
	 * Strict-mode toggles. Default `true` preserves OpenRouter behaviour
	 * (cache_control + reasoning top-level object injected). Disable both
	 * for Fireworks, Groq, Together, Cerebras. In edit mode the stored values
	 * seed the switches so they can be corrected without recreating.
	 */
	let supportsCacheControl = $state(untrack(() => provider?.supportsCacheControl ?? true));
	let supportsReasoningParam = $state(untrack(() => provider?.supportsReasoningParam ?? true));
	let saving = $state(false);
	let testing = $state(false);
	let error = $state<string | null>(null);

	/**
	 * Provider name: the immutable id in edit mode, otherwise auto-generated
	 * URL-safe from the display name.
	 */
	const name = $derived(
		isEdit
			? (provider?.id ?? '')
			: displayName
					.toLowerCase()
					.replace(/[^a-z0-9]+/g, '-')
					.replace(/^-+|-+$/g, '')
					.slice(0, 64)
	);

	/**
	 * Form validation. The API key is required only when creating; in edit mode
	 * a blank key keeps the stored one.
	 */
	const isValid = $derived(
		name.length > 0 &&
			displayName.trim().length > 0 &&
			baseUrl.trim().length > 0 &&
			(isEdit || apiKey.trim().length > 0)
	);

	/**
	 * Handles form submission for both create and edit modes.
	 */
	async function handleSubmit(): Promise<void> {
		if (!isValid) return;

		saving = true;
		error = null;

		try {
			if (isEdit) {
				const trimmedKey = apiKey.trim();
				const response = await updateCustomProvider(
					name,
					displayName.trim(),
					baseUrl.trim(),
					trimmedKey.length > 0 ? trimmedKey : undefined,
					undefined,
					supportsCacheControl,
					supportsReasoningParam
				);
				onupdated?.(response.provider, response.warning);
			} else {
				const response = await createCustomProvider(
					name,
					displayName.trim(),
					baseUrl.trim(),
					apiKey.trim(),
					supportsCacheControl,
					supportsReasoningParam
				);
				oncreated?.(response.provider, response.warning);
			}
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			saving = false;
		}
	}

	/**
	 * Tests the connection against the provider's saved configuration
	 * (edit mode only) and reports the outcome as a toast.
	 */
	async function handleTestConnection(): Promise<void> {
		if (!isEdit) return;
		testing = true;
		try {
			const result = await testConnection(name);
			if (result.success) {
				notify(
					'success',
					$i18n('llm_connection_connected', { latency: `${result.latency_ms ?? 0}ms` })
				);
			} else {
				notify('error', result.error_message || $i18n('llm_connection_failed'));
			}
		} catch (e) {
			notify('error', getErrorMessage(e));
		} finally {
			testing = false;
		}
	}
</script>

<form
	class="custom-provider-form"
	onsubmit={(e) => {
		e.preventDefault();
		handleSubmit();
	}}
>
	<div class="form-row">
		<Input label={$i18n('llm_custom_provider_name')} value={name} disabled />
		<Input
			label={$i18n('llm_custom_provider_display_name')}
			placeholder="RouterLab"
			bind:value={displayName}
			disabled={saving}
			required
		/>
	</div>

	<Input
		label={$i18n('llm_custom_provider_base_url')}
		type="url"
		placeholder="https://api.routerlab.ch/v1"
		bind:value={baseUrl}
		disabled={saving}
		help={$i18n('settings_custom_provider_base_url_help')}
		required
	/>

	<PasswordInput
		label={$i18n('api_key_label')}
		placeholder={isEdit ? $i18n('llm_custom_provider_api_key_edit_help') : 'sk-...'}
		bind:value={apiKey}
		disabled={saving}
		required={!isEdit}
	/>

	<div class="toggle-row">
		<span class="toggle-text">
			<strong id="custom-provider-cache-label">
				{$i18n('llm_custom_provider_supports_cache_control')}
			</strong>
			<span>{$i18n('llm_custom_provider_supports_cache_control_help')}</span>
		</span>
		<Switch
			checked={supportsCacheControl}
			onchange={(value) => (supportsCacheControl = value)}
			disabled={saving}
			labelledBy="custom-provider-cache-label"
		/>
	</div>

	<div class="toggle-row">
		<span class="toggle-text">
			<strong id="custom-provider-reasoning-label">
				{$i18n('llm_custom_provider_supports_reasoning_param')}
			</strong>
			<span>{$i18n('llm_custom_provider_supports_reasoning_param_help')}</span>
		</span>
		<Switch
			checked={supportsReasoningParam}
			onchange={(value) => (supportsReasoningParam = value)}
			disabled={saving}
			labelledBy="custom-provider-reasoning-label"
		/>
	</div>

	{#if error}
		<div class="form-error">{error}</div>
	{/if}

	<div class="form-actions">
		{#if isEdit}
			<div class="test-action">
				<Button variant="outline" onclick={handleTestConnection} disabled={testing || saving}>
					{#if testing}
						<Spinner size="sm" />
						<span>{$i18n('llm_connection_testing')}</span>
					{:else}
						<Zap size={14} aria-hidden="true" />
						<span>{$i18n('llm_connection_test')}</span>
					{/if}
				</Button>
			</div>
		{/if}
		<Button variant="ghost" onclick={oncancel} disabled={saving}>
			{$i18n('common_cancel')}
		</Button>
		<Button variant="primary" type="submit" disabled={saving || !isValid}>
			{saving
				? $i18n('common_saving')
				: isEdit
					? $i18n('llm_form_save_changes')
					: $i18n('common_save')}
		</Button>
	</div>
</form>

<style>
	.custom-provider-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.form-row {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
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
		align-items: center;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-sm);
	}

	.test-action {
		margin-right: auto;
	}

	.form-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.toggle-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
		padding: var(--spacing-sm) 0;
	}

	.toggle-row + .toggle-row {
		border-top: 1px solid var(--color-border-light);
	}

	.toggle-text strong {
		display: block;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.toggle-text span {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: 2px;
		max-width: 56ch;
	}

	@media (max-width: 640px) {
		.form-row {
			grid-template-columns: 1fr;
		}
	}
</style>
