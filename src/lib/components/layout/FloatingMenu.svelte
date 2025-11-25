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

<nav class="floating-menu" aria-label="Main navigation">
	<div class="flex items-center gap-md flex-1">
		<h1 class="floating-menu-title">{title}</h1>
	</div>

	<div class="flex items-center gap-md">
		{#if actions}
			{@render actions()}
		{/if}

		<button
			type="button"
			class="btn btn-ghost btn-icon"
			onclick={toggleTheme}
			aria-label={currentTheme === 'light' ? 'Switch to dark mode' : 'Switch to light mode'}
		>
			{#if currentTheme === 'light'}
				<Moon size={18} />
			{:else}
				<Sun size={18} />
			{/if}
		</button>

		<a href="/settings" class="btn btn-secondary">
			<Settings size={16} />
			<span class="floating-menu-link-text">Configuration</span>
		</a>

		<a href="/agent" class="btn btn-primary">
			<Bot size={16} />
			<span class="floating-menu-link-text">Agent</span>
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
