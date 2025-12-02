<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

ExportPanel - Main export wizard.
Multi-step process: entity selection, options, preview, and export.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { save } from '@tauri-apps/plugin-dialog';
	import { Button, Card, Badge, StatusIndicator } from '$lib/components/ui';
	import EntitySelector from './EntitySelector.svelte';
	import ExportPreview from './ExportPreview.svelte';
	import type {
		ExportSelection,
		ExportOptions,
		ExportPreviewData,
		MCPSanitizationConfig,
		AgentSummary
	} from '$types';
	import type { MCPServerConfig } from '$types/mcp';
	import type { LLMModel } from '$types/llm';
	import type { PromptSummary } from '$types/prompt';

	/** Props */
	interface Props {
		/** Callback when export completes */
		onexport?: (success: boolean) => void;
	}

	let { onexport }: Props = $props();

	/** Wizard step */
	let currentStep = $state<'selection' | 'options' | 'preview'>('selection');

	/** Available entities */
	let agents = $state<AgentSummary[]>([]);
	let mcpServers = $state<MCPServerConfig[]>([]);
	let models = $state<LLMModel[]>([]);
	let prompts = $state<PromptSummary[]>([]);

	/** Selection state */
	let selectedAgents = $state<string[]>([]);
	let selectedMcpServers = $state<string[]>([]);
	let selectedModels = $state<string[]>([]);
	let selectedPrompts = $state<string[]>([]);

	/** Export options */
	let includeTimestamps = $state(false);
	let sanitizeMcp = $state(true);

	/** Preview state */
	let preview = $state<ExportPreviewData | null>(null);
	let mcpSanitization = $state<Record<string, MCPSanitizationConfig>>({});

	/** Loading and error state */
	let loading = $state(false);
	let exporting = $state(false);
	let error = $state<string | null>(null);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);

	/** Computed selection */
	const selection = $derived<ExportSelection>({
		agents: selectedAgents,
		mcpServers: selectedMcpServers,
		models: selectedModels,
		prompts: selectedPrompts
	});

	/** Check if any entities are selected */
	const hasSelection = $derived(
		selectedAgents.length > 0 ||
			selectedMcpServers.length > 0 ||
			selectedModels.length > 0 ||
			selectedPrompts.length > 0
	);

	/**
	 * Loads available entities from backend
	 */
	async function loadEntities(): Promise<void> {
		loading = true;
		error = null;
		try {
			[agents, mcpServers, models, prompts] = await Promise.all([
				invoke<AgentSummary[]>('list_agents'),
				invoke<MCPServerConfig[]>('list_mcp_servers'),
				invoke<LLMModel[]>('list_models'),
				invoke<PromptSummary[]>('list_prompts')
			]);
		} catch (err) {
			error = `Failed to load entities: ${err}`;
		} finally {
			loading = false;
		}
	}

	/**
	 * Prepares export preview
	 */
	async function preparePreview(): Promise<void> {
		loading = true;
		error = null;
		try {
			preview = await invoke<ExportPreviewData>('prepare_export_preview', { selection });

			// Initialize sanitization config for each MCP server
			// Use id if available (export preview), otherwise fallback to name
			const sanitizationConfig: Record<string, MCPSanitizationConfig> = {};
			for (const server of preview.mcpServers) {
				const serverId = server.id ?? server.name;
				sanitizationConfig[serverId] = {
					clearEnvKeys: [],
					modifyEnv: {},
					modifyArgs: [],
					excludeFromExport: false
				};
			}
			mcpSanitization = sanitizationConfig;

			currentStep = 'preview';
		} catch (err) {
			error = `Failed to prepare preview: ${err}`;
		} finally {
			loading = false;
		}
	}

	/**
	 * Generates and saves export file using native dialog
	 */
	async function generateExport(): Promise<void> {
		if (!preview) return;

		exporting = true;
		error = null;
		try {
			const options: ExportOptions = {
				format: 'json',
				includeTimestamps,
				sanitizeMcp
			};

			// Filter out excluded servers from selection and sanitization
			const filteredSelection: ExportSelection = {
				...selection,
				mcpServers: selection.mcpServers.filter(
					(id) => !mcpSanitization[id]?.excludeFromExport
				)
			};

			// Remove excluded servers from sanitization config
			const filteredSanitization: Record<string, MCPSanitizationConfig> = {};
			for (const [id, config] of Object.entries(mcpSanitization)) {
				if (!config.excludeFromExport) {
					filteredSanitization[id] = config;
				}
			}

			// Generate export file content
			const exportData = await invoke<string>('generate_export_file', {
				selection: filteredSelection,
				options,
				sanitization: sanitizeMcp ? filteredSanitization : {}
			});

			// Parse result to get metadata
			const exportPackage = JSON.parse(exportData);
			const defaultFilename = `zileo-export-${new Date().toISOString().slice(0, 10)}.json`;

			// Show native save dialog
			const filePath = await save({
				defaultPath: defaultFilename,
				filters: [
					{
						name: 'JSON',
						extensions: ['json']
					}
				],
				title: 'Save Export File'
			});

			// User cancelled
			if (!filePath) {
				exporting = false;
				return;
			}

			// Save file to selected path
			await invoke('save_export_to_file', {
				path: filePath,
				content: exportData
			});

			const totalCount =
				exportPackage.manifest.counts.agents +
				exportPackage.manifest.counts.mcpServers +
				exportPackage.manifest.counts.models +
				exportPackage.manifest.counts.prompts;

			// Extract filename from path for display
			const savedFilename = filePath.split('/').pop() || filePath.split('\\').pop() || filePath;

			message = {
				type: 'success',
				text: `Exported ${totalCount} items to ${savedFilename}`
			};
			onexport?.(true);

			// Reset state
			resetWizard();
		} catch (err) {
			error = `Export failed: ${err}`;
			message = { type: 'error', text: `Export failed: ${err}` };
			onexport?.(false);
		} finally {
			exporting = false;
		}
	}

	/**
	 * Resets the wizard to initial state
	 */
	function resetWizard(): void {
		currentStep = 'selection';
		selectedAgents = [];
		selectedMcpServers = [];
		selectedModels = [];
		selectedPrompts = [];
		includeTimestamps = false;
		sanitizeMcp = true;
		preview = null;
		mcpSanitization = {};
		error = null;
	}

	/**
	 * Navigates to the next step
	 */
	function nextStep(): void {
		if (currentStep === 'selection') {
			currentStep = 'options';
		} else if (currentStep === 'options') {
			preparePreview();
		}
	}

	/**
	 * Navigates to the previous step
	 */
	function previousStep(): void {
		if (currentStep === 'options') {
			currentStep = 'selection';
		} else if (currentStep === 'preview') {
			currentStep = 'options';
		}
	}

	/**
	 * Updates MCP sanitization config for a server
	 */
	function handleMcpSanitizationChange(serverId: string, config: MCPSanitizationConfig): void {
		mcpSanitization = {
			...mcpSanitization,
			[serverId]: config
		};
	}

	// Load entities on mount
	$effect(() => {
		loadEntities();
	});
