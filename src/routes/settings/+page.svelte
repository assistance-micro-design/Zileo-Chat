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
	import { Card, Button, Input, Select, StatusIndicator, Modal } from '$lib/components/ui';
	import { MCPServerCard, MCPServerForm, MCPServerTester } from '$lib/components/mcp';
	import { ProviderCard, ModelCard, ModelForm } from '$lib/components/llm';
	import { MemorySettings, MemoryList } from '$lib/components/settings/memory';
	import { AgentSettings } from '$lib/components/settings/agents';
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
		Settings
	} from 'lucide-svelte';

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

	/** Navigation sections */
	const sections = [
		{ id: 'providers', label: 'Providers', icon: Globe },
		{ id: 'models', label: 'Models', icon: Cpu },
		{ id: 'agents', label: 'Agents', icon: Bot },
		{ id: 'mcp', label: 'MCP Servers', icon: Plug },
		{ id: 'memory', label: 'Memory', icon: Brain },
		{ id: 'theme', label: 'Theme', icon: Palette }
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
			sections.forEach((section) => {
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
				<div class="sidebar-icon-collapsed" title="Settings">
					<Settings size={24} />
				</div>
			{:else}
				<h2 class="sidebar-title">Settings</h2>
			{/if}
		{/snippet}

		{#snippet nav()}
			{#if !sidebarCollapsed}
				<div class="nav-items">
					{#each sections as section}
						{@const Icon = section.icon}
						<button
							type="button"
							class="nav-button"
							class:active={activeSection === section.id}
							onclick={() => scrollToSection(section.id)}
						>
							<Icon size={20} />
							<span class="nav-text">{section.label}</span>
						</button>
					{/each}
				</div>
			{:else}
				<div class="nav-items-collapsed">
					{#each sections as section}
						{@const Icon = section.icon}
						<button
							type="button"
							class="nav-button-icon"
							class:active={activeSection === section.id}
							onclick={() => scrollToSection(section.id)}
							title={section.label}
						>
							<Icon size={20} />
						</button>
					{/each}
				</div>
			{/if}
		{/snippet}

		{#snippet footer()}
			{#if sidebarCollapsed}
				<div class="security-badge-collapsed" title="AES-256 Encrypted">
					<ShieldCheck size={20} />
				</div>
			{:else}
				<div class="security-badge">
					<ShieldCheck size={16} />
					<span class="security-text">AES-256 Encrypted</span>
				</div>
			{/if}
		{/snippet}
	</Sidebar>

	<!-- Settings Content -->
	<main class="content-area" bind:this={contentAreaRef}>
		<!-- Providers Section -->
		<section id="providers" class="settings-section">
			<h2 class="section-title">Providers</h2>

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
							<span>Loading providers...</span>
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
				<h2 class="section-title">Models</h2>
				<div class="models-header-actions">
					<Select
						options={modelsProviderOptions}
						value={selectedModelsProvider}
						onchange={handleModelsProviderChange}
					/>
					<Button variant="primary" size="sm" onclick={openCreateModelModal}>
						<Plus size={16} />
						<span>Add Model</span>
					</Button>
				</div>
			</div>

			{#if llmState.loading}
				<Card>
					{#snippet body()}
						<div class="llm-loading">
							<StatusIndicator status="running" />
							<span>Loading models...</span>
						</div>
					{/snippet}
				</Card>
			{:else if filteredModels.length === 0}
				<Card>
					{#snippet body()}
						<div class="models-empty">
							<Cpu size={48} class="empty-icon" />
							<h3 class="empty-title">No Models Found</h3>
							<p class="empty-description">
								{#if selectedModelsProvider === 'all'}
									No models configured yet.
								{:else}
									No models configured for {selectedModelsProvider === 'mistral' ? 'Mistral' : 'Ollama'}.
								{/if}
								Add a custom model to get started.
							</p>
							<Button variant="primary" onclick={openCreateModelModal}>
								<Plus size={16} />
								<span>Add Your First Model</span>
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
			<AgentSettings />
		</section>

		<!-- MCP Servers Section -->
		<section id="mcp" class="settings-section">
			<div class="section-header-row">
				<h2 class="section-title">MCP Servers</h2>
				<Button variant="primary" size="sm" onclick={openCreateModal}>
					<Plus size={16} />
					<span>Add Server</span>
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
							<span>Loading MCP servers...</span>
						</div>
					{/snippet}
				</Card>
			{:else if mcpState.servers.length === 0}
				<Card>
					{#snippet body()}
						<div class="mcp-empty">
							<Plug size={48} class="empty-icon" />
							<h3 class="empty-title">No MCP Servers Configured</h3>
							<p class="empty-description">
								MCP servers provide external tools and resources for your agents.
								Add a server to get started.
							</p>
							<Button variant="primary" onclick={openCreateModal}>
								<Plus size={16} />
								<span>Add Your First Server</span>
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
			<h2 class="section-title">Memory</h2>

			<div class="memory-subsections">
				<!-- Embedding Configuration -->
				<div class="memory-subsection">
					<h3 class="subsection-title">Embedding Configuration</h3>
					<MemorySettings />
				</div>

				<!-- Memory Management -->
				<div class="memory-subsection">
					<h3 class="subsection-title">Memory Management</h3>
					<MemoryList />
				</div>
			</div>
		</section>

		<!-- Theme Section -->
		<section id="theme" class="settings-section">
			<h2 class="section-title">Theme</h2>

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
								<h3 class="theme-title">Light Mode</h3>
								<p class="theme-description">Bright and clean interface</p>
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
								<h3 class="theme-title">Dark Mode</h3>
								<p class="theme-description">Easy on the eyes</p>
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
						<h3 class="card-title">Security Information</h3>
					</div>
				{/snippet}
				{#snippet body()}
					<p class="security-info-text">
						API keys are stored securely using your operating system's keychain
						(Linux: libsecret, macOS: Keychain, Windows: Credential Manager) with
						additional AES-256 encryption for defense in depth.
					</p>
				{/snippet}
			</Card>
		</section>
	</main>
</div>

<!-- MCP Server Modal (Create/Edit) -->
<Modal
	open={showMCPModal}
	title={mcpModalMode === 'create' ? 'Add MCP Server' : 'Edit MCP Server'}
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
			Close
		</Button>
	{/snippet}
</Modal>

<!-- Model Modal (Create/Edit) -->
<Modal
	open={showModelModal}
	title={modelModalMode === 'create' ? 'Add Custom Model' : 'Edit Model'}
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
	title={`Configure ${apiKeyProvider === 'mistral' ? 'Mistral' : 'Ollama'}`}
	onclose={closeApiKeyModal}
>
	{#snippet body()}
		<div class="api-key-modal-content">
			{#if apiKeyProvider === 'ollama'}
				<p class="api-key-info">
					Ollama runs locally and does not require an API key. You can configure the server URL below.
				</p>
				<Input
					type="url"
					label="Server URL"
					value={llmState.providers.ollama?.base_url ?? 'http://localhost:11434'}
					help="The URL of your local Ollama server"
					disabled
				/>
				<div class="status-row">
					<StatusIndicator status="completed" size="sm" />
					<span class="status-text">No API key required</span>
				</div>
			{:else}
				<p class="api-key-info">
					Enter your Mistral API key. It will be stored securely using your operating system's keychain.
				</p>
				<Input
					type="password"
					label="API Key"
					placeholder="sk-..."
					bind:value={settings.apiKey}
					disabled={saving}
					help="Your Mistral API key"
				/>
				{#if providerHasApiKey('mistral')}
					<div class="status-row">
						<StatusIndicator status="completed" size="sm" />
						<span class="status-text">API key already configured</span>
					</div>
				{/if}
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="api-key-modal-actions">
			<Button variant="ghost" onclick={closeApiKeyModal} disabled={saving}>
				Cancel
			</Button>
			{#if apiKeyProvider === 'mistral'}
				{#if providerHasApiKey('mistral')}
					<Button
						variant="danger"
						onclick={() => handleDeleteApiKey('mistral')}
						disabled={saving}
					>
						Delete Key
					</Button>
				{/if}
				<Button
					variant="primary"
					onclick={handleSaveApiKey}
					disabled={saving || !settings.apiKey.trim()}
				>
					{saving ? 'Saving...' : 'Save API Key'}
				</Button>
			{:else}
				<Button variant="primary" onclick={closeApiKeyModal}>
					Done
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
		/* Performance optimizations */
		contain: content;
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
