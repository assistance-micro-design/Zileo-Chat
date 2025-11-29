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
	import { PanelLeftClose, PanelLeftOpen } from 'lucide-svelte';

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
		title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
	>
		{#if collapsed}
			<PanelLeftOpen size={18} />
		{:else}
			<PanelLeftClose size={18} />
		{/if}
	</button>
</aside>

<style>
	.sidebar-toggle {
		position: absolute;
		bottom: var(--spacing-lg);
		right: calc(-1 * var(--spacing-md) - 2px);
		width: 28px;
		height: 28px;
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		display: flex;
		align-items: center;
		justify-content: center;
		cursor: pointer;
		transition: background-color var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast), transform var(--transition-fast), box-shadow var(--transition-fast);
		color: var(--color-text-tertiary);
		z-index: 10;
		box-shadow: var(--shadow-sm);
	}

	.sidebar-toggle:hover {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: var(--color-accent-text);
		transform: scale(1.05);
		box-shadow: var(--shadow-md);
	}

	.sidebar-toggle:active {
		transform: scale(0.95);
	}

	.sidebar {
		position: relative;
	}
</style>
