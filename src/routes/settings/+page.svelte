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

Settings Page - Refactored with Design System Components
Uses: Sidebar, NavItem, Card, Button, Input, Select, Badge, StatusIndicator
Includes MCP server configuration section for managing external tool servers.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type { LLMProvider } from '$types/security';
	import type { MCPServer, MCPServerConfig, MCPTestResult } from '$types/mcp';
	import type {
		LLMModel,
		ProviderType,
		CreateModelRequest,
		UpdateModelRequest,
		LLMState
	} from '$types/llm';
	import { Sidebar } from '$lib/components/layout';
	import { Card, Button, Input, Select, StatusIndicator, Modal, HelpButton } from '$lib/components/ui';
	import { MCPServerCard, MCPServerForm, MCPServerTester } from '$lib/components/mcp';
	import { ProviderCard, ModelCard, ModelForm } from '$lib/components/llm';
	import { MemorySettings, MemoryList } from '$lib/components/settings/memory';

	/** Reference to MemorySettings for refreshing stats when memories change */
	let memorySettingsRef: MemorySettings;
	import { AgentSettings } from '$lib/components/settings/agents';
	import { ValidationSettings } from '$lib/components/settings/validation';
	import { PromptSettings } from '$lib/components/settings/prompts';
	import { ImportExportSettings } from '$lib/components/settings/import-export';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { theme, type Theme } from '$lib/stores/theme';
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
		type MCPState
	} from '$lib/stores/mcp';
	import {
		createInitialLLMState,
		setLLMLoading,
		setLLMError,
		setModels,
		setProviderSettings,
		addModel as addModelToState,
		updateModelInState,
		removeModel,
		getModelsByProvider,
		getAllModels,
		getDefaultModel,
		hasApiKey as hasApiKeyInState,
		loadAllLLMData,
		createModel,
		updateModel,
		deleteModel,
		updateProviderSettings
	} from '$lib/stores/llm';
	import { agentStore } from '$lib/stores/agents';
	import { promptStore } from '$lib/stores/prompts';
	import {
		Globe,
		Cpu,
		Palette,
		Sparkles,
		Server,
		Sun,
		Moon,
		ShieldCheck,
		Plus,
		Plug,
		Brain,
		Bot,
		Settings,
		BookOpen,
		FolderSync
	} from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/** Settings state (for API key input) */
	let settings = $state({
		provider: 'Mistral' as LLMProvider,
		apiKey: ''
	});

	/** UI state */
	let saving = $state(false);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
	let activeSection = $state('providers');
	let sidebarCollapsed = $state(false);

	/** MCP state */
	let mcpState = $state<MCPState>(createInitialMCPState());
	let showMCPModal = $state(false);
	let mcpModalMode = $state<'create' | 'edit'>('create');
	let editingServer = $state<MCPServerConfig | undefined>(undefined);
	let mcpSaving = $state(false);
	let testResult = $state<MCPTestResult | null>(null);
	let testError = $state<string | null>(null);
	let showTestModal = $state(false);
	let testingServerConfig = $state<MCPServerConfig | null>(null);

	/** LLM state */
	let llmState = $state<LLMState>(createInitialLLMState());
	let showModelModal = $state(false);
	let modelModalMode = $state<'create' | 'edit'>('create');
	let editingModel = $state<LLMModel | undefined>(undefined);
	let modelSaving = $state(false);
	let selectedModelsProvider = $state<ProviderType | 'all'>('all');
	let showApiKeyModal = $state(false);
	let apiKeyProvider = $state<ProviderType>('mistral');

	/** Agent refresh trigger - increment to force AgentSettings to reload */
	let agentRefreshKey = $state(0);

	/** Navigation sections with i18n keys */
	const sectionDefs = [
		{ id: 'providers', labelKey: 'settings_providers', icon: Globe },
		{ id: 'models', labelKey: 'settings_models', icon: Cpu },
		{ id: 'agents', labelKey: 'settings_agents', icon: Bot },
		{ id: 'mcp', labelKey: 'settings_mcp_servers', icon: Plug },
		{ id: 'memory', labelKey: 'settings_memory', icon: Brain },
		{ id: 'validation', labelKey: 'settings_validation', icon: ShieldCheck },
		{ id: 'prompts', labelKey: 'settings_prompts', icon: BookOpen },
		{ id: 'import-export', labelKey: 'settings_import_export', icon: FolderSync },
		{ id: 'theme', labelKey: 'settings_theme', icon: Palette }
	] as const;

	/**
	 * Current theme value - synced with theme store
	 */
	let currentTheme = $state<Theme>('light');

	/**
	 * Scrolls to section and updates active section
	 */
	function scrollToSection(sectionId: string): void {
		activeSection = sectionId;
		const element = document.getElementById(sectionId);
		if (element) {
			element.scrollIntoView({ behavior: 'smooth', block: 'start' });
		}
	}

	/**
	 * Handle theme change
	 */
	function handleThemeChange(newTheme: Theme): void {
		theme.setTheme(newTheme);
	}

	// MCP Functions

	/**
	 * Loads MCP servers from backend
	 */
	async function loadMCPServers(): Promise<void> {
		mcpState = setMCPLoading(mcpState, true);
		try {
			const servers = await loadServers();
			mcpState = setServers(mcpState, servers);
		} catch (err) {
			mcpState = setMCPError(mcpState, `Failed to load MCP servers: ${err}`);
		}
	}

	/**
	 * Refreshes all data stores after an import operation.
	 * Called when ImportExportSettings signals that new data was imported.
	 */
	async function handleImportRefresh(): Promise<void> {
		// Reload all data stores in parallel
		await Promise.all([
			loadMCPServers(),
			loadLLMData(),
			agentStore.loadAgents(),
			promptStore.loadPrompts()
		]);
		// Increment refresh key to trigger AgentSettings $effect
		// This ensures the UI updates even if store subscription doesn't propagate
		agentRefreshKey++;
	}

	/**
	 * Opens the create server modal
	 */
	function openCreateModal(): void {
		mcpModalMode = 'create';
		editingServer = undefined;
		showMCPModal = true;
	}

	/**
	 * Opens the edit server modal
	 */
	function openEditModal(server: MCPServer): void {
		mcpModalMode = 'edit';
		editingServer = {
			id: server.id,
			name: server.name,
			enabled: server.enabled,
			command: server.command,
			args: server.args,
			env: server.env,
			description: server.description
		};
		showMCPModal = true;
	}

	/**
	 * Closes the MCP modal
	 */
	function closeMCPModal(): void {
		showMCPModal = false;
		editingServer = undefined;
	}

	/**
	 * Saves an MCP server (create or update)
	 */
	async function handleSaveMCPServer(config: MCPServerConfig): Promise<void> {
		mcpSaving = true;
		try {
			if (mcpModalMode === 'create') {
				const server = await createServer(config);
				mcpState = addServer(mcpState, server);
			} else {
				const server = await updateServerConfig(config.id, config);
				mcpState = updateServer(mcpState, config.id, server);
			}
			closeMCPModal();
		} catch (err) {
			mcpState = setMCPError(mcpState, `Failed to save server: ${err}`);
		} finally {
			mcpSaving = false;
		}
	}

	/**
	 * Deletes an MCP server
	 */
	async function handleDeleteServer(server: MCPServer): Promise<void> {
		if (!confirm(`Are you sure you want to delete "${server.name}"?`)) {
			return;
		}

		try {
			await deleteServer(server.id);
			mcpState = removeServer(mcpState, server.id);
		} catch (err) {
			mcpState = setMCPError(mcpState, `Failed to delete server: ${err}`);
		}
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
			description: server.description
		};
		showTestModal = true;

		try {
			const result = await testServer(testingServerConfig);
			testResult = result;
		} catch (err) {
			testError = `${err}`;
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
			testError = `${err}`;
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
			mcpState = setMCPError(mcpState, `Failed to toggle server: ${err}`);
		}
	}

	// =========================================================================
	// LLM Functions
	// =========================================================================

	/**
	 * Loads all LLM data (providers and models) from the backend
	 */
	async function loadLLMData(): Promise<void> {
		llmState = setLLMLoading(llmState, true);
		try {
			const data = await loadAllLLMData();
			llmState = setProviderSettings(llmState, 'mistral', data.mistral);
			llmState = setProviderSettings(llmState, 'ollama', data.ollama);
			llmState = setModels(llmState, data.models);
		} catch (err) {
			llmState = setLLMError(llmState, `Failed to load LLM data: ${err}`);
		}
	}

	/**
	 * Opens the create model modal
	 */
	function openCreateModelModal(): void {
		modelModalMode = 'create';
		editingModel = undefined;
		showModelModal = true;
	}

	/**
	 * Opens the edit model modal
	 */
	function openEditModelModal(model: LLMModel): void {
		modelModalMode = 'edit';
		editingModel = model;
		showModelModal = true;
	}

	/**
	 * Closes the model modal
	 */
	function closeModelModal(): void {
		showModelModal = false;
		editingModel = undefined;
	}

	/**
	 * Handles model form submission (create or update)
	 */
	async function handleSaveModel(data: CreateModelRequest | UpdateModelRequest): Promise<void> {
		modelSaving = true;
		try {
			if (modelModalMode === 'create') {
				const model = await createModel(data as CreateModelRequest);
				llmState = addModelToState(llmState, model);
				message = { type: 'success', text: `Model "${model.name}" created successfully` };
			} else if (editingModel) {
				const model = await updateModel(editingModel.id, data as UpdateModelRequest);
				llmState = updateModelInState(llmState, editingModel.id, model);
				message = { type: 'success', text: `Model "${model.name}" updated successfully` };
			}
			closeModelModal();
		} catch (err) {
			message = { type: 'error', text: `Failed to save model: ${err}` };
		} finally {
			modelSaving = false;
		}
	}

	/**
	 * Handles model deletion
	 */
	async function handleDeleteModel(model: LLMModel): Promise<void> {
		if (!confirm(`Are you sure you want to delete "${model.name}"?`)) {
			return;
		}

		try {
			await deleteModel(model.id);
			llmState = removeModel(llmState, model.id);
			message = { type: 'success', text: `Model "${model.name}" deleted successfully` };
		} catch (err) {
			message = { type: 'error', text: `Failed to delete model: ${err}` };
		}
	}

	/**
	 * Handles setting a model as the default for its provider
	 */
	async function handleSetDefaultModel(model: LLMModel): Promise<void> {
		try {
			const updatedSettings = await updateProviderSettings(
				model.provider,
				undefined,
				model.id,
				undefined
			);
			llmState = setProviderSettings(llmState, model.provider, updatedSettings);
			message = { type: 'success', text: `"${model.name}" set as default` };
		} catch (err) {
			message = { type: 'error', text: `Failed to set default model: ${err}` };
		}
	}

	/**
	 * Opens the API key configuration modal for a provider
	 */
	function openApiKeyModal(provider: ProviderType): void {
		apiKeyProvider = provider;
		settings.apiKey = '';
		showApiKeyModal = true;
	}

	/**
	 * Closes the API key modal
	 */
	function closeApiKeyModal(): void {
		showApiKeyModal = false;
		settings.apiKey = '';
	}

	/**
	 * Saves API key for the selected provider
	 */
	async function handleSaveApiKey(): Promise<void> {
		if (!settings.apiKey.trim()) {
			message = { type: 'error', text: 'API key cannot be empty' };
			return;
		}

		saving = true;
		message = null;

		try {
			// Map ProviderType to LLMProvider format (capitalize first letter)
			const providerName = apiKeyProvider.charAt(0).toUpperCase() + apiKeyProvider.slice(1);
			await invoke('save_api_key', {
				provider: providerName,
				apiKey: settings.apiKey
			});
			settings.apiKey = '';
			// Reload provider settings to get updated api_key_configured status
			await loadLLMData();
			message = { type: 'success', text: 'API key saved securely' };
			closeApiKeyModal();
		} catch (err) {
			message = { type: 'error', text: `Failed to save: ${err}` };
		} finally {
			saving = false;
		}
	}

	/**
	 * Deletes API key for a provider
	 */
	async function handleDeleteApiKey(provider: ProviderType): Promise<void> {
		if (!confirm(`Are you sure you want to delete the API key for ${provider}?`)) {
			return;
		}

		saving = true;
		message = null;

		try {
			const providerName = provider.charAt(0).toUpperCase() + provider.slice(1);
			await invoke('delete_api_key', { provider: providerName });
			// Reload provider settings
			await loadLLMData();
			message = { type: 'success', text: 'API key deleted' };
		} catch (err) {
			message = { type: 'error', text: `Failed to delete: ${err}` };
		} finally {
			saving = false;
		}
	}

	/**
	 * Handles provider models filter change
	 */
	function handleModelsProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		selectedModelsProvider = event.currentTarget.value as ProviderType | 'all';
	}

	/**
	 * Gets filtered models for the selected provider (or all if 'all' selected)
	 */
	const filteredModels = $derived(
		selectedModelsProvider === 'all'
			? getAllModels(llmState)
			: getModelsByProvider(llmState, selectedModelsProvider)
	);

	/**
	 * Gets the default model for a specific provider
	 */
	function getProviderDefaultModel(provider: ProviderType): LLMModel | undefined {
		return getDefaultModel(llmState, provider);
	}

	/**
	 * Checks if a provider has an API key configured
	 */
	function providerHasApiKey(provider: ProviderType): boolean {
		return hasApiKeyInState(llmState, provider);
	}

	/** Provider filter options for models section */
	const modelsProviderOptions: SelectOption[] = [
		{ value: 'all', label: 'All Providers' },
		{ value: 'mistral', label: 'Mistral' },
		{ value: 'ollama', label: 'Ollama' }
	];

	/**
	 * Reference to content area for IntersectionObserver
	 */
	let contentAreaRef: HTMLElement | null = $state(null);

	/**
	 * Initialize on mount:
	 * - Subscribe to theme store
	 * - Load MCP servers
	 * - Load LLM data
	 * - Setup IntersectionObserver for section detection
	 */
	onMount(() => {
		// Subscribe to theme store and sync value
		const unsubscribeTheme = theme.subscribe((value) => {
			currentTheme = value;
		});

		// Load MCP servers on mount
		loadMCPServers();

		// Load LLM data on mount
		loadLLMData();

		// Setup IntersectionObserver for automatic section detection
		let observer: IntersectionObserver | null = null;

		if (contentAreaRef) {
			observer = new IntersectionObserver(
				(entries) => {
					// Find the entry with the highest intersection ratio
					const visibleEntry = entries.reduce((best, entry) => {
						if (entry.isIntersecting && entry.intersectionRatio > (best?.intersectionRatio || 0)) {
							return entry;
						}
						return best;
					}, null as IntersectionObserverEntry | null);

					if (visibleEntry?.target?.id) {
						activeSection = visibleEntry.target.id;
					}
				},
				{
					root: contentAreaRef,
					threshold: [0.3, 0.5, 0.7],
					rootMargin: '-10% 0px -10% 0px'
				}
			);

			// Observe all sections
			sectionDefs.forEach((section) => {
				const element = document.getElementById(section.id);
				if (element) {
					observer?.observe(element);
				}
			});
		}

		// Cleanup on unmount
		return () => {
			unsubscribeTheme();
			observer?.disconnect();
		};
	});
