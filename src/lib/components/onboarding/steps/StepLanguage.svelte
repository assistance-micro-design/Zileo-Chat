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
	import { Globe } from '@lucide/svelte';
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

	<div class="choice-grid">
		<button
			type="button"
			class="choice-card"
			class:selected={$localeStore === 'en'}
			aria-pressed={$localeStore === 'en'}
			onclick={() => selectLanguage('en')}
		>
			<Globe size={24} aria-hidden="true" />
			<span class="label">{$i18n('onboarding_language_english')}</span>
		</button>

		<button
			type="button"
			class="choice-card"
			class:selected={$localeStore === 'fr'}
			aria-pressed={$localeStore === 'fr'}
			onclick={() => selectLanguage('fr')}
		>
			<Globe size={24} aria-hidden="true" />
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
		font-size: var(--font-size-base);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 400px;
	}

	.choice-grid {
		display: grid;
		grid-template-columns: 1fr 1fr;
		gap: var(--spacing-md);
		width: 100%;
		max-width: 420px;
		margin-top: var(--spacing-md);
	}

	.choice-card {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-lg);
		background: var(--surface-1);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		color: var(--color-text-primary);
		cursor: pointer;
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-medium);
		transition:
			border-color 0.2s ease,
			box-shadow 0.2s ease,
			background-color 0.2s ease;
	}

	.choice-card:hover {
		border-color: var(--color-accent-hover);
	}

	.choice-card.selected {
		border-color: var(--color-accent-deep);
		background: var(--color-accent-light);
		box-shadow: var(--glow-accent-soft);
	}

	.choice-card:focus-visible {
		outline: 2px solid var(--color-accent-deep);
		outline-offset: 2px;
	}

	.label {
		font-size: var(--font-size-base);
		color: var(--color-text-primary);
	}
</style>
