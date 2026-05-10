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
Settings > Memory Page
Manages memory configuration and memory list with lazy loading.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, StatusIndicator } from '$lib/components/ui';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';

	/** Lazy loaded components */
	type LazyMemorySettings =
		typeof import('$lib/components/settings/memory/MemorySettings.svelte').default;
	type LazyMemoryList = typeof import('$lib/components/settings/memory/MemoryList.svelte').default;

	let MemorySettingsComponent = $state<LazyMemorySettings | null>(null);
	let MemoryListComponent = $state<LazyMemoryList | null>(null);
	let loadError = $state<string | null>(null);

	/** Reference for MemorySettings to reload stats after mutations. */
	let memorySettingsRef = $state<{ reload: () => Promise<void> } | undefined>(undefined);

	onSettingsRefresh(() => memorySettingsRef?.reload());

	onMount(() => {
		Promise.all([
			import('$lib/components/settings/memory/MemorySettings.svelte'),
			import('$lib/components/settings/memory/MemoryList.svelte')
		])
			.then(([memorySettingsModule, memoryListModule]) => {
				MemorySettingsComponent = memorySettingsModule.default;
				MemoryListComponent = memoryListModule.default;
			})
			.catch((err: unknown) => {
				loadError = getErrorMessage(err);
			});
	});
</script>

<section class="settings-section">
	<SettingsSectionHeader
		titleKey="settings_memory"
		helpTitleKey="help_memory_title"
		helpDescriptionKey="help_memory_description"
		helpTutorialKey="help_memory_tutorial"
	/>

	{#if MemorySettingsComponent && MemoryListComponent}
		<div class="memory-subsections">
			<!-- Embedding Configuration (card provides its own title) -->
			<MemorySettingsComponent bind:this={memorySettingsRef} />

			<!-- Memory Management -->
			<div class="memory-subsection">
				<h3 class="subsection-title">{$i18n('memory_management')}</h3>
				<MemoryListComponent onchange={() => memorySettingsRef?.reload()} />
			</div>
		</div>
	{:else if loadError}
		<Card>
			{#snippet body()}
				<div class="lazy-loading">
					<StatusIndicator status="error" />
					<span>{loadError}</span>
				</div>
			{/snippet}
		</Card>
	{:else}
		<Card>
			{#snippet body()}
				<div class="lazy-loading">
					<StatusIndicator status="running" />
					<span>{$i18n('common_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{/if}
</section>

<style>
	.memory-subsections {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xl);
	}

	.memory-subsection {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.subsection-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
		padding-bottom: var(--spacing-sm);
		border-bottom: 1px solid var(--color-border);
	}
</style>