</script>

<div class="settings-page">
	<!-- Settings Sidebar -->
	<Sidebar bind:collapsed={sidebarCollapsed}>
		{#snippet header()}
			{#if sidebarCollapsed}
				<div class="sidebar-icon-collapsed" title={$i18n('settings_title')}>
					<Settings size={24} />
				</div>
			{:else}
				<h2 class="sidebar-title">{$i18n('settings_title')}</h2>
			{/if}
		{/snippet}

		{#snippet nav()}
			{#if !sidebarCollapsed}
				<div class="nav-items">
					{#each sectionDefs as section}
						{@const Icon = section.icon}
						<button
							type="button"
							class="nav-button"
							class:active={activeSection === section.id}
							onclick={() => scrollToSection(section.id)}
						>
							<Icon size={20} />
							<span class="nav-text">{$i18n(section.labelKey)}</span>
						</button>
					{/each}
				</div>
			{:else}
				<div class="nav-items-collapsed">
					{#each sectionDefs as section}
						{@const Icon = section.icon}
						<button
							type="button"
							class="nav-button-icon"
							class:active={activeSection === section.id}
							onclick={() => scrollToSection(section.id)}
							title={$i18n(section.labelKey)}
						>
							<Icon size={20} />
						</button>
					{/each}
				</div>
			{/if}
		{/snippet}

		{#snippet footer()}
			{#if sidebarCollapsed}
				<div class="security-badge-collapsed" title={$i18n('settings_security_badge')}>
					<ShieldCheck size={20} />
				</div>
			{:else}
				<div class="security-badge">
					<ShieldCheck size={16} />
					<span class="security-text">{$i18n('settings_security_badge')}</span>
				</div>
			{/if}
		{/snippet}
	</Sidebar>

	<!-- Settings Content -->
	<main class="content-area" bind:this={contentAreaRef}>
		<!-- Providers Section -->
		<section id="providers" class="settings-section">
			<div class="section-title-row">
				<h2 class="section-title">{$i18n('settings_providers')}</h2>
				<HelpButton
					titleKey="help_providers_title"
					descriptionKey="help_providers_description"
					tutorialKey="help_providers_tutorial"
				/>
			</div>

			{#if llmState.error}
				<div class="llm-error">
					{llmState.error}
				</div>
			{/if}

			{#if llmState.loading}
				<Card>
					{#snippet body()}
						<div class="llm-loading">
							<StatusIndicator status="running" />
							<span>{$i18n('providers_loading')}</span>
						</div>
					{/snippet}
				</Card>
			{:else}
				<div class="provider-grid">
					<!-- Mistral Provider Card -->
					<ProviderCard
						provider="mistral"
						settings={llmState.providers.mistral}
						hasApiKey={providerHasApiKey('mistral')}
						defaultModel={getProviderDefaultModel('mistral')}
						onConfigure={() => openApiKeyModal('mistral')}
					>
						{#snippet icon()}
							<Sparkles size={24} class="icon-accent" />
						{/snippet}
					</ProviderCard>

					<!-- Ollama Provider Card -->
					<ProviderCard
						provider="ollama"
						settings={llmState.providers.ollama}
						hasApiKey={true}
						defaultModel={getProviderDefaultModel('ollama')}
						onConfigure={() => openApiKeyModal('ollama')}
					>
						{#snippet icon()}
							<Server size={24} class="icon-success" />
						{/snippet}
					</ProviderCard>
				</div>
			{/if}

			{#if message}
				<div class="message-toast" class:success={message.type === 'success'} class:error={message.type === 'error'}>
					{message.text}
				</div>
			{/if}
		</section>

		<!-- Models Section -->
		<section id="models" class="settings-section">
			<div class="section-header-row">
				<div class="section-title-row">
					<h2 class="section-title">{$i18n('settings_models')}</h2>
					<HelpButton
						titleKey="help_models_title"
						descriptionKey="help_models_description"
						tutorialKey="help_models_tutorial"
					/>
				</div>
				<div class="models-header-actions">
					<Select
						options={modelsProviderOptions}
						value={selectedModelsProvider}
						onchange={handleModelsProviderChange}
					/>
					<Button variant="primary" size="sm" onclick={openCreateModelModal}>
						<Plus size={16} />
						<span>{$i18n('models_add')}</span>
					</Button>
				</div>
			</div>

			{#if llmState.loading}
				<Card>
					{#snippet body()}
						<div class="llm-loading">
							<StatusIndicator status="running" />
							<span>{$i18n('models_loading')}</span>
						</div>
					{/snippet}
				</Card>
			{:else if filteredModels.length === 0}
				<Card>
					{#snippet body()}
						<div class="models-empty">
							<Cpu size={48} class="empty-icon" />
							<h3 class="empty-title">{$i18n('models_not_found')}</h3>
							<p class="empty-description">
								{#if selectedModelsProvider === 'all'}
									{$i18n('models_not_configured_all')}
								{:else if selectedModelsProvider === 'mistral'}
									{$i18n('models_not_configured_mistral')}
								{:else}
									{$i18n('models_not_configured_ollama')}
								{/if}
								{$i18n('models_add_custom')}
							</p>
							<Button variant="primary" onclick={openCreateModelModal}>
								<Plus size={16} />
								<span>{$i18n('models_add_first')}</span>
							</Button>
						</div>
					{/snippet}
				</Card>
			{:else}
				<div class="models-grid">
					{#each filteredModels as model (model.id)}
						<ModelCard
							{model}
							isDefault={llmState.providers[model.provider]?.default_model_id === model.id}
							onEdit={() => openEditModelModal(model)}
							onDelete={() => handleDeleteModel(model)}
							onSetDefault={() => handleSetDefaultModel(model)}
						/>
					{/each}
				</div>
			{/if}
		</section>

		<!-- Agents Section -->
		<section id="agents" class="settings-section">
			<AgentSettings refreshTrigger={agentRefreshKey} />
		</section>

		<!-- MCP Servers Section -->
		<section id="mcp" class="settings-section">
			<div class="section-header-row">
				<div class="section-title-row">
					<h2 class="section-title">{$i18n('settings_mcp_servers')}</h2>
					<HelpButton
						titleKey="help_mcp_title"
						descriptionKey="help_mcp_description"
						tutorialKey="help_mcp_tutorial"
					/>
				</div>
				<Button variant="primary" size="sm" onclick={openCreateModal}>
					<Plus size={16} />
					<span>{$i18n('mcp_add_server')}</span>
				</Button>
			</div>

			{#if mcpState.error}
				<div class="mcp-error">
					{mcpState.error}
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
							<Button variant="primary" onclick={openCreateModal}>
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
							onDelete={() => handleDeleteServer(server)}
						/>
					{/each}
				</div>
			{/if}
		</section>

		<!-- Memory Section -->
		<section id="memory" class="settings-section">
			<div class="section-title-row">
				<h2 class="section-title">{$i18n('settings_memory')}</h2>
				<HelpButton
					titleKey="help_memory_title"
					descriptionKey="help_memory_description"
					tutorialKey="help_memory_tutorial"
				/>
			</div>

			<div class="memory-subsections">
				<!-- Embedding Configuration -->
				<div class="memory-subsection">
					<h3 class="subsection-title">{$i18n('memory_embedding_config')}</h3>
					<MemorySettings bind:this={memorySettingsRef} />
				</div>

				<!-- Memory Management -->
				<div class="memory-subsection">
					<h3 class="subsection-title">{$i18n('memory_management')}</h3>
					<MemoryList onchange={() => memorySettingsRef?.refreshStats()} />
				</div>
			</div>
		</section>

		<!-- Validation Section -->
		<section id="validation" class="settings-section">
			<div class="section-title-row">
				<h2 class="section-title">{$i18n('settings_validation')}</h2>
				<HelpButton
					titleKey="help_validation_title"
					descriptionKey="help_validation_description"
					tutorialKey="help_validation_tutorial"
				/>
			</div>
			<p class="section-description">
				{$i18n('validation_description')}
			</p>
			<ValidationSettings />
		</section>

		<!-- Prompts Section -->
		<section id="prompts" class="settings-section">
			<PromptSettings />
		</section>

		<!-- Import/Export Section -->
		<section id="import-export" class="settings-section">
			<ImportExportSettings onRefreshNeeded={handleImportRefresh} />
		</section>

		<!-- Theme Section -->
		<section id="theme" class="settings-section">
			<div class="section-title-row">
				<h2 class="section-title">{$i18n('settings_theme')}</h2>
				<HelpButton
					titleKey="help_theme_title"
					descriptionKey="help_theme_description"
					tutorialKey="help_theme_tutorial"
				/>
			</div>

			<div class="theme-grid">
				<!-- Light Theme Card -->
				<button
					type="button"
					class="theme-card"
					class:selected={currentTheme === 'light'}
					onclick={() => handleThemeChange('light')}
				>
					<div class="theme-preview light">
						<div class="theme-header">
							<Sun size={24} />
							<div>
								<h3 class="theme-title">{$i18n('theme_light')}</h3>
								<p class="theme-description">{$i18n('theme_light_description')}</p>
							</div>
						</div>
						<div class="theme-colors">
							<div class="color-swatch accent"></div>
							<div class="color-swatch secondary"></div>
							<div class="color-swatch bg-light"></div>
						</div>
					</div>
				</button>

				<!-- Dark Theme Card -->
				<button
					type="button"
					class="theme-card"
					class:selected={currentTheme === 'dark'}
					onclick={() => handleThemeChange('dark')}
				>
					<div class="theme-preview dark">
						<div class="theme-header">
							<Moon size={24} />
							<div>
								<h3 class="theme-title">{$i18n('theme_dark')}</h3>
								<p class="theme-description">{$i18n('theme_dark_description')}</p>
							</div>
						</div>
						<div class="theme-colors">
							<div class="color-swatch accent"></div>
							<div class="color-swatch secondary"></div>
							<div class="color-swatch bg-dark"></div>
						</div>
					</div>
				</button>
			</div>
		</section>

		<!-- Security Info -->
		<section class="settings-section">
			<Card>
				{#snippet header()}
					<div class="security-header">
						<ShieldCheck size={24} class="icon-success" />
						<h3 class="card-title">{$i18n('security_title')}</h3>
					</div>
				{/snippet}
				{#snippet body()}
					<p class="security-info-text">
						{$i18n('security_description')}
					</p>
				{/snippet}
			</Card>
		</section>
	</main>
</div>

<!-- MCP Server Modal (Create/Edit) -->
<Modal
	open={showMCPModal}
	title={mcpModalMode === 'create' ? $i18n('mcp_modal_add') : $i18n('mcp_modal_edit')}
	onclose={closeMCPModal}
>
	{#snippet body()}
		<MCPServerForm
			mode={mcpModalMode}
			server={editingServer}
			onsave={handleSaveMCPServer}
			oncancel={closeMCPModal}
			saving={mcpSaving}
		/>
	{/snippet}
</Modal>

<!-- MCP Server Test Modal -->
<Modal
	open={showTestModal}
	title={`Test: ${testingServerConfig?.name ?? 'Server'}`}
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

<!-- Model Modal (Create/Edit) -->
<Modal
	open={showModelModal}
	title={modelModalMode === 'create' ? $i18n('modal_add_custom_model') : $i18n('modal_edit_model')}
	onclose={closeModelModal}
>
	{#snippet body()}
		<ModelForm
			mode={modelModalMode}
			model={editingModel}
			provider={selectedModelsProvider === 'all' ? 'mistral' : selectedModelsProvider}
			onsubmit={handleSaveModel}
			oncancel={closeModelModal}
			saving={modelSaving}
		/>
	{/snippet}
</Modal>

<!-- API Key Modal -->
<Modal
	open={showApiKeyModal}
	title={apiKeyProvider === 'mistral' ? $i18n('api_key_modal_mistral') : $i18n('api_key_modal_ollama')}
	onclose={closeApiKeyModal}
>
	{#snippet body()}
		<div class="api-key-modal-content">
			{#if apiKeyProvider === 'ollama'}
				<p class="api-key-info">
					{$i18n('api_key_ollama_info')}
				</p>
				<Input
					type="url"
					label={$i18n('api_key_server_url')}
					value={llmState.providers.ollama?.base_url ?? 'http://localhost:11434'}
					help={$i18n('api_key_server_url_help')}
					disabled
				/>
				<div class="status-row">
					<StatusIndicator status="completed" size="sm" />
					<span class="status-text">{$i18n('api_key_not_required')}</span>
				</div>
			{:else}
				<p class="api-key-info">
					{$i18n('api_key_mistral_info')}
				</p>
				<Input
					type="password"
					label={$i18n('api_key_label')}
					placeholder={$i18n('api_key_placeholder')}
					bind:value={settings.apiKey}
					disabled={saving}
					help={$i18n('api_key_help')}
				/>
				{#if providerHasApiKey('mistral')}
					<div class="status-row">
						<StatusIndicator status="completed" size="sm" />
						<span class="status-text">{$i18n('api_key_configured')}</span>
					</div>
				{/if}
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="api-key-modal-actions">
			<Button variant="ghost" onclick={closeApiKeyModal} disabled={saving}>
				{$i18n('common_cancel')}
			</Button>
			{#if apiKeyProvider === 'mistral'}
				{#if providerHasApiKey('mistral')}
					<Button
						variant="danger"
						onclick={() => handleDeleteApiKey('mistral')}
						disabled={saving}
					>
						{$i18n('api_key_delete')}
					</Button>
				{/if}
				<Button
					variant="primary"
					onclick={handleSaveApiKey}
					disabled={saving || !settings.apiKey.trim()}
				>
					{saving ? $i18n('common_saving') : $i18n('api_key_save')}
				</Button>
			{:else}
				<Button variant="primary" onclick={closeApiKeyModal}>
					{$i18n('common_done')}
				</Button>
			{/if}
		</div>
	{/snippet}
</Modal>

<style>
	.settings-page {
		display: flex;
		height: 100%;
		flex: 1;
		min-width: 0;
	}

	/* Sidebar */
	.sidebar-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.sidebar-icon-collapsed {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--color-accent);
		padding: var(--spacing-xs);
	}

	.nav-items {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.nav-items-collapsed {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.nav-button {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: background-color var(--transition-fast), color var(--transition-fast);
		width: 100%;
		text-align: left;
	}

	.nav-button:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.nav-button.active {
		background: var(--color-accent-light);
		color: var(--color-accent);
		font-weight: var(--font-weight-medium);
	}

	.nav-button-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-sm);
		color: var(--color-text-secondary);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: background-color var(--transition-fast), color var(--transition-fast);
	}

	.nav-button-icon:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.nav-button-icon.active {
		background: var(--color-accent-light);
		color: var(--color-accent);
	}

	.security-badge {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm);
		background: var(--color-success-light);
		border-radius: var(--border-radius-md);
		color: var(--color-success);
	}

	.security-badge-collapsed {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-sm);
		background: var(--color-success-light);
		border-radius: var(--border-radius-md);
		color: var(--color-success);
	}

	.security-text {
		font-size: var(--font-size-xs);
	}

	/* Content Area */
	.content-area {
		flex: 1;
		min-height: 0;
		min-width: 0;
		overflow-y: auto;
		padding: var(--spacing-xl);
		/* NOTE: Removed contain: content - it breaks position:fixed modals inside */
		-webkit-overflow-scrolling: touch;
	}

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

	.section-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-lg);
		line-height: var(--line-height-relaxed);
	}

	.section-header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--spacing-lg);
	}

	.section-header-row .section-title {
		margin-bottom: 0;
	}

	.section-header-row :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	/* Memory Section */
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

	/* Provider Cards */
	.provider-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
		margin-bottom: var(--spacing-lg);
	}

	.status-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		background: var(--color-success-light);
		border-radius: var(--border-radius-md);
	}

	.status-text {
		font-size: var(--font-size-sm);
		color: var(--color-success);
	}

	.message-toast {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		margin-top: var(--spacing-md);
	}

	.message-toast.success {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.message-toast.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}

	/* MCP Servers */
	.mcp-server-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
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

	.mcp-error {
		padding: var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
		margin-bottom: var(--spacing-lg);
	}

	/* LLM Section */
	.llm-error {
		padding: var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
		margin-bottom: var(--spacing-lg);
	}

	.llm-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	/* Models Section */
	.models-header-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.models-header-actions :global(.form-group) {
		margin-bottom: 0;
	}

	.models-header-actions :global(.form-select) {
		width: auto;
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-xs);
	}

	.models-header-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.models-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
	}

	.models-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: var(--spacing-2xl);
		gap: var(--spacing-md);
	}

	.models-empty :global(.empty-icon) {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.models-empty :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	/* API Key Modal */
	.api-key-modal-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.api-key-info {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		line-height: var(--line-height-relaxed);
		margin: 0;
	}

	.api-key-modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}

	/* Theme Cards */
	.theme-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
	}

	.theme-card {
		cursor: pointer;
		background: none;
		border: none;
		padding: 0;
		width: 100%;
		text-align: left;
	}

	.theme-preview {
		background: var(--color-bg-primary);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		overflow: hidden;
		transition: border-color var(--transition-fast);
	}

	.theme-card.selected .theme-preview {
		border-color: var(--color-accent);
	}

	.theme-preview .theme-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-lg);
	}

	.theme-preview.light .theme-header {
		background: #ffffff;
		color: #212529;
	}

	.theme-preview.dark .theme-header {
		background: #2b2d31;
		color: #ffffff;
	}

	.theme-preview.dark .theme-description {
		color: #b5bac1;
	}

	.theme-title {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
	}

	.theme-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.theme-colors {
		display: flex;
		gap: var(--spacing-sm);
		padding: var(--spacing-lg);
		background: var(--color-bg-secondary);
	}

	.color-swatch {
		width: 40px;
		height: 40px;
		border-radius: var(--border-radius-md);
	}

	.color-swatch.accent {
		background: #94EFEE;
	}

	.color-swatch.secondary {
		background: #FE7254;
	}

	.color-swatch.bg-light {
		background: #ffffff;
		border: 1px solid #dee2e6;
	}

	.color-swatch.bg-dark {
		background: #2b2d31;
	}

	/* Security Section */
	.security-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.security-header :global(.icon-success) {
		color: var(--color-success);
	}

	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.security-info-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		line-height: var(--line-height-relaxed);
	}

	/* Responsive */
	@media (max-width: 768px) {
		.provider-grid,
		.theme-grid,
		.mcp-server-grid,
		.models-grid {
			grid-template-columns: 1fr;
		}

		.models-header-actions {
			flex-direction: column;
			align-items: stretch;
		}
	}
</style>
