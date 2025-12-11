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
  Sidebar Component
  A collapsible sidebar with header, navigation, and footer slots.
  Supports expand/collapse toggle with smooth transition.
  Passes collapsed state to snippets via context.

  @example
  <Sidebar bind:collapsed>
    {#snippet header(isCollapsed)}
      {#if isCollapsed}
        <IconButton />
      {:else}
        <h2>Workflows</h2>
      {/if}
    {/snippet}
    {#snippet nav(isCollapsed)}
      <WorkflowList collapsed={isCollapsed} />
    {/snippet}
    {#snippet footer()}
      <p>Footer content</p>
    {/snippet}
  </Sidebar>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { PanelLeftClose, PanelLeftOpen } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Sidebar props
	 */
	interface Props {
		/** Whether the sidebar is collapsed */
		collapsed?: boolean;
		/** Header slot content - receives collapsed state */
		header?: Snippet<[boolean]>;
		/** Navigation slot content - receives collapsed state */
		nav?: Snippet<[boolean]>;
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

<aside class="sidebar" class:collapsed aria-label={$i18n('layout_sidebar_navigation')}>
	{#if header}
		<div class="sidebar-header">
			{@render header(collapsed)}
		</div>
	{/if}

	{#if nav}
		<nav class="sidebar-nav">
			{@render nav(collapsed)}
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
		aria-label={collapsed ? $i18n('layout_expand_sidebar') : $i18n('layout_collapse_sidebar')}
		aria-expanded={!collapsed}
		title={collapsed ? $i18n('layout_expand_sidebar') : $i18n('layout_collapse_sidebar')}
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
		transition: background-color var(--transition-fast), border-color var(--transition-fast), color var(--transition-fast), transform var(--transition-fast);
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
