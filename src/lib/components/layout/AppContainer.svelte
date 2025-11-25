<!--
  AppContainer Component
  Root layout container that provides the main app structure.
  Uses a flex column layout to organize FloatingMenu and main content.

  @example
  <AppContainer>
    <FloatingMenu />
    <div class="main-content">
      <Sidebar />
      <main class="content-area">Content</main>
    </div>
  </AppContainer>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * AppContainer props
	 */
	interface Props {
		/** Main content slot */
		children: Snippet;
	}

	let { children }: Props = $props();
</script>

<div class="app-container">
	{@render children()}
</div>
