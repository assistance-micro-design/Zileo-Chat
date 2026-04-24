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
	import type { ProviderType, ProviderSettings } from '$types/llm';
	import LLMSection from '$lib/components/settings/providers/LLMSection.svelte';
	import APIKeysSection from '$lib/components/settings/providers/APIKeysSection.svelte';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';

	/**
	 * Component reference for the reload fallback (LLMSection owns local
	 * state; we call ref.reload() instead of routing through a store).
	 */
	let llmSectionRef: LLMSection;

	/** API Key Modal state */
	let showApiKeyModal = $state(false);
	let apiKeyProvider = $state<ProviderType>('mistral');
	let apiKeyProviderDisplayName = $state<string | undefined>(undefined);
	let apiKeyProviderSettings = $state<ProviderSettings | null>(null);
	let apiKeyHasKey = $state(false);
	let apiKeyIsCustom = $state(false);

	/**
	 * Opens API key configuration modal with provider state from LLMSection
	 */
	function handleConfigureApiKey(provider: ProviderType, hasKey: boolean, provSettings: ProviderSettings | null, displayName?: string, isCustom?: boolean): void {
		apiKeyProvider = provider;
		apiKeyHasKey = hasKey;
		apiKeyProviderSettings = provSettings;
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

	onSettingsRefresh(() => llmSectionRef?.reload());
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
