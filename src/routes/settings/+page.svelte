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

Settings Page - Refactored with extracted section components (OPT-6)
Uses: MCPSection, LLMSection, APIKeysSection and other section components.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import type { ProviderType, ProviderSettings } from '$types/llm';
	import { Sidebar } from '$lib/components/layout';
	import { Card, HelpButton, StatusIndicator } from '$lib/components/ui';

	// Static imports for lightweight components
	import { ValidationSettings } from '$lib/components/settings/validation';
	import { PromptSettings } from '$lib/components/settings/prompts';
	import { ImportExportSettings } from '$lib/components/settings/import-export';

	// Extracted section components (OPT-6)
	import MCPSection from '$lib/components/settings/MCPSection.svelte';
	import LLMSection from '$lib/components/settings/LLMSection.svelte';
	import APIKeysSection from '$lib/components/settings/APIKeysSection.svelte';

	import { theme, type Theme } from '$lib/stores/theme';
	import { agentStore } from '$lib/stores/agents';
	import { promptStore } from '$lib/stores/prompts';

	// ============================================================================
	// OPT-8: Lazy Loading for Heavy Components
	// ============================================================================

	/** Lazy loaded component types */
	type LazyMemorySettings = typeof import('$lib/components/settings/memory/MemorySettings.svelte').default;
	type LazyMemoryList = typeof import('$lib/components/settings/memory/MemoryList.svelte').default;
	type LazyAgentSettings = typeof import('$lib/components/settings/agents/AgentSettings.svelte').default;

	/** Lazy loaded component references */
	let MemorySettingsComponent = $state<LazyMemorySettings | null>(null);
	let MemoryListComponent = $state<LazyMemoryList | null>(null);
	let AgentSettingsComponent = $state<LazyAgentSettings | null>(null);

	import {
		Globe,
		Cpu,
		Palette,
		Plug,
		Sun,
		Moon,
		ShieldCheck,
		Brain,
		Bot,
		Settings,
		BookOpen,
		FolderSync
	} from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/** UI state */
	let activeSection = $state('providers');
	let sidebarCollapsed = $state(false);

	/** Component references for reload */
	let mcpSectionRef: MCPSection;
	let llmSectionRef: LLMSection;

	/** Reference for MemorySettings to refresh stats */
	let memorySettingsRef = $state<{ refreshStats: () => Promise<void> } | undefined>(undefined);

	/** API Key Modal state */
	let showApiKeyModal = $state(false);
	let apiKeyProvider = $state<ProviderType>('mistral');
	let apiKeyProviderSettings = $state<ProviderSettings | null>(null);
	let apiKeyHasKey = $state(false);

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

	/**
	 * Opens API key configuration modal
	 */
	function handleConfigureApiKey(provider: ProviderType): void {
		apiKeyProvider = provider;
		showApiKeyModal = true;
	}

	/**
	 * Reloads LLM data after API key changes
	 */
	function handleApiKeyReload(): void {
		llmSectionRef?.reload();
	}

	/**
	 * Refreshes all data stores after an import operation.
	 * Called when ImportExportSettings signals that new data was imported.
	 */
	async function handleImportRefresh(): Promise<void> {
		// Reload all data stores in parallel
		await Promise.all([
			mcpSectionRef?.reload(),
			llmSectionRef?.reload(),
			agentStore.loadAgents(),
			promptStore.loadPrompts()
		]);
		// Increment refresh key to trigger AgentSettings $effect
		agentRefreshKey++;
	}

	/**
	 * Reference to content area for IntersectionObserver
	 */
	let contentAreaRef: HTMLElement | null = $state(null);

	/**
	 * Initialize on mount:
	 * - Lazy load heavy components
	 * - Subscribe to theme store
	 * - Setup IntersectionObserver for section detection
	 */
	onMount(() => {
		// OPT-8: Lazy load heavy components in parallel
		Promise.all([
			import('$lib/components/settings/memory/MemorySettings.svelte'),
			import('$lib/components/settings/memory/MemoryList.svelte'),
			import('$lib/components/settings/agents/AgentSettings.svelte')
		])
			.then(([memorySettingsModule, memoryListModule, agentSettingsModule]) => {
				MemorySettingsComponent = memorySettingsModule.default;
				MemoryListComponent = memoryListModule.default;
				AgentSettingsComponent = agentSettingsModule.default;
			})
			.catch((err: unknown) => {
				console.warn('[Settings] Failed to lazy load components:', err);
			});

		// Subscribe to theme store and sync value
		const unsubscribeTheme = theme.subscribe((value) => {
			currentTheme = value;
		});

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
		<!-- Providers and Models Sections (LLMSection) -->
		<LLMSection
			bind:this={llmSectionRef}
			onConfigureApiKey={handleConfigureApiKey}
		/>

		<!-- Agents Section (Lazy Loaded - OPT-8) -->
		<section id="agents" class="settings-section">
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

		<!-- MCP Servers Section -->
		<MCPSection bind:this={mcpSectionRef} />

		<!-- Memory Section (Lazy Loaded - OPT-8) -->
		<section id="memory" class="settings-section">
			<div class="section-title-row">
				<h2 class="section-title">{$i18n('settings_memory')}</h2>
				<HelpButton
					titleKey="help_memory_title"
					descriptionKey="help_memory_description"
					tutorialKey="help_memory_tutorial"
				/>
			</div>

			{#if MemorySettingsComponent && MemoryListComponent}
				<div class="memory-subsections">
					<!-- Embedding Configuration -->
					<div class="memory-subsection">
						<h3 class="subsection-title">{$i18n('memory_embedding_config')}</h3>
						<MemorySettingsComponent bind:this={memorySettingsRef} />
					</div>

					<!-- Memory Management -->
					<div class="memory-subsection">
						<h3 class="subsection-title">{$i18n('memory_management')}</h3>
						<MemoryListComponent onchange={() => memorySettingsRef?.refreshStats()} />
					</div>
				</div>
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

<!-- API Key Modal -->
<APIKeysSection
	open={showApiKeyModal}
	provider={apiKeyProvider}
	providerSettings={apiKeyProviderSettings}
	hasApiKey={apiKeyHasKey}
	onclose={() => { showApiKeyModal = false; }}
	onReload={handleApiKeyReload}
/>

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

	/* OPT-8: Lazy Loading */
	.lazy-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	/* Responsive */
	@media (max-width: 768px) {
		.theme-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
