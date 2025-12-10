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
	import { Card, StatusIndicator, HelpButton } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	/** Lazy loaded components */
	type LazyMemorySettings = typeof import('$lib/components/settings/memory/MemorySettings.svelte').default;
	type LazyMemoryList = typeof import('$lib/components/settings/memory/MemoryList.svelte').default;

	let MemorySettingsComponent = $state<LazyMemorySettings | null>(null);
	let MemoryListComponent = $state<LazyMemoryList | null>(null);

	/** Reference for MemorySettings to refresh stats */
	let memorySettingsRef = $state<{ refreshStats: () => Promise<void> } | undefined>(undefined);

	onMount(() => {
		// Lazy load memory components in parallel
		Promise.all([
			import('$lib/components/settings/memory/MemorySettings.svelte'),
			import('$lib/components/settings/memory/MemoryList.svelte')
		])
			.then(([memorySettingsModule, memoryListModule]) => {
				MemorySettingsComponent = memorySettingsModule.default;
				MemoryListComponent = memoryListModule.default;
			})
			.catch((err: unknown) => {
				console.warn('[Settings/Memory] Failed to lazy load components:', err);
			});
	});
</script>

<section class="settings-section">
	<div class="section-title-row">
		<h2 class="section-title">{$i18n('settings_memory')}</h2>
		<HelpButton
			titleKey="help_memory_title"
			descriptionKey="help_memory_description"
			tutorialKey="help_memory_tutorial"
		/>
	</div>

	{#if MemorySettingsComponent && MemoryListComponent}
		<div class="memory-subsections">
			<!-- Embedding Configuration -->
			<div class="memory-subsection">
				<h3 class="subsection-title">{$i18n('memory_embedding_config')}</h3>
				<MemorySettingsComponent bind:this={memorySettingsRef} />
			</div>

			<!-- Memory Management -->
			<div class="memory-subsection">
				<h3 class="subsection-title">{$i18n('memory_management')}</h3>
				<MemoryListComponent onchange={() => memorySettingsRef?.refreshStats()} />
			</div>
		</div>
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
	.settings-section {
		margin-bottom: var(--spacing-2xl);
		padding-bottom: var(--spacing-xl);
	}

	.section-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row .section-title {
		margin-bottom: 0;
	}

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

	.lazy-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}
</style>
