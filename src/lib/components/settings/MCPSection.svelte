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
MCP Servers Section - Extracted from Settings page
Manages MCP server configuration: list, create, edit, delete, test, start/stop.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		LegacyHttpAuthWarning,
		MCPServer,
		MCPServerConfig,
		MCPServerConfigWithSecret,
		MCPTestResult
	} from '$types/mcp';
	import { Card, Button, StatusIndicator, Modal, DeleteConfirmModal, ErrorBanner } from '$lib/components/ui';
	import { MCPServerCard, MCPServerForm, MCPServerTester } from '$lib/components/mcp';
	import SettingsSectionHeader from './SettingsSectionHeader.svelte';
	import {
		createInitialMCPState,
		setServers,
		addServer,
		removeServer,
		updateServer,
		setMCPLoading,
		setMCPError,
		setTestingServer,
		loadServers,
		createServer,
		updateServerConfig,
		deleteServer,
		testServer,
		startServer,
		stopServer,
		listLegacyHttpAuth,
		type MCPState
	} from '$lib/stores/mcp';
	import { Plus, Plug } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { createModalController } from '$lib/utils/modal.svelte';
	import type { ModalController } from '$lib/utils/modal.svelte';
	import { getErrorMessage } from '$lib/utils/error';
	import { dispatchSettingsRefresh } from '$lib/utils/settings-refresh';

	/** MCP state */
	let mcpState = $state<MCPState>(createInitialMCPState());
	const mcpModal: ModalController<MCPServerConfig> = createModalController<MCPServerConfig>();
	let mcpSaving = $state(false);
	let mcpWarning = $state<string | null>(null);
	let testResult = $state<MCPTestResult | null>(null);
	let testError = $state<string | null>(null);
	let showTestModal = $state(false);
	let testingServerConfig = $state<MCPServerConfig | null>(null);

	/** Delete confirmation state */
	let showDeleteConfirm = $state(false);
	let serverToDelete = $state<MCPServer | null>(null);
	let serverDeleting = $state(false);

	/** Legacy HTTP auth migration state. */
	let legacyAuthWarnings = $state<LegacyHttpAuthWarning[]>([]);
	let legacyBannerDismissed = $state(false);

	/**
	 * Loads MCP servers from backend
	 */
	async function loadMCPServers(): Promise<void> {
		mcpState = setMCPLoading(mcpState, true);
		try {
			const servers = await loadServers();
			mcpState = setServers(mcpState, servers);
		} catch (err) {
			mcpState = setMCPError(mcpState, $i18n('settings_mcp_load_failed', { error: getErrorMessage(err) }));
		}
	}

	/**
	 * Opens the edit server modal (create uses mcpModal.openCreate() directly).
	 * Auth fields are propagated so the form can pre-fill the auth method and
	 * metadata without exposing any secret.
	 */
	function openEditModal(server: MCPServer): void {
		mcpModal.openEdit({
			id: server.id,
			name: server.name,
			enabled: server.enabled,
			command: server.command,
			args: server.args,
			env: server.env,
			description: server.description,
			authType: server.authType,
			authMetadata: server.authMetadata,
			extraHeaders: server.extraHeaders
		});
	}

	/**
	 * Saves an MCP server (create or update). Accepts a config that may
	 * include an `authSecret` payload; the backend persists the secret in
	 * the OS keychain and never returns it on read.
	 */
	async function handleSaveMCPServer(config: MCPServerConfigWithSecret): Promise<void> {
		mcpSaving = true;
		mcpWarning = null;
		try {
			if (mcpModal.mode === 'create') {
				const response = await createServer(config);
				mcpState = addServer(mcpState, response.server);
				mcpWarning = response.warning ?? null;
			} else {
				const response = await updateServerConfig(config.id, config);
				mcpState = updateServer(mcpState, config.id, response.server);
				mcpWarning = response.warning ?? null;
			}
			mcpModal.close();
			// Refresh the legacy banner: a successful save likely cleared a warning.
			refreshLegacyAuthWarnings();
			dispatchSettingsRefresh();
		} catch (err) {
			mcpState = setMCPError(mcpState, $i18n('settings_mcp_save_failed', { error: getErrorMessage(err) }));
		} finally {
			mcpSaving = false;
		}
	}

	/**
	 * Reloads the list of HTTP MCP servers still relying on legacy env-var
	 * authentication. Failures are silent — the banner just stays hidden.
	 */
	async function refreshLegacyAuthWarnings(): Promise<void> {
		try {
			legacyAuthWarnings = await listLegacyHttpAuth();
		} catch {
			legacyAuthWarnings = [];
		}
	}

	/**
	 * Opens the edit modal for the first server flagged in the legacy banner.
	 * If the server has been deleted between mount and click, falls back to
	 * dismissing the banner.
	 */
	function configureLegacyAuth(): void {
		const first = legacyAuthWarnings[0];
		if (!first) {
			legacyBannerDismissed = true;
			return;
		}
		const target = mcpState.servers.find((s) => s.id === first.id);
		if (target) {
			openEditModal(target);
		} else {
			legacyBannerDismissed = true;
		}
	}

	/**
	 * Requests delete confirmation for an MCP server
	 */
	function handleDeleteServerRequest(server: MCPServer): void {
		serverToDelete = server;
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes server deletion
	 */
	async function confirmDeleteServer(): Promise<void> {
		if (!serverToDelete) return;
		serverDeleting = true;
		try {
			await deleteServer(serverToDelete.id);
			const deletedId = serverToDelete.id;
			mcpState = removeServer(mcpState, deletedId);
			legacyAuthWarnings = legacyAuthWarnings.filter((w) => w.id !== deletedId);
			showDeleteConfirm = false;
			serverToDelete = null;
			dispatchSettingsRefresh();
		} catch (err) {
			mcpState = setMCPError(mcpState, $i18n('settings_mcp_delete_failed', { error: getErrorMessage(err) }));
		} finally {
			serverDeleting = false;
		}
	}

	/**
	 * Cancels delete confirmation
	 */
	function cancelDeleteServer(): void {
		showDeleteConfirm = false;
		serverToDelete = null;
	}

	/**
	 * Tests an MCP server connection
	 */
	async function handleTestServer(server: MCPServer): Promise<void> {
		mcpState = setTestingServer(mcpState, server.id);
		testResult = null;
		testError = null;
		testingServerConfig = {
			id: server.id,
			name: server.name,
			enabled: server.enabled,
			command: server.command,
			args: server.args,
			env: server.env,
			description: server.description,
			authType: server.authType,
			authMetadata: server.authMetadata,
			extraHeaders: server.extraHeaders
		};
		showTestModal = true;

		try {
			const result = await testServer(testingServerConfig);
			testResult = result;
		} catch (err) {
			testError = getErrorMessage(err);
		} finally {
			mcpState = setTestingServer(mcpState, null);
		}
	}

	/**
	 * Retries the current test
	 */
	async function handleRetryTest(): Promise<void> {
		if (!testingServerConfig) return;

		mcpState = setTestingServer(mcpState, testingServerConfig.id);
		testResult = null;
		testError = null;

		try {
			const result = await testServer(testingServerConfig);
			testResult = result;
		} catch (err) {
			testError = getErrorMessage(err);
		} finally {
			mcpState = setTestingServer(mcpState, null);
		}
	}

	/**
	 * Closes the test modal
	 */
	function closeTestModal(): void {
		showTestModal = false;
		testResult = null;
		testError = null;
		testingServerConfig = null;
	}

	/**
	 * Toggles server start/stop
	 */
	async function handleToggleServer(server: MCPServer): Promise<void> {
		try {
			let updatedServer: MCPServer;
			if (server.status === 'running') {
				updatedServer = await stopServer(server.id);
			} else {
				updatedServer = await startServer(server.id);
			}
			mcpState = updateServer(mcpState, server.id, updatedServer);
		} catch (err) {
			mcpState = setMCPError(mcpState, $i18n('settings_mcp_toggle_failed', { error: getErrorMessage(err) }));
		}
	}

	/**
	 * Reloads MCP servers (exposed for parent component)
	 */
	export function reload(): void {
		loadMCPServers();
	}

	onMount(() => {
		loadMCPServers();
		refreshLegacyAuthWarnings();
	});
</script>

<section id="mcp" class="settings-section">
	<SettingsSectionHeader
		titleKey="settings_mcp_servers"
		helpTitleKey="help_mcp_title"
		helpDescriptionKey="help_mcp_description"
		helpTutorialKey="help_mcp_tutorial"
		createLabelKey="mcp_add_server"
		onCreate={() => mcpModal.openCreate()}
	/>

	{#if mcpWarning}
		<ErrorBanner variant="warning" message={mcpWarning} onDismiss={() => (mcpWarning = null)} />
	{/if}

	{#if mcpState.error}
		<ErrorBanner message={mcpState.error} onDismiss={() => (mcpState = setMCPError(mcpState, null))} />
	{/if}

	{#if !legacyBannerDismissed && legacyAuthWarnings.length > 0}
		<div class="legacy-auth-banner" role="region" aria-label={$i18n('mcp_auth_legacy_banner_title')}>
			<div class="legacy-auth-text">
				<strong>{$i18n('mcp_auth_legacy_banner_title')}</strong>
				<p>{$i18n('mcp_auth_legacy_banner_body')}</p>
				<ul class="legacy-auth-list">
					{#each legacyAuthWarnings as warning (warning.id)}
						<li>{warning.name}</li>
					{/each}
				</ul>
			</div>
			<div class="legacy-auth-actions">
				<Button variant="primary" size="sm" onclick={configureLegacyAuth}>
					{$i18n('mcp_auth_legacy_banner_action')}
				</Button>
				<Button variant="ghost" size="sm" onclick={() => (legacyBannerDismissed = true)}>
					{$i18n('common_close')}
				</Button>
			</div>
		</div>
	{/if}

	{#if mcpState.loading}
		<Card>
			{#snippet body()}
				<div class="mcp-loading">
					<StatusIndicator status="running" />
					<span>{$i18n('mcp_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if mcpState.servers.length === 0}
		<Card>
			{#snippet body()}
				<div class="mcp-empty">
					<Plug size={48} class="empty-icon" />
					<h3 class="empty-title">{$i18n('mcp_not_configured')}</h3>
					<p class="empty-description">
						{$i18n('mcp_description')}
					</p>
					<Button variant="primary" onclick={() => mcpModal.openCreate()}>
						<Plus size={16} />
						<span>{$i18n('mcp_add_first')}</span>
					</Button>
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="mcp-server-grid">
			{#each mcpState.servers as server (server.id)}
				<MCPServerCard
					{server}
					testing={mcpState.testingServerId === server.id}
					onEdit={() => openEditModal(server)}
					onTest={() => handleTestServer(server)}
					onToggle={() => handleToggleServer(server)}
					onDelete={() => handleDeleteServerRequest(server)}
				/>
			{/each}
		</div>
	{/if}
</section>

<!-- MCP Server Modal (Create/Edit) -->
<Modal
	open={mcpModal.show}
	title={mcpModal.mode === 'create' ? $i18n('mcp_modal_add') : $i18n('mcp_modal_edit')}
	onclose={() => mcpModal.close()}
>
	{#snippet body()}
		<MCPServerForm
			mode={mcpModal.mode}
			server={mcpModal.editing}
			onsave={handleSaveMCPServer}
			oncancel={() => mcpModal.close()}
			saving={mcpSaving}
		/>
	{/snippet}
</Modal>

<!-- MCP Server Test Modal -->
<Modal
	open={showTestModal}
	title={$i18n('settings_mcp_test_title', { name: testingServerConfig?.name ?? 'Server' })}
	onclose={closeTestModal}
>
	{#snippet body()}
		<MCPServerTester
			result={testResult}
			loading={mcpState.testingServerId !== null}
			error={testError}
			onRetry={handleRetryTest}
		/>
	{/snippet}
	{#snippet footer()}
		<Button variant="ghost" onclick={closeTestModal}>
			{$i18n('common_close')}
		</Button>
	{/snippet}
</Modal>

<!-- Server Delete Confirmation Modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="settings_mcp_delete_title"
	confirmMessageKey="settings_mcp_delete_confirm_msg"
	deleting={serverDeleting}
	itemName={serverToDelete?.name}
	onConfirm={confirmDeleteServer}
	onCancel={cancelDeleteServer}
/>

<style>
	/* MCP Servers */
	.mcp-server-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
		contain: layout style; /* Isolate layout recalculations */
	}

	.mcp-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	.mcp-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: var(--spacing-2xl);
		gap: var(--spacing-md);
	}

	.mcp-empty :global(.empty-icon) {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.empty-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.empty-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		max-width: 400px;
	}

	.mcp-empty :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.legacy-auth-banner {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		margin-bottom: var(--spacing-md);
		background: var(--color-warning-bg);
		border: 1px solid var(--color-warning);
		border-radius: var(--border-radius-md);
	}

	.legacy-auth-text strong {
		display: block;
		color: var(--color-warning);
		margin-bottom: var(--spacing-xs);
		font-weight: var(--font-weight-semibold);
	}

	.legacy-auth-text p {
		margin: 0 0 var(--spacing-xs) 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.legacy-auth-list {
		margin: 0;
		padding-left: var(--spacing-lg);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.legacy-auth-actions {
		display: flex;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	/* Responsive */
	@media (max-width: 768px) {
		.mcp-server-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
