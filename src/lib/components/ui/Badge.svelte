<!--
  Badge Component
  A small status indicator with semantic color variants.

  @example
  <Badge variant="success">Active</Badge>
  <Badge variant="warning">Pending</Badge>
  <Badge variant="error">Failed</Badge>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * Badge component props
	 */
	interface Props {
		/** Visual variant for semantic meaning */
		variant?: 'primary' | 'success' | 'warning' | 'error';
		/** Badge content */
		children: Snippet;
	}

	let { variant = 'primary', children }: Props = $props();
</script>

<span class="badge badge-{variant}">
	{@render children()}
</span>
