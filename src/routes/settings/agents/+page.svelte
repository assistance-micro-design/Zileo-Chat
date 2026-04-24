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
	import { getErrorMessage } from '$lib/utils/error';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';

	/** Lazy loaded AgentSettings component */
	type LazyAgentSettings = typeof import('$lib/components/settings/agents/AgentSettings.svelte').default;
	let AgentSettingsComponent = $state<LazyAgentSettings | null>(null);
	let loadError = $state<string | null>(null);

	/**
	 * Refresh trigger for AgentSettings.
	 * The agent subtree loads its data through the reactive store exposed by
	 * AgentSettings itself; bumping this counter re-runs its internal $effect
	 * to reload without needing a component ref exposed to this route.
	 */
	let agentRefreshKey = $state(0);

	onSettingsRefresh(() => {
		agentRefreshKey++;
	});

	onMount(() => {
		import('$lib/components/settings/agents/AgentSettings.svelte')
			.then((module) => {
				AgentSettingsComponent = module.default;
			})
			.catch((err: unknown) => {
				loadError = getErrorMessage(err);
			});
	});
</script>

<section class="settings-section">
	{#if AgentSettingsComponent}
		<AgentSettingsComponent refreshTrigger={agentRefreshKey} />
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

