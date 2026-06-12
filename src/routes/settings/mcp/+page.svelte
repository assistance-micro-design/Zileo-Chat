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
	import { invoke } from '@tauri-apps/api/core';
	import MCPSection from '$lib/components/settings/MCPSection.svelte';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { Card, Switch } from '$lib/components/ui';
	import { onSettingsRefresh } from '$lib/utils/settings-refresh';
	import { i18n } from '$lib/i18n';
	import { toastStore } from '$lib/stores/toast';
	import { getErrorMessage } from '$lib/utils/error';
	import { TriangleAlert } from '@lucide/svelte';
	import type { McpNetworkSettings } from '$types/mcp-network';

	/**
	 * Component reference for the reload fallback (MCPSection owns local
	 * state that is not exposed via a store, so we call ref.reload() here).
	 */
	let mcpSectionRef: MCPSection;

	// Skip the echo of our own MCPSection save/delete dispatches.
	onSettingsRefresh(() => mcpSectionRef?.reload(), { ignoreSource: 'mcp' });

	/** Network connectivity opt-in (LAN). Disabled by default. */
	let allowPrivateNetwork = $state(false);
	let networkLoading = $state(true);
	let networkSaving = $state(false);

	onMount(async () => {
		try {
			const settings = await invoke<McpNetworkSettings>('get_mcp_network_settings');
			allowPrivateNetwork = settings.allowPrivateNetwork;
		} catch (err) {
			toastStore.add({
				type: 'error',
				title: $i18n('mcp_network_save_error'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 5000
			});
		} finally {
			networkLoading = false;
		}
	});

	async function handleLanToggle(next: boolean): Promise<void> {
		const previous = allowPrivateNetwork;
		allowPrivateNetwork = next;
		networkSaving = true;
		try {
			const updated = await invoke<McpNetworkSettings>('update_mcp_network_settings', {
				request: { allowPrivateNetwork: next }
			});
			allowPrivateNetwork = updated.allowPrivateNetwork;
			toastStore.add({
				type: 'success',
				title: $i18n('mcp_network_saved'),
				message: $i18n(
					updated.allowPrivateNetwork
						? 'mcp_network_enabled_message'
						: 'mcp_network_disabled_message'
				),
				persistent: false,
				duration: 2500
			});
		} catch (err) {
			// Roll the toggle back to its persisted value on failure.
			allowPrivateNetwork = previous;
			toastStore.add({
				type: 'error',
				title: $i18n('mcp_network_save_error'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 6000
			});
		} finally {
			networkSaving = false;
		}
	}
</script>

<section class="network-section">
	<SettingsSectionHeader
		titleKey="mcp_network_title"
		descriptionKey="mcp_network_description"
		helpTitleKey="help_mcp_network_title"
		helpDescriptionKey="help_mcp_network_description"
		helpTutorialKey="help_mcp_network_tutorial"
	/>

	<div class="network-card">
		<Card>
			{#snippet body()}
				<div class="network-body">
					<div class="toggle-row">
						<span class="toggle-text">
							<strong id="mcp-network-lan-label">{$i18n('mcp_network_lan_label')}</strong>
							<span>{$i18n('mcp_network_lan_help')}</span>
						</span>
						<Switch
							checked={allowPrivateNetwork}
							disabled={networkLoading || networkSaving}
							onchange={handleLanToggle}
							labelledBy="mcp-network-lan-label"
						/>
					</div>

					{#if allowPrivateNetwork}
						<div class="lan-warning" role="alert">
							<TriangleAlert size={18} aria-hidden="true" />
							<div class="lan-warning-text">
								<strong>{$i18n('mcp_network_lan_warning_title')}</strong>
								<span>{$i18n('mcp_network_lan_warning_body')}</span>
								<span>{$i18n('mcp_network_lan_auth_note')}</span>
							</div>
						</div>
					{/if}
				</div>
			{/snippet}
		</Card>
	</div>
</section>

<MCPSection bind:this={mcpSectionRef} />

<style>
	.network-section {
		margin-bottom: var(--spacing-xl);
	}

	.network-section :global(.settings-header) {
		margin-bottom: var(--spacing-lg);
	}

	.network-card {
		max-width: 720px;
	}

	.network-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.toggle-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
	}

	.toggle-text strong {
		display: block;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.toggle-text span {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: 2px;
		max-width: 56ch;
	}

	.lan-warning {
		display: flex;
		gap: var(--spacing-sm);
		align-items: flex-start;
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		background: var(--color-warning-light);
		border: 1px solid rgba(217, 144, 12, 0.35);
		color: var(--color-warning);
	}

	.lan-warning-text {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		font-size: var(--font-size-sm);
	}
</style>
