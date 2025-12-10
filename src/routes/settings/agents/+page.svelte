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
Settings > Agents Page
Manages agent configuration with lazy loading.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, StatusIndicator } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';

	/** Lazy loaded AgentSettings component */
	type LazyAgentSettings = typeof import('$lib/components/settings/agents/AgentSettings.svelte').default;
	let AgentSettingsComponent = $state<LazyAgentSettings | null>(null);

	/** Refresh trigger for AgentSettings */
	let agentRefreshKey = $state(0);

	/**
	 * Handle cross-page refresh events (from import/export)
	 */
	function handleSettingsRefresh(): void {
		agentRefreshKey++;
	}

	onMount(() => {
		// Lazy load AgentSettings component
		import('$lib/components/settings/agents/AgentSettings.svelte')
			.then((module) => {
				AgentSettingsComponent = module.default;
			})
			.catch((err: unknown) => {
				console.warn('[Settings/Agents] Failed to lazy load component:', err);
			});

		// Only add event listeners in browser context (onMount only runs client-side)
		window.addEventListener('settings:refresh', handleSettingsRefresh);
		return () => {
			window.removeEventListener('settings:refresh', handleSettingsRefresh);
		};
	});
</script>

<section class="settings-section">
	{#if AgentSettingsComponent}
		<AgentSettingsComponent refreshTrigger={agentRefreshKey} />
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

	.lazy-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}
</style>
