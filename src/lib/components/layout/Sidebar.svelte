<!--
  Sidebar Component
  A collapsible sidebar with header, navigation, and footer slots.
  Supports expand/collapse toggle with smooth transition.

  @example
  <Sidebar bind:collapsed>
    {#snippet header()}
      <h2>Workflows</h2>
    {/snippet}
    {#snippet nav()}
      <NavItem href="/agent" icon={Bot}>Agent</NavItem>
    {/snippet}
    {#snippet footer()}
      <p>Footer content</p>
    {/snippet}
  </Sidebar>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { ChevronLeft, ChevronRight } from 'lucide-svelte';

	/**
	 * Sidebar props
	 */
	interface Props {
		/** Whether the sidebar is collapsed */
		collapsed?: boolean;
		/** Header slot content */
		header?: Snippet;
		/** Navigation slot content */
		nav?: Snippet;
		/** Footer slot content */
		footer?: Snippet;
	}

	let { collapsed = $bindable(false), header, nav, footer }: Props = $props();

	/**
	 * Toggle sidebar collapsed state
	 */
	function toggleCollapsed(): void {
		collapsed = !collapsed;
	}
</script>

<aside class="sidebar" class:collapsed aria-label="Sidebar navigation">
	{#if header}
		<div class="sidebar-header">
			{@render header()}
		</div>
	{/if}

	{#if nav}
		<nav class="sidebar-nav">
			{@render nav()}
		</nav>
	{/if}

	{#if footer}
		<div class="sidebar-footer">
			{@render footer()}
		</div>
	{/if}

	<button
		type="button"
		class="sidebar-toggle"
		onclick={toggleCollapsed}
		aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		aria-expanded={!collapsed}
	>
		{#if collapsed}
			<ChevronRight size={16} />
		{:else}
			<ChevronLeft size={16} />
		{/if}
	</button>
</aside>

<style>
	.sidebar-toggle {
		position: absolute;
		bottom: var(--spacing-md);
		right: calc(-1 * var(--spacing-md));
		width: 24px;
		height: 24px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-full);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition: all var(--transition-fast);
		color: var(--color-text-secondary);
		z-index: 1;
	}

	.sidebar-toggle:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.sidebar {
		position: relative;
	}
</style>
