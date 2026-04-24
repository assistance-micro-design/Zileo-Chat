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
  ValidationInfoCard - Reusable info-card for validation mode displays.
  Used in Auto/Manual modes to show entity validation status.
  Extracted from ValidationSettings to deduplicate info-card blocks.
-->

<script lang="ts">
  import type { Snippet } from 'svelte';
  import { i18n } from '$lib/i18n';

  /**
   * ValidationInfoCard props.
   * @param variant - Visual style: 'approved' (green) or 'validation-required' (orange)
   * @param icon - Icon character displayed in the header
   * @param titleKey - i18n key for the card title
   * @param statusKey - i18n key for the status text
   * @param children - Snippet for the item list content
   */
  interface Props {
    variant: 'approved' | 'validation-required';
    icon: string;
    titleKey: string;
    statusKey: string;
    children?: Snippet;
  }

  let { variant, icon, titleKey, statusKey, children }: Props = $props();
</script>

<div
  class="info-card"
  class:approved={variant === 'approved'}
  class:validation-required={variant === 'validation-required'}
>
  <div class="info-card-header">
    <span class="info-card-icon">{icon}</span>
    <span class="info-card-title">{$i18n(titleKey)}</span>
  </div>
  <span class="info-card-status">{$i18n(statusKey)}</span>
  {#if children}
    {@render children()}
  {/if}
</div>

<style>
  .info-card {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    padding: var(--spacing-md);
    border-radius: var(--border-radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .info-card.approved {
    border-color: var(--color-success);
    background: color-mix(in srgb, var(--color-success) 5%, var(--color-bg-secondary));
  }

  .info-card.validation-required {
    border-color: var(--color-warning);
    background: color-mix(in srgb, var(--color-warning) 5%, var(--color-bg-secondary));
  }

  .info-card-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .info-card-icon {
    font-size: var(--font-size-base);
  }

  .info-card.approved .info-card-icon {
    color: var(--color-success);
  }

  .info-card.validation-required .info-card-icon {
    color: var(--color-warning);
  }

  .info-card-title {
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .info-card-status {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }
</style>