</script>

<div class="export-panel">
	<!-- Header -->
	<div class="panel-header">
		<h2 class="panel-title">Export Settings</h2>
		<div class="step-indicator">
			<Badge variant={currentStep === 'selection' ? 'primary' : 'success'}>
				Step 1: Select
			</Badge>
			<span class="step-arrow">→</span>
			<Badge variant={currentStep === 'options' ? 'primary' : currentStep === 'preview' ? 'success' : 'primary'}>
				Step 2: Options
			</Badge>
			<span class="step-arrow">→</span>
			<Badge variant={currentStep === 'preview' ? 'primary' : 'primary'}>
				Step 3: Preview
			</Badge>
		</div>
	</div>

	{#if message}
		<div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
			{message.text}
		</div>
	{/if}

	{#if error}
		<div class="error-message">
			<Badge variant="error">Error</Badge>
			<span>{error}</span>
		</div>
	{/if}

	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>Loading...</span>
				</div>
			{/snippet}
		</Card>
	{:else}
		<!-- Step 1: Entity Selection -->
		{#if currentStep === 'selection'}
			<Card>
				{#snippet body()}
					<div class="step-content">
						<h3 class="step-title">Select Entities to Export</h3>
						<p class="step-description">
							Choose which agents, MCP servers, models, and prompts to include in the export.
						</p>

						<div class="entity-selectors">
							<EntitySelector
								entityType="agent"
								items={agents}
								selected={selectedAgents}
								onchange={(ids) => (selectedAgents = ids)}
							/>
							<EntitySelector
								entityType="mcp"
								items={mcpServers}
								selected={selectedMcpServers}
								onchange={(ids) => (selectedMcpServers = ids)}
							/>
							<EntitySelector
								entityType="model"
								items={models}
								selected={selectedModels}
								onchange={(ids) => (selectedModels = ids)}
							/>
							<EntitySelector
								entityType="prompt"
								items={prompts}
								selected={selectedPrompts}
								onchange={(ids) => (selectedPrompts = ids)}
							/>
						</div>
					</div>
				{/snippet}
				{#snippet footer()}
					<div class="step-actions">
						<Button variant="primary" onclick={nextStep} disabled={!hasSelection}>
							Next: Options
						</Button>
					</div>
				{/snippet}
			</Card>
		{/if}

		<!-- Step 2: Export Options -->
		{#if currentStep === 'options'}
			<Card>
				{#snippet body()}
					<div class="step-content">
						<h3 class="step-title">Export Options</h3>
						<p class="step-description">
							Configure how the export should be generated.
						</p>

						<div class="options-list">
							<label class="option-item">
								<input
									type="checkbox"
									checked={includeTimestamps}
									onchange={() => (includeTimestamps = !includeTimestamps)}
								/>
								<div class="option-info">
									<span class="option-label">Include Timestamps</span>
									<span class="option-description">
										Export created_at and updated_at timestamps for each entity.
									</span>
								</div>
							</label>

							<label class="option-item">
								<input
									type="checkbox"
									checked={sanitizeMcp}
									onchange={() => (sanitizeMcp = !sanitizeMcp)}
								/>
								<div class="option-info">
									<span class="option-label">Sanitize MCP Environment Variables</span>
									<span class="option-description">
										Review and clear sensitive environment variables before export (recommended).
									</span>
								</div>
							</label>
						</div>
					</div>
				{/snippet}
				{#snippet footer()}
					<div class="step-actions">
						<Button variant="ghost" onclick={previousStep}>
							Back
						</Button>
						<Button variant="primary" onclick={nextStep}>
							Next: Preview
						</Button>
					</div>
				{/snippet}
			</Card>
		{/if}

		<!-- Step 3: Preview -->
		{#if currentStep === 'preview' && preview !== null}
			{@const exportPreview = preview}
			<Card>
				{#snippet body()}
					<div class="step-content">
						<ExportPreview
							preview={exportPreview}
							{mcpSanitization}
							onMcpSanitizationChange={handleMcpSanitizationChange}
						/>
					</div>
				{/snippet}
				{#snippet footer()}
					<div class="step-actions">
						<Button variant="ghost" onclick={previousStep} disabled={exporting}>
							Back
						</Button>
						<Button variant="primary" onclick={generateExport} disabled={exporting}>
							{exporting ? 'Exporting...' : 'Export File'}
						</Button>
					</div>
				{/snippet}
			</Card>
		{/if}
	{/if}
</div>

<style>
	.export-panel {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.panel-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-lg);
		flex-wrap: wrap;
	}

	.panel-title {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.step-indicator {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.step-arrow {
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}

	.message {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.message.success {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.message.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}

	.error-message {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		background: var(--color-error-light);
		color: var(--color-error);
		font-size: var(--font-size-sm);
	}

	.loading-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-2xl);
	}

	.step-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.step-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.step-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.entity-selectors {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
		gap: var(--spacing-md);
	}

	.options-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.option-item {
		display: flex;
		gap: var(--spacing-md);
		align-items: flex-start;
		padding: var(--spacing-md);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition: background 0.2s;
		user-select: none;
	}

	.option-item:hover {
		background: var(--color-bg-hover);
	}

	.option-item input[type='checkbox'] {
		cursor: pointer;
		width: 18px;
		height: 18px;
		margin: 2px 0 0 0;
		flex-shrink: 0;
	}

	.option-info {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.option-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.option-description {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.step-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}

	@media (max-width: 768px) {
		.panel-header {
			flex-direction: column;
			align-items: flex-start;
		}

		.step-indicator {
			flex-direction: column;
			align-items: flex-start;
		}

		.step-arrow {
			transform: rotate(90deg);
		}

		.entity-selectors {
			grid-template-columns: 1fr;
		}
	}
</style>
