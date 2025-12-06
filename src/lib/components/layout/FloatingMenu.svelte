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
  FloatingMenu Component
  A fixed top navigation bar with logo, navigation, and theme toggle.
  Uses backdrop blur for visual depth and stays fixed at the top.

  Layout: Logo (left) | Navigation (center) | Theme/Language (right)

  @example
  <FloatingMenu />
-->
<script lang="ts">
	import { Sun, Moon, Settings, Bot } from 'lucide-svelte';
	import { theme } from '$lib/stores/theme';
	import { i18n } from '$lib/i18n';
	import LanguageSelector from '$lib/components/ui/LanguageSelector.svelte';

	/**
	 * FloatingMenu props
	 */
	interface Props {
		/** Application title */
		title?: string;
	}

	let { title = 'Zileo Chat' }: Props = $props();

	/**
	 * Current theme value for reactive UI updates
	 */
	let currentTheme = $state<'light' | 'dark'>('light');

	/**
	 * Subscribe to theme changes
	 */
	theme.subscribe((value) => {
		currentTheme = value;
	});

	/**
	 * Toggle theme between light and dark
	 */
	function toggleTheme(): void {
		theme.toggle();
	}
</script>

<nav class="floating-menu" aria-label={$i18n('layout_main_navigation')}>
	<!-- Left: Logo/Title -->
	<div class="menu-left">
		<h1 class="floating-menu-title">{title}</h1>
	</div>

	<!-- Center: Main Navigation -->
	<div class="menu-center">
		<a href="/agent" class="btn btn-primary">
			<Bot size={16} />
			<span class="floating-menu-link-text">{$i18n('layout_agent')}</span>
		</a>

		<a href="/settings" class="btn btn-secondary">
			<Settings size={16} />
			<span class="floating-menu-link-text">{$i18n('layout_configuration')}</span>
		</a>
	</div>

	<!-- Right: Language & Theme -->
	<div class="menu-right">
		<LanguageSelector />

		<button
			type="button"
			class="btn btn-ghost btn-icon"
			onclick={toggleTheme}
			aria-label={currentTheme === 'light' ? $i18n('layout_switch_to_dark_mode') : $i18n('layout_switch_to_light_mode')}
		>
			{#if currentTheme === 'light'}
				<Moon size={18} />
			{:else}
				<Sun size={18} />
			{/if}
		</button>
	</div>
</nav>

<style>
	.floating-menu {
		justify-content: space-between;
	}

	.menu-left {
		flex: 1;
		display: flex;
		align-items: center;
	}

	.menu-center {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.menu-right {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--spacing-md);
	}

	.floating-menu-title {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.floating-menu-link-text {
		display: inline;
	}

	@media (max-width: 640px) {
		.floating-menu-link-text {
			display: none;
		}
	}
</style>
