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
Settings > Providers Page
Manages LLM providers and models configuration.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import type { ProviderType } from '$types/llm';
	import LLMSection from '$lib/components/settings/LLMSection.svelte';
	import APIKeysSection from '$lib/components/settings/APIKeysSection.svelte';
	import type { ProviderSettings } from '$types/llm';

	/** Component reference for reload capability */
	let llmSectionRef: LLMSection;

	/** API Key Modal state */
	let showApiKeyModal = $state(false);
	let apiKeyProvider = $state<ProviderType>('mistral');
	let apiKeyProviderDisplayName = $state<string | undefined>(undefined);
	let apiKeyProviderSettings = $state<ProviderSettings | null>(null);
	let apiKeyHasKey = $state(false);
	let apiKeyIsCustom = $state(false);

	/**
	 * Opens API key configuration modal
	 */
	function handleConfigureApiKey(provider: ProviderType, displayName?: string, isCustom?: boolean): void {
		apiKeyProvider = provider;
		apiKeyProviderDisplayName = displayName;
		apiKeyIsCustom = isCustom ?? false;
		showApiKeyModal = true;
	}

	/**
	 * Reloads LLM data after API key changes
	 */
	function handleApiKeyReload(): void {
		llmSectionRef?.reload();
	}

	/**
	 * Handle cross-page refresh events (from import/export)
	 */
	function handleSettingsRefresh(): void {
		llmSectionRef?.reload();
	}

	onMount(() => {
		// Only add event listeners in browser context (onMount only runs client-side)
		window.addEventListener('settings:refresh', handleSettingsRefresh);
		return () => {
			window.removeEventListener('settings:refresh', handleSettingsRefresh);
		};
	});
</script>

<LLMSection
	bind:this={llmSectionRef}
	onConfigureApiKey={handleConfigureApiKey}
/>

<!-- API Key Modal -->
<APIKeysSection
	open={showApiKeyModal}
	provider={apiKeyProvider}
	providerDisplayName={apiKeyProviderDisplayName}
	providerSettings={apiKeyProviderSettings}
	hasApiKey={apiKeyHasKey}
	isCustom={apiKeyIsCustom}
	onclose={() => { showApiKeyModal = false; }}
	onReload={handleApiKeyReload}
/>
