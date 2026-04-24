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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

SettingsSectionHeader - Reusable header for settings sections.
Three action variants: default create button, custom `actions` snippet, or no action.
-->

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Button, HelpButton } from '$lib/components/ui';
	import { Plus } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** i18n key for the section title */
		titleKey: string;
		/** i18n key for the section description (optional, hidden when omitted) */
		descriptionKey?: string;
		/** i18n key for help button title */
		helpTitleKey: string;
		/** i18n key for help button description */
		helpDescriptionKey: string;
		/** i18n key for help button tutorial */
		helpTutorialKey: string;
		/** i18n key for create button label (used with onCreate) */
		createLabelKey?: string;
		/** Callback when create button is clicked (renders the default create button) */
		onCreate?: () => void;
		/** Custom action snippet rendered in place of the default create button */
		actions?: Snippet;
	}

	let {
		titleKey,
		descriptionKey,
		helpTitleKey,
		helpDescriptionKey,
		helpTutorialKey,
		createLabelKey,
		onCreate,
		actions
	}: Props = $props();
</script>

<header class="settings-header">
	<div class="header-content">
		<div class="header-title-row">
			<h3 class="header-title">{$i18n(titleKey)}</h3>
			<HelpButton
				titleKey={helpTitleKey}
				descriptionKey={helpDescriptionKey}
				tutorialKey={helpTutorialKey}
			/>
		</div>
		{#if descriptionKey}
			<p class="header-description">
				{$i18n(descriptionKey)}
			</p>
		{/if}
	</div>
	{#if actions}
		<div class="header-actions">
			{@render actions()}
		</div>
	{:else if onCreate && createLabelKey}
		<Button variant="primary" size="sm" onclick={onCreate}>
			<Plus size={16} />
			<span>{$i18n(createLabelKey)}</span>
		</Button>
	{/if}
</header>

<style>
	.settings-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--spacing-lg);
	}

	.header-content {
		flex: 1;
	}

	.header-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.header-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.header-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.header-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.settings-header :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}
</style>
