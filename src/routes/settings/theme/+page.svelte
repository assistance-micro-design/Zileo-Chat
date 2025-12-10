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
Settings > Theme Page
Manages theme selection (light/dark).
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, HelpButton } from '$lib/components/ui';
	import { theme, type Theme } from '$lib/stores/theme';
	import { Sun, Moon, ShieldCheck } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/** Current theme value - synced with theme store */
	let currentTheme = $state<Theme>('light');

	/**
	 * Handle theme change
	 */
	function handleThemeChange(newTheme: Theme): void {
		theme.setTheme(newTheme);
	}

	onMount(() => {
		// Subscribe to theme store and sync value
		const unsubscribeTheme = theme.subscribe((value) => {
			currentTheme = value;
		});

		return () => {
			unsubscribeTheme();
		};
	});
</script>

<section class="settings-section">
	<div class="section-title-row">
		<h2 class="section-title">{$i18n('settings_theme')}</h2>
		<HelpButton
			titleKey="help_theme_title"
			descriptionKey="help_theme_description"
			tutorialKey="help_theme_tutorial"
		/>
	</div>

	<div class="theme-grid">
		<!-- Light Theme Card -->
		<button
			type="button"
			class="theme-card"
			class:selected={currentTheme === 'light'}
			onclick={() => handleThemeChange('light')}
		>
			<div class="theme-preview light">
				<div class="theme-header">
					<Sun size={24} />
					<div>
						<h3 class="theme-title">{$i18n('theme_light')}</h3>
						<p class="theme-description">{$i18n('theme_light_description')}</p>
					</div>
				</div>
				<div class="theme-colors">
					<div class="color-swatch accent"></div>
					<div class="color-swatch secondary"></div>
					<div class="color-swatch bg-light"></div>
				</div>
			</div>
		</button>

		<!-- Dark Theme Card -->
		<button
			type="button"
			class="theme-card"
			class:selected={currentTheme === 'dark'}
			onclick={() => handleThemeChange('dark')}
		>
			<div class="theme-preview dark">
				<div class="theme-header">
					<Moon size={24} />
					<div>
						<h3 class="theme-title">{$i18n('theme_dark')}</h3>
						<p class="theme-description">{$i18n('theme_dark_description')}</p>
					</div>
				</div>
				<div class="theme-colors">
					<div class="color-swatch accent"></div>
					<div class="color-swatch secondary"></div>
					<div class="color-swatch bg-dark"></div>
				</div>
			</div>
		</button>
	</div>
</section>

<!-- Security Info -->
<section class="settings-section">
	<Card>
		{#snippet header()}
			<div class="security-header">
				<ShieldCheck size={24} class="icon-success" />
				<h3 class="card-title">{$i18n('security_title')}</h3>
			</div>
		{/snippet}
		{#snippet body()}
			<p class="security-info-text">
				{$i18n('security_description')}
			</p>
		{/snippet}
	</Card>
</section>

<style>
	.settings-section {
		margin-bottom: var(--spacing-2xl);
		padding-bottom: var(--spacing-xl);
	}

	.section-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row .section-title {
		margin-bottom: 0;
	}

	/* Theme Cards */
	.theme-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
	}

	.theme-card {
		cursor: pointer;
		background: none;
		border: none;
		padding: 0;
		width: 100%;
		text-align: left;
	}

	.theme-preview {
		background: var(--color-bg-primary);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		overflow: hidden;
		transition: border-color var(--transition-fast);
	}

	.theme-card.selected .theme-preview {
		border-color: var(--color-accent);
	}

	.theme-preview .theme-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-lg);
	}

	.theme-preview.light .theme-header {
		background: #ffffff;
		color: #212529;
	}

	.theme-preview.dark .theme-header {
		background: #2b2d31;
		color: #ffffff;
	}

	.theme-preview.dark .theme-description {
		color: #b5bac1;
	}

	.theme-title {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
	}

	.theme-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.theme-colors {
		display: flex;
		gap: var(--spacing-sm);
		padding: var(--spacing-lg);
		background: var(--color-bg-secondary);
	}

	.color-swatch {
		width: 40px;
		height: 40px;
		border-radius: var(--border-radius-md);
	}

	.color-swatch.accent {
		background: #94EFEE;
	}

	.color-swatch.secondary {
		background: #FE7254;
	}

	.color-swatch.bg-light {
		background: #ffffff;
		border: 1px solid #dee2e6;
	}

	.color-swatch.bg-dark {
		background: #2b2d31;
	}

	/* Security Section */
	.security-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.security-header :global(.icon-success) {
		color: var(--color-success);
	}

	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.security-info-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		line-height: var(--line-height-relaxed);
	}

	/* Responsive */
	@media (max-width: 768px) {
		.theme-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
