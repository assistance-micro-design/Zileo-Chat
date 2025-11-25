<!--
  Card Component
  A container component with header, body, and footer slots.

  @example
  <Card title="Settings" description="Configure your preferences">
    {#snippet body()}
      <p>Card content here</p>
    {/snippet}
    {#snippet footer()}
      <Button>Save</Button>
    {/snippet}
  </Card>
-->
<script lang="ts">
	import type { Snippet } from 'svelte';

	/**
	 * Card props
	 */
	interface Props {
		/** Card title */
		title?: string;
		/** Card description */
		description?: string;
		/** Header actions slot (rendered in header right side) */
		headerActions?: Snippet;
		/** Main content slot */
		body?: Snippet;
		/** Footer content slot */
		footer?: Snippet;
	}

	let { title, description, headerActions, body, footer }: Props = $props();
</script>

<div class="card">
	{#if title || headerActions}
		<div class="card-header">
			<div>
				{#if title}
					<h3 class="card-title">{title}</h3>
				{/if}
				{#if description}
					<p class="card-description">{description}</p>
				{/if}
			</div>
			{#if headerActions}
				<div class="card-header-actions">
					{@render headerActions()}
				</div>
			{/if}
		</div>
	{/if}

	{#if body}
		<div class="card-body">
			{@render body()}
		</div>
	{/if}

	{#if footer}
		<div class="card-footer">
			{@render footer()}
		</div>
	{/if}
</div>

<style>
	.card-header-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}
</style>
