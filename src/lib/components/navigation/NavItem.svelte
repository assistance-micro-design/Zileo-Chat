<!--
  NavItem Component
  A navigation item with icon and label for use in sidebars and menus.
  Supports active state and collapsed mode (icon only).

  @example
  <NavItem href="/agent" active={$page.url.pathname === '/agent'}>
    {#snippet icon()}
      <Bot size={20} />
    {/snippet}
    Agent
  </NavItem>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * NavItem props
	 */
	interface Props {
		/** Navigation URL */
		href: string;
		/** Whether the item is currently active */
		active?: boolean;
		/** Icon snippet (rendered left of label) */
		icon?: Snippet;
		/** Label text content */
		children: Snippet;
	}

	let { href, active = false, icon, children }: Props = $props();
</script>

<a {href} class="nav-item" class:active aria-current={active ? 'page' : undefined}>
	{#if icon}
		<span class="nav-icon">
			{@render icon()}
		</span>
	{/if}
	<span class="nav-text">
		{@render children()}
	</span>
</a>
