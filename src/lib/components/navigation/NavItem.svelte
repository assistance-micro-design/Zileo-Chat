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
