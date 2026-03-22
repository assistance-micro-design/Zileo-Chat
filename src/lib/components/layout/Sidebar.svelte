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

	<div class="sidebar-footer sidebar-toggle-footer">
		<button
			type="button"
			class="sidebar-toggle"
			onclick={toggleCollapsed}
			aria-label={collapsed ? $i18n('layout_expand_sidebar') : $i18n('layout_collapse_sidebar')}
			aria-expanded={!collapsed}
			title={collapsed ? $i18n('layout_expand_sidebar') : $i18n('layout_collapse_sidebar')}
		>
			{#if collapsed}
				<PanelLeftOpen size={20} />
			{:else}
				<PanelLeftClose size={20} />
				<span class="toggle-label">{$i18n('layout_collapse_sidebar')}</span>
			{/if}
		</button>
	</div>
</aside>

<style>
	.sidebar {
		position: relative;
	}

	.sidebar-toggle-footer {
		margin-top: auto;
		padding: 0;
		border-top: none;
		background: none;
	}

	.sidebar-toggle {
		width: 100%;
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-md) var(--spacing-lg);
		background: var(--color-bg-tertiary);
		border: none;
		border-top: 1px solid var(--color-border);
		cursor: pointer;
		transition: background-color var(--transition-fast), color var(--transition-fast);
		color: var(--color-accent);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	:global(.sidebar.collapsed) .sidebar-toggle {
		padding: var(--spacing-md) var(--spacing-sm);
	}

	.sidebar-toggle:hover {
		background: var(--color-accent);
		color: var(--color-accent-text);
	}

	.sidebar-toggle:active {
		background: var(--color-accent-dark, var(--color-accent));
		color: var(--color-accent-text);
	}

	.toggle-label {
		white-space: nowrap;
		overflow: hidden;
	}
</style>
