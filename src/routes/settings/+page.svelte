<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

Settings Page - Refactored with Design System Components
Uses: Sidebar, NavItem, Card, Button, Input, Select, Badge, StatusIndicator
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import type { LLMProvider } from '$types/security';
	import { Sidebar } from '$lib/components/layout';
	import { Card, Button, Input, Select, Badge, StatusIndicator } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { theme, type Theme } from '$lib/stores/theme';
	import { Globe, Cpu, Palette, Sparkles, Server, Sun, Moon, ShieldCheck } from 'lucide-svelte';

	/** Settings state */
	let settings = $state({
		provider: 'Mistral' as LLMProvider,
		model: 'mistral-large',
		apiKey: ''
	});

	/** UI state */
	let saving = $state(false);
	let hasStoredKey = $state(false);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
	let activeSection = $state('providers');
	let sidebarCollapsed = $state(false);

	/** Provider options */
	const providerOptions: SelectOption[] = [
		{ value: 'Mistral', label: 'Mistral' },
		{ value: 'Ollama', label: 'Ollama' },
		{ value: 'OpenAI', label: 'OpenAI' },
		{ value: 'Anthropic', label: 'Anthropic' }
	];

	/** Navigation sections */
	const sections = [
		{ id: 'providers', label: 'Providers', icon: Globe },
		{ id: 'models', label: 'Models', icon: Cpu },
		{ id: 'theme', label: 'Theme', icon: Palette }
	] as const;

	/**
	 * Get current theme value
	 */
	let currentTheme = $state<Theme>('light');
	theme.subscribe((value) => {
		currentTheme = value;
	});

	/**
	 * Checks if the current provider has a stored API key
	 */
	async function checkApiKeyStatus(): Promise<void> {
		try {
			hasStoredKey = await invoke<boolean>('has_api_key', {
				provider: settings.provider
			});
		} catch {
			hasStoredKey = false;
		}
	}

	/**
	 * Saves the API key securely using OS keychain + AES-256 encryption
	 */
	async function saveApiKey(): Promise<void> {
		if (!settings.apiKey.trim()) {
			message = { type: 'error', text: 'API key cannot be empty' };
			return;
		}

		saving = true;
		message = null;

		try {
			await invoke('save_api_key', {
				provider: settings.provider,
				apiKey: settings.apiKey
			});
			settings.apiKey = '';
			hasStoredKey = true;
			message = { type: 'success', text: 'API key saved securely' };
		} catch (err) {
			message = { type: 'error', text: `Failed to save: ${err}` };
		} finally {
			saving = false;
		}
	}

	/**
	 * Deletes the stored API key for the current provider
	 */
	async function deleteApiKey(): Promise<void> {
		saving = true;
		message = null;

		try {
			await invoke('delete_api_key', {
				provider: settings.provider
			});
			hasStoredKey = false;
			message = { type: 'success', text: 'API key deleted' };
		} catch (err) {
			message = { type: 'error', text: `Failed to delete: ${err}` };
		} finally {
			saving = false;
		}
	}

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
	 * Handle provider change
	 */
	function handleProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		settings.provider = event.currentTarget.value as LLMProvider;
	}

	/**
	 * Handle theme change
	 */
	function handleThemeChange(newTheme: Theme): void {
		theme.setTheme(newTheme);
	}

	/**
	 * Effect to check API key status when provider changes
	 */
	$effect(() => {
		checkApiKeyStatus();
	});
</script>

