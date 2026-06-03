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

<script lang="ts">
	/**
	 * Step 5: API Key configuration
	 * User enters Mistral API key with optional test
	 */
	import { i18n } from '$lib/i18n';
	import { scale } from 'svelte/transition';
	import { tauriInvoke } from '$lib/tauri';
	import { onboardingStore, onboardingLoading } from '$lib/stores/onboarding';
	import { Button, Input } from '$lib/components/ui';
	import { getErrorMessage } from '$lib/utils/error';
	import { openExternalUrl } from '$lib/tauri';
	import { isAllowedScheme } from '$lib/utils/url';
	import { motionDuration } from '$lib/utils/motion';
	import { CircleCheck } from '@lucide/svelte';

	const MISTRAL_CONSOLE_URL = 'https://console.mistral.ai';

	/**
	 * Opens the Mistral console in the user's default browser via the Tauri opener plugin.
	 */
	async function openMistralConsole(event: MouseEvent): Promise<void> {
		event.preventDefault();
		if (isAllowedScheme(MISTRAL_CONSOLE_URL)) {
			await openExternalUrl(MISTRAL_CONSOLE_URL);
		}
	}

	interface Props {
		onNext: () => void;
	}

	let { onNext }: Props = $props();

	/** Minimum API key length enforced by the backend validator. */
	const MIN_API_KEY_LEN = 16;

	let apiKey = $state('');
	let testError = $state<string | null>(null);
	let testSuccess = $state(false);
	/** Tracks the last key already persisted, to avoid redundant saves on blur. */
	let savedKey = $state('');

	/**
	 * Persists the entered key so it is never lost when the user advances with
	 * the modal footer "Next" button (which only blurs the field, never calling
	 * testConnection). The footer button blurs the input first, so this fires
	 * before navigation. Errors are surfaced inline but never block navigation.
	 */
	async function persistApiKey(): Promise<void> {
		const trimmed = apiKey.trim();
		if (trimmed.length < MIN_API_KEY_LEN || trimmed === savedKey) return;

		try {
			await tauriInvoke('save_api_key', { provider: 'mistral', apiKey: trimmed });
			savedKey = trimmed;
		} catch (e) {
			testError = getErrorMessage(e);
		}
	}

	/**
	 * Auto-saves the key on blur so a key typed without an explicit test is not
	 * silently discarded when the user clicks the footer "Next" button.
	 */
	function handleBlur(): void {
		void persistApiKey();
	}

	async function testConnection(): Promise<void> {
		if (!apiKey.trim()) return;

		onboardingStore.setLoading(true);
		testError = null;
		testSuccess = false;

		try {
			// Send the lowercase provider id (the app-wide convention). The
			// backend canonicalizes built-in providers to their keystore key
			// ("Mistral"), so save and every read path (boot init, STT, this
			// connection test) agree even on case-sensitive OS keychains such
			// as the Linux secret-service. save_api_key also reconfigures the
			// running provider, so the key works without an app restart.
			const trimmed = apiKey.trim();
			await tauriInvoke('save_api_key', { provider: 'mistral', apiKey: trimmed });
			savedKey = trimmed;

			// Then test the connection
			const result = await tauriInvoke<{ success: boolean; latency_ms?: number; error?: string }>(
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
			testError = getErrorMessage(e);
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
			onblur={handleBlur}
		/>

		<p class="help-text">
			<a href={MISTRAL_CONSOLE_URL} onclick={openMistralConsole}>
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
			<div class="status success" transition:scale={{ duration: motionDuration(250), start: 0.85 }}>
				<CircleCheck size={16} aria-hidden="true" />
				<span>{$i18n('onboarding_apikey_valid')}</span>
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
		font-size: var(--font-size-base);
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
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		text-align: center;
	}

	.status.success {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-xs);
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
		border-radius: var(--border-radius-sm);
	}
</style>
