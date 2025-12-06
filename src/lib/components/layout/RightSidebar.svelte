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
  RightSidebar Component
  A collapsible right sidebar with header and content slots.
  Supports expand/collapse toggle with smooth transition.
  Passes collapsed state to snippets.

  @example
  <RightSidebar bind:collapsed>
    {#snippet header(isCollapsed)}
      {#if isCollapsed}
        <IconButton />
      {:else}
        <h2>Activity</h2>
      {/if}
    {/snippet}
    {#snippet content(isCollapsed)}
      <ActivityFeed collapsed={isCollapsed} />
    {/snippet}
  </RightSidebar>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';
	import { PanelRightClose, PanelRightOpen } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * RightSidebar props
	 */
	interface Props {
		/** Whether the sidebar is collapsed */
		collapsed?: boolean;
		/** Header slot content - receives collapsed state */
		header?: Snippet<[boolean]>;
		/** Content slot content - receives collapsed state */
		content?: Snippet<[boolean]>;
	}

	let { collapsed = $bindable(false), header, content }: Props = $props();

	/**
	 * Toggle sidebar collapsed state
	 */
	function toggleCollapsed(): void {
		collapsed = !collapsed;
	}
</script>

<aside class="right-sidebar" class:collapsed aria-label={$i18n('layout_activity_sidebar')}>
	{#if header}
		<div class="right-sidebar-header">
			{@render header(collapsed)}
		</div>
	{/if}

	{#if content}
		<div class="right-sidebar-content">
			{@render content(collapsed)}
		</div>
	{/if}

	<button
		type="button"
		class="right-sidebar-toggle"
		onclick={toggleCollapsed}
		aria-label={collapsed ? $i18n('layout_expand_sidebar') : $i18n('layout_collapse_sidebar')}
		aria-expanded={!collapsed}
		title={collapsed ? $i18n('layout_expand_sidebar') : $i18n('layout_collapse_sidebar')}
	>
		{#if collapsed}
			<PanelRightOpen size={18} />
		{:else}
			<PanelRightClose size={18} />
		{/if}
	</button>
</aside>

<style>
	.right-sidebar {
		position: relative;
		width: var(--right-sidebar-width, 320px);
		background: var(--color-bg-secondary);
		border-left: 1px solid var(--color-border);
		display: flex;
		flex-direction: column;
		transition: width var(--transition-base);
		overflow: hidden;
		box-shadow: inset 1px 0 0 0 var(--color-border-light);
	}

	.right-sidebar.collapsed {
		width: var(--right-sidebar-collapsed-width, 48px);
	}

	.right-sidebar-header {
		padding: var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: linear-gradient(180deg, var(--color-bg-secondary) 0%, var(--color-bg-tertiary) 100%);
		min-height: 72px;
	}

	.right-sidebar.collapsed .right-sidebar-header {
		padding: var(--spacing-md) var(--spacing-sm);
		display: flex;
		flex-direction: column;
		justify-content: center;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.right-sidebar-content {
		flex: 1;
		overflow-y: auto;
		padding: var(--spacing-md);
		scrollbar-width: thin;
		scrollbar-color: var(--color-border) transparent;
	}

	.right-sidebar-content::-webkit-scrollbar {
		width: 6px;
	}

	.right-sidebar-content::-webkit-scrollbar-track {
		background: transparent;
	}

	.right-sidebar-content::-webkit-scrollbar-thumb {
		background: var(--color-border);
		border-radius: var(--border-radius-full);
	}

	.right-sidebar-content::-webkit-scrollbar-thumb:hover {
		background: var(--color-border-dark);
	}

	.right-sidebar.collapsed .right-sidebar-content {
		padding: var(--spacing-sm);
		overflow-x: hidden;
	}

	.right-sidebar-toggle {
		position: absolute;
		bottom: var(--spacing-lg);
		left: calc(-1 * var(--spacing-md) - 2px);
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

	.right-sidebar-toggle:hover {
		background: var(--color-accent);
		border-color: var(--color-accent);
		color: var(--color-accent-text);
		transform: scale(1.05);
		box-shadow: var(--shadow-md);
	}

	.right-sidebar-toggle:active {
		transform: scale(0.95);
	}
</style>
