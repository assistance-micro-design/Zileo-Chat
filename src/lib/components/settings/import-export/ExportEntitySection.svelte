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
  ExportEntitySection - Collapsible entity section for ExportPreview.
  Extracts the common pattern of expandable Card with header button + item list.
  Extracted from ExportPreview to deduplicate collapsible sections.
-->

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Card, Badge } from '$lib/components/ui';

  /**
   * ExportEntitySection props.
   * @param title - Section title text
   * @param count - Number of items (shown as badge)
   * @param expanded - Whether the section is expanded
   * @param onToggle - Callback to toggle expanded state
   * @param children - Snippet for the expanded content (item list)
   */
  interface Props {
    title: string;
    count: number;
    expanded: boolean;
    onToggle: () => void;
    children?: Snippet;
  }

  let { title, count, expanded, onToggle, children }: Props = $props();
</script>

<Card>
  {#snippet header()}
    <button
      type="button"
      class="section-header"
      aria-expanded={expanded}
      onclick={onToggle}
    >
      <div class="section-title">
        <span class="title-text">{title}</span>
        <Badge variant="primary">{count}</Badge>
      </div>
      <span class="expand-icon" class:expanded>&#x25BC;</span>
    </button>
  {/snippet}
  {#snippet body()}
    {#if expanded && children}
      {@render children()}
    {/if}
  {/snippet}
</Card>

<style>
  .section-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    padding: 0;
    background: none;
    border: none;
    cursor: pointer;
    gap: var(--spacing-md);
    transition: opacity 0.2s;
  }

  .section-header:hover {
    opacity: 0.8;
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .title-text {
    font-size: var(--font-size-md);
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .expand-icon {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
    transition: transform 0.2s;
  }

  .expand-icon.expanded {
    transform: rotate(180deg);
  }
</style>
