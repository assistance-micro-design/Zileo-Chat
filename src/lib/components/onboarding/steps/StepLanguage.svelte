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
	 * Step 1: Language selection
	 * User selects preferred language (EN/FR)
	 */
	import { i18n } from '$lib/i18n';
	import { localeStore } from '$lib/stores/locale';
	import type { Locale } from '$types/i18n';

	interface Props {
		onNext?: () => void;
	}

	let { onNext: _onNext }: Props = $props();

	function selectLanguage(locale: Locale): void {
		localeStore.setLocale(locale);
	}
</script>

<div class="step-language" data-step="language">
	<h1 id="onboarding-title" class="step-title">{$i18n('onboarding_language_title')}</h1>
	<p class="step-description">{$i18n('onboarding_language_description')}</p>

	<div class="language-options">
		<button
			class="language-button"
			onclick={() => selectLanguage('en')}
			aria-label={$i18n('onboarding_language_english')}
		>
			<span class="flag">GB</span>
			<span class="label">{$i18n('onboarding_language_english')}</span>
		</button>

		<button
			class="language-button"
			onclick={() => selectLanguage('fr')}
			aria-label={$i18n('onboarding_language_french')}
		>
			<span class="flag">FR</span>
			<span class="label">{$i18n('onboarding_language_french')}</span>
		</button>
	</div>
</div>

<style>
	.step-language {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
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

	.language-options {
		display: flex;
		gap: var(--spacing-lg);
		margin-top: var(--spacing-md);
	}

	.language-button {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-xl);
		background: var(--color-bg-secondary);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-lg);
		cursor: pointer;
		transition: all 0.2s ease;
		min-width: 120px;
	}

	.language-button:hover {
		border-color: var(--color-primary);
		background: var(--color-bg-hover);
	}

	.language-button:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
	}

	.flag {
		font-size: 2rem;
		font-weight: var(--font-weight-bold);
		color: var(--color-text-primary);
	}

	.label {
		font-size: var(--font-size-md);
		color: var(--color-text-primary);
	}
</style>
