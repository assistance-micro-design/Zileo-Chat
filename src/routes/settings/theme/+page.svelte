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
	import { Card } from '$lib/components/ui';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { theme, type Theme } from '$lib/stores/theme';
	import { Sun, Moon, ShieldCheck } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Handle theme change
	 */
	function handleThemeChange(newTheme: Theme): void {
		theme.setTheme(newTheme);
	}
</script>

<section class="settings-section">
	<SettingsSectionHeader
		titleKey="settings_theme"
		descriptionKey="settings_theme_description"
		helpTitleKey="help_theme_title"
		helpDescriptionKey="help_theme_description"
		helpTutorialKey="help_theme_tutorial"
	/>

	<div class="theme-grid">
		<!-- Light Theme Card -->
		<button
			type="button"
			class="theme-card"
			class:selected={$theme === 'light'}
			aria-pressed={$theme === 'light'}
			onclick={() => handleThemeChange('light')}
		>
			<div class="theme-preview light" aria-hidden="true">
				<div class="mini-card">
					<span class="brand-dot"></span>
					<span>{$i18n('theme_light')}</span>
				</div>
			</div>
			<div class="theme-body">
				<span class="theme-id">
					<Sun size={18} aria-hidden="true" />
					<span class="theme-id-text">
						<strong>{$i18n('theme_light')}</strong>
						<span class="theme-id-desc">{$i18n('theme_light_description')}</span>
					</span>
				</span>
				<span class="swatches" aria-hidden="true">
					<span style="background:#94efee"></span>
					<span style="background:#fe7254"></span>
					<span style="background:#f4f6fa"></span>
				</span>
			</div>
		</button>

		<!-- Dark Theme Card -->
		<button
			type="button"
			class="theme-card"
			class:selected={$theme === 'dark'}
			aria-pressed={$theme === 'dark'}
			onclick={() => handleThemeChange('dark')}
		>
			<div class="theme-preview dark" aria-hidden="true">
				<div class="mini-card">
					<span class="brand-dot"></span>
					<span>{$i18n('theme_dark')}</span>
				</div>
			</div>
			<div class="theme-body">
				<span class="theme-id">
					<Moon size={18} aria-hidden="true" />
					<span class="theme-id-text">
						<strong>{$i18n('theme_dark')}</strong>
						<span class="theme-id-desc">{$i18n('theme_dark_description')}</span>
					</span>
				</span>
				<span class="swatches" aria-hidden="true">
					<span style="background:#94efee"></span>
					<span style="background:#fe7254"></span>
					<span style="background:#0f1117"></span>
				</span>
			</div>
		</button>
	</div>
</section>

<!-- Security Info -->
<section class="settings-section">
	<div class="security-card">
		<Card>
			{#snippet body()}
				<div class="security-body">
					<ShieldCheck size={24} class="icon-success" aria-hidden="true" />
					<div>
						<strong class="security-title">{$i18n('security_title')}</strong>
						<p class="security-info-text">
							{$i18n('security_description')}
						</p>
					</div>
				</div>
			{/snippet}
		</Card>
	</div>
</section>

<style>
	/* Theme Cards */
	.theme-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
		max-width: 760px;
		margin-bottom: var(--spacing-lg);
	}

	.theme-card {
		display: block;
		width: 100%;
		padding: 0;
		text-align: left;
		font: inherit;
		color: var(--color-text-primary);
		background: var(--surface-1);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		overflow: hidden;
		cursor: pointer;
		transition:
			border-color var(--transition-fast),
			box-shadow var(--transition-fast);
	}

	.theme-card.selected {
		border-color: var(--color-accent-deep);
		box-shadow: var(--glow-accent-soft);
	}

	.theme-card:focus-visible {
		outline: none;
		border-color: var(--color-accent-deep);
		box-shadow: var(--glow-accent);
	}

	/* Tinted, non-flat previews (light is no longer pure white,
	   dark is no longer Discord gray). */
	.theme-preview {
		height: 110px;
		display: flex;
		align-items: center;
		justify-content: center;
		position: relative;
	}

	.theme-preview.light {
		background:
			radial-gradient(300px 90px at 80% 0%, rgba(78, 205, 203, 0.18), transparent 60%), #f4f6fa;
		color: #171a21;
	}

	.theme-preview.dark {
		background:
			radial-gradient(300px 90px at 80% 0%, rgba(148, 239, 238, 0.14), transparent 60%), #0f1117;
		color: #f2f4fa;
	}

	.mini-card {
		width: 70%;
		height: 56px;
		display: flex;
		align-items: center;
		gap: 8px;
		padding: 0 12px;
		border-radius: 10px;
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.18);
		font-size: 11px;
	}

	.theme-preview.light .mini-card {
		background: #fff;
		border: 1px solid rgba(23, 26, 33, 0.08);
	}

	.theme-preview.dark .mini-card {
		background: #181c29;
		border: 1px solid rgba(148, 163, 205, 0.14);
	}

	.brand-dot {
		width: 14px;
		height: 14px;
		border-radius: 5px;
		background: var(--gradient-brand);
		flex-shrink: 0;
	}

	.theme-body {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
	}

	.theme-id {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.theme-id-text {
		display: flex;
		flex-direction: column;
	}

	.theme-id-text strong {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
	}

	.theme-id-desc {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.swatches {
		display: flex;
		gap: 5px;
		flex-shrink: 0;
	}

	.swatches span {
		width: 16px;
		height: 16px;
		border-radius: 50%;
		border: 1px solid var(--color-border);
	}

	/* Security Section */
	.security-card {
		max-width: 760px;
	}

	.security-body {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-md);
	}

	.security-body :global(.icon-success) {
		color: var(--color-success);
		flex-shrink: 0;
	}

	.security-title {
		display: block;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
	}

	.security-info-text {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		line-height: var(--line-height-relaxed);
		margin-top: var(--spacing-xs);
	}

	/* Responsive */
	@media (max-width: 768px) {
		.theme-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
