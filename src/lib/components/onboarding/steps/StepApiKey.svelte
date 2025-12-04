<script lang="ts">
	/**
	 * Step 5: API Key configuration
	 * User enters Mistral API key with optional test
	 */
	import { i18n } from '$lib/i18n';
	import { invoke } from '@tauri-apps/api/core';
	import { onboardingStore, onboardingLoading } from '$lib/stores/onboarding';
	import { Button, Input } from '$lib/components/ui';

	interface Props {
		onNext: () => void;
	}

	let { onNext }: Props = $props();

	let apiKey = $state('');
	let testError = $state<string | null>(null);
	let testSuccess = $state(false);

	async function testConnection(): Promise<void> {
		if (!apiKey.trim()) return;

		onboardingStore.setLoading(true);
		testError = null;
		testSuccess = false;

		try {
			// First save the API key
			await invoke('save_api_key', { provider: 'mistral', apiKey: apiKey.trim() });

			// Then test the connection
			const result = await invoke<{ success: boolean; latency_ms?: number; error?: string }>(
				'test_provider_connection',
				{ provider: 'mistral' }
			);

			if (result.success) {
				testSuccess = true;
				onboardingStore.setApiKeyValid(true);
			} else {
				testError = result.error || $i18n('onboarding_apikey_invalid');
				onboardingStore.setApiKeyValid(false);
			}
		} catch (e) {
			testError = String(e);
			onboardingStore.setApiKeyValid(false);
		} finally {
			onboardingStore.setLoading(false);
		}
	}

	function handleSkip(): void {
		onNext();
	}
</script>

<div class="step-apikey" data-step="api_key">
	<h1 class="step-title">{$i18n('onboarding_apikey_title')}</h1>
	<p class="step-description">{$i18n('onboarding_apikey_description')}</p>

	<div class="apikey-form">
		<Input
			type="password"
			bind:value={apiKey}
			placeholder={$i18n('onboarding_apikey_placeholder')}
			label=""
			disabled={$onboardingLoading}
		/>

		<p class="help-text">
			<a href="https://console.mistral.ai" target="_blank" rel="noopener noreferrer">
				{$i18n('onboarding_apikey_help')}
			</a>
		</p>

		<div class="button-row">
			<Button
				variant="secondary"
				onclick={testConnection}
				disabled={!apiKey.trim() || $onboardingLoading}
			>
				{#if $onboardingLoading}
					{$i18n('onboarding_apikey_testing')}
				{:else}
					{$i18n('onboarding_apikey_test')}
				{/if}
			</Button>
		</div>

		{#if testSuccess}
			<div class="status success">
				{$i18n('onboarding_apikey_valid')}
			</div>
		{/if}

		{#if testError}
			<div class="status error">
				{testError}
			</div>
		{/if}
	</div>

	<button class="skip-step" onclick={handleSkip}>
		{$i18n('onboarding_apikey_skip')}
	</button>
</div>

<style>
	.step-apikey {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
		width: 100%;
	}

	.step-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.step-description {
		font-size: var(--font-size-md);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 400px;
	}

	.apikey-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		width: 100%;
		max-width: 400px;
	}

	.help-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.help-text a {
		color: var(--color-primary);
		text-decoration: none;
	}

	.help-text a:hover {
		text-decoration: underline;
	}

	.button-row {
		display: flex;
		justify-content: center;
	}

	.status {
		padding: var(--spacing-sm) var(--spacing-md);
		border-radius: var(--radius-md);
		font-size: var(--font-size-sm);
		text-align: center;
	}

	.status.success {
		background: var(--color-success-bg, #d1fae5);
		color: var(--color-success, #059669);
	}

	.status.error {
		background: var(--color-error-bg, #fee2e2);
		color: var(--color-error, #dc2626);
	}

	.skip-step {
		background: none;
		border: none;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
		cursor: pointer;
		padding: var(--spacing-xs) var(--spacing-sm);
		transition: color 0.2s ease;
	}

	.skip-step:hover {
		color: var(--color-text-primary);
	}

	.skip-step:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
		border-radius: var(--radius-sm);
	}
</style>
