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
Settings > MCP Page
Manages MCP server configuration.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import MCPSection from '$lib/components/settings/MCPSection.svelte';

	/** Component reference for reload capability */
	let mcpSectionRef: MCPSection;

	/**
	 * Handle cross-page refresh events (from import/export)
	 */
	function handleSettingsRefresh(): void {
		mcpSectionRef?.reload();
	}

	onMount(() => {
		// Only add event listeners in browser context (onMount only runs client-side)
		window.addEventListener('settings:refresh', handleSettingsRefresh);
		return () => {
			window.removeEventListener('settings:refresh', handleSettingsRefresh);
		};
	});
</script>

<MCPSection bind:this={mcpSectionRef} />