<div class="settings-page">
	<!-- Settings Sidebar -->
	<Sidebar bind:collapsed={sidebarCollapsed}>
		{#snippet header()}
			<h2 class="sidebar-title">Settings</h2>
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
			{#if !sidebarCollapsed}
				<div class="security-badge">
					<ShieldCheck size={16} />
					<span class="security-text">AES-256 Encrypted</span>
				</div>
			{/if}
		{/snippet}
	</Sidebar>

	<!-- Settings Content -->
	<main class="content-area">
		<!-- Providers Section -->
		<section id="providers" class="settings-section">
			<h2 class="section-title">Providers</h2>

			<div class="provider-grid">
				<!-- Mistral Provider Card -->
				<Card>
					{#snippet header()}
						<div class="card-header-content">
							<div class="provider-info">
								<Sparkles size={24} class="icon-accent" />
								<div>
									<h3 class="provider-name">Mistral</h3>
									<p class="provider-type">API Provider</p>
								</div>
							</div>
							<Badge variant={settings.provider === 'Mistral' ? 'success' : 'primary'}>
								{settings.provider === 'Mistral' ? 'Selected' : 'Available'}
							</Badge>
						</div>
					{/snippet}
					{#snippet body()}
						<div class="provider-body">
							<Input
								type="password"
								label="API Key"
								placeholder={hasStoredKey && settings.provider === 'Mistral' ? '(key stored securely)' : 'sk-...'}
								bind:value={settings.apiKey}
								disabled={saving || settings.provider !== 'Mistral'}
								help="Your Mistral API key"
							/>
							{#if settings.provider === 'Mistral' && hasStoredKey}
								<div class="status-row">
									<StatusIndicator status="completed" size="sm" />
									<span class="status-text">Key stored securely</span>
								</div>
							{/if}
						</div>
					{/snippet}
					{#snippet footer()}
						<Button
							variant={settings.provider === 'Mistral' ? 'ghost' : 'primary'}
							size="sm"
							onclick={() => { settings.provider = 'Mistral'; }}
							disabled={settings.provider === 'Mistral'}
						>
							{settings.provider === 'Mistral' ? 'Selected' : 'Select'}
						</Button>
					{/snippet}
				</Card>

				<!-- Ollama Provider Card -->
				<Card>
					{#snippet header()}
						<div class="card-header-content">
							<div class="provider-info">
								<Server size={24} class="icon-success" />
								<div>
									<h3 class="provider-name">Ollama</h3>
									<p class="provider-type">Local Provider</p>
								</div>
							</div>
							<Badge variant={settings.provider === 'Ollama' ? 'success' : 'primary'}>
								{settings.provider === 'Ollama' ? 'Selected' : 'Available'}
							</Badge>
						</div>
					{/snippet}
					{#snippet body()}
						<div class="provider-body">
							<Input
								type="url"
								label="Endpoint URL"
								value="http://localhost:11434"
								disabled
							/>
							<div class="status-row">
								<StatusIndicator status="completed" size="sm" />
								<span class="status-text">No API key required</span>
							</div>
						</div>
					{/snippet}
					{#snippet footer()}
						<Button
							variant={settings.provider === 'Ollama' ? 'ghost' : 'primary'}
							size="sm"
							onclick={() => { settings.provider = 'Ollama'; }}
							disabled={settings.provider === 'Ollama'}
						>
							{settings.provider === 'Ollama' ? 'Selected' : 'Select'}
						</Button>
					{/snippet}
				</Card>
			</div>

			<!-- API Key Actions -->
			{#if settings.provider !== 'Ollama'}
				<Card>
					{#snippet header()}
						<h3 class="card-title">API Key Management</h3>
					{/snippet}
					{#snippet body()}
						<div class="api-key-actions">
							<Button
								variant="primary"
								onclick={saveApiKey}
								disabled={saving || !settings.apiKey.trim()}
							>
								{saving ? 'Saving...' : 'Save API Key'}
							</Button>
							{#if hasStoredKey}
								<Button
									variant="danger"
									onclick={deleteApiKey}
									disabled={saving}
								>
									Delete Stored Key
								</Button>
							{/if}
						</div>
						{#if message}
							<div class="message-toast" class:success={message.type === 'success'} class:error={message.type === 'error'}>
								{message.text}
							</div>
						{/if}
					{/snippet}
				</Card>
			{/if}
		</section>

		<!-- Models Section -->
		<section id="models" class="settings-section">
			<h2 class="section-title">Models</h2>

			<Card>
				{#snippet header()}
					<h3 class="card-title">Model Configuration</h3>
				{/snippet}
				{#snippet body()}
					<div class="model-form">
						<Select
							label="Provider"
							options={providerOptions}
							value={settings.provider}
							onchange={handleProviderChange}
						/>
						<Input
							label="Model"
							value={settings.model}
							oninput={(e) => settings.model = e.currentTarget.value}
							help="Model identifier (e.g., mistral-large, llama3)"
						/>
						<div class="model-info">
							<h4 class="info-title">Selected Model</h4>
							<div class="info-grid">
								<div class="info-item">
									<span class="info-label">Provider</span>
									<span class="info-value">{settings.provider}</span>
								</div>
								<div class="info-item">
									<span class="info-label">Model</span>
									<span class="info-value">{settings.model}</span>
								</div>
							</div>
						</div>
					</div>
				{/snippet}
			</Card>
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

<style>
	.settings-page {
		display: flex;
		height: 100%;
	}

	/* Sidebar */
	.sidebar-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
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
		transition: all var(--transition-fast);
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
		transition: all var(--transition-fast);
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

	.security-text {
		font-size: var(--font-size-xs);
	}

	/* Content Area */
	.content-area {
		flex: 1;
		overflow-y: auto;
		padding: var(--spacing-xl);
	}

	.settings-section {
		margin-bottom: var(--spacing-2xl);
	}

	.section-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-lg);
	}

	/* Provider Cards */
	.provider-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
		margin-bottom: var(--spacing-lg);
	}

	.card-header-content {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}

	.provider-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.provider-info :global(.icon-accent) {
		color: var(--color-accent);
	}

	.provider-info :global(.icon-success) {
		color: var(--color-success);
	}

	.provider-name {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
	}

	.provider-type {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.provider-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
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

	/* API Key Actions */
	.api-key-actions {
		display: flex;
		gap: var(--spacing-md);
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

	/* Model Form */
	.model-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.model-info {
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.info-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-sm);
	}

	.info-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
	}

	.info-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.info-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.info-value {
		font-weight: var(--font-weight-semibold);
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
		.info-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
