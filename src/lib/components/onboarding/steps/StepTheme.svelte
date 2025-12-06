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
	 * Step 2: Theme selection
	 * User selects light or dark theme
	 */
	import { i18n } from '$lib/i18n';
	import { theme, type Theme } from '$lib/stores/theme';

	interface Props {
		onNext?: () => void;
	}

	let { onNext: _onNext }: Props = $props();

	function selectTheme(selectedTheme: Theme): void {
		theme.setTheme(selectedTheme);
	}
</script>

<div class="step-theme" data-step="theme">
	<h1 class="step-title">{$i18n('onboarding_theme_title')}</h1>
	<p class="step-description">{$i18n('onboarding_theme_description')}</p>

	<div class="theme-options">
		<button
			class="theme-button"
			data-theme-option="light"
			onclick={() => selectTheme('light')}
			aria-label={$i18n('onboarding_theme_light')}
		>
			<div class="theme-preview theme-preview-light">
				<div class="preview-header"></div>
				<div class="preview-content">
					<div class="preview-line"></div>
					<div class="preview-line short"></div>
				</div>
			</div>
			<span class="label">{$i18n('onboarding_theme_light')}</span>
		</button>

		<button
			class="theme-button"
			data-theme-option="dark"
			onclick={() => selectTheme('dark')}
			aria-label={$i18n('onboarding_theme_dark')}
		>
			<div class="theme-preview theme-preview-dark">
				<div class="preview-header"></div>
				<div class="preview-content">
					<div class="preview-line"></div>
					<div class="preview-line short"></div>
				</div>
			</div>
			<span class="label">{$i18n('onboarding_theme_dark')}</span>
		</button>
	</div>
</div>

<style>
	.step-theme {
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

	.theme-options {
		display: flex;
		gap: var(--spacing-lg);
		margin-top: var(--spacing-md);
	}

	.theme-button {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 2px solid var(--color-border);
		border-radius: var(--radius-lg);
		cursor: pointer;
		transition: all 0.2s ease;
	}

	.theme-button:hover {
		border-color: var(--color-primary);
	}

	.theme-button:focus-visible {
		outline: 2px solid var(--color-primary);
		outline-offset: 2px;
	}

	.theme-preview {
		width: 140px;
		height: 100px;
		border-radius: var(--radius-md);
		overflow: hidden;
		display: flex;
		flex-direction: column;
	}

	.theme-preview-light {
		background: #ffffff;
		border: 1px solid #e5e7eb;
	}

	.theme-preview-light .preview-header {
		height: 20px;
		background: #f3f4f6;
		border-bottom: 1px solid #e5e7eb;
	}

	.theme-preview-light .preview-content {
		flex: 1;
		padding: 8px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.theme-preview-light .preview-line {
		height: 8px;
		background: #e5e7eb;
		border-radius: 4px;
	}

	.theme-preview-light .preview-line.short {
		width: 60%;
	}

	.theme-preview-dark {
		background: #1f2937;
		border: 1px solid #374151;
	}

	.theme-preview-dark .preview-header {
		height: 20px;
		background: #111827;
		border-bottom: 1px solid #374151;
	}

	.theme-preview-dark .preview-content {
		flex: 1;
		padding: 8px;
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.theme-preview-dark .preview-line {
		height: 8px;
		background: #374151;
		border-radius: 4px;
	}

	.theme-preview-dark .preview-line.short {
		width: 60%;
	}

	.label {
		font-size: var(--font-size-md);
		color: var(--color-text-primary);
	}
</style>
