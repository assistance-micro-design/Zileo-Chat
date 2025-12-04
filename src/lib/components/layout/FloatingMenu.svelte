<!--
  FloatingMenu Component
  A fixed top navigation bar with logo, navigation, and theme toggle.
  Uses backdrop blur for visual depth and stays fixed at the top.

  @example
  <FloatingMenu>
    {#snippet actions()}
      <Button>Custom Action</Button>
    {/snippet}
  </FloatingMenu>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
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
		/** Additional actions slot (rendered before theme toggle) */
		actions?: Snippet;
	}

	let { title = 'Zileo Chat 3', actions }: Props = $props();

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
	<div class="flex items-center gap-md flex-1">
		<h1 class="floating-menu-title">{title}</h1>
	</div>

	<div class="flex items-center gap-md">
		{#if actions}
			{@render actions()}
		{/if}

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

		<a href="/settings" class="btn btn-secondary">
			<Settings size={16} />
			<span class="floating-menu-link-text">{$i18n('layout_configuration')}</span>
		</a>

		<a href="/agent" class="btn btn-primary">
			<Bot size={16} />
			<span class="floating-menu-link-text">{$i18n('layout_agent')}</span>
		</a>
	</div>
</nav>

<style>
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
