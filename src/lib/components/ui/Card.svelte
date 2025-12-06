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
		/** Card title (used with default header) */
		title?: string;
		/** Card description (used with default header) */
		description?: string;
		/** Custom header slot (overrides title/description) */
		header?: Snippet;
		/** Header actions slot (rendered in header right side) */
		headerActions?: Snippet;
		/** Main content slot */
		body?: Snippet;
		/** Footer content slot */
		footer?: Snippet;
	}

	let { title, description, header, headerActions, body, footer }: Props = $props();
</script>

<div class="card">
	{#if header || title || headerActions}
		<div class="card-header">
			{#if header}
				{@render header()}
			{:else}
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
