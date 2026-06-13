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

ExportPanel - Main export wizard.
Multi-step process: entity selection, options, preview, and export.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriInvoke, saveDialog, isTauriRuntime } from '$lib/tauri';
	import { Button, Card, StatusIndicator, Switch } from '$lib/components/ui';
	import EntitySelector from './EntitySelector.svelte';
	import ExportPreview from './ExportPreview.svelte';
	import ImportExportSteps from './ImportExportSteps.svelte';
	import type { WizardStepItem } from './ImportExportSteps.svelte';
	import { isLocalOrPrivateHttpServer } from './mcp-export.helpers';
	import { Download, Info } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { downloadBrowserFile } from '$lib/utils/browser-download';
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
	import type { SkillSummary } from '$types/skill';
	import type { ProviderInfo } from '$types/custom-provider';

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
	let skills = $state<SkillSummary[]>([]);
	let customProviders = $state<ProviderInfo[]>([]);

	/** Selection state */
	let selectedAgents = $state<string[]>([]);
	let selectedMcpServers = $state<string[]>([]);
	let selectedModels = $state<string[]>([]);
	let selectedPrompts = $state<string[]>([]);
	let selectedSkills = $state<string[]>([]);
	let selectedCustomProviders = $state<string[]>([]);

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

	/** Computed selection */
	const selection = $derived<ExportSelection>({
		agents: selectedAgents,
		mcpServers: selectedMcpServers,
		models: selectedModels,
		prompts: selectedPrompts,
		skills: selectedSkills,
		customProviders: selectedCustomProviders
	});

	/** Check if any entities are selected */
	const hasSelection = $derived(
		selectedAgents.length +
			selectedMcpServers.length +
			selectedModels.length +
			selectedPrompts.length +
			selectedSkills.length +
			selectedCustomProviders.length >
			0
	);

	/** Wizard progress, derived from the current step. */
	const exportSteps = $derived<WizardStepItem[]>([
		{
			label: $i18n('ie_step_select'),
			state: currentStep === 'selection' ? 'current' : 'done'
		},
		{
			label: $i18n('ie_step_options'),
			state: currentStep === 'options' ? 'current' : currentStep === 'preview' ? 'done' : 'upcoming'
		},
		{
			label: $i18n('ie_step_preview'),
			state: currentStep === 'preview' ? 'current' : 'upcoming'
		}
	]);

	/**
	 * Names of HTTP MCP servers whose URL targets a loopback/private address.
	 * These export fine but are refused on re-import (SSRF defense), so we flag
	 * them in the preview. Keyed by name (the import/export identity).
	 */
	const nonReimportableMcp = $derived(
		new Set(
			mcpServers
				.filter((server) => isLocalOrPrivateHttpServer(server.command, server.args))
				.map((server) => server.name)
		)
	);

	/**
	 * Loads available entities from backend
	 */
	async function loadEntities(): Promise<void> {
		loading = true;
		error = null;
		try {
			const allProviders = await Promise.all([
				tauriInvoke<AgentSummary[]>('list_agents'),
				tauriInvoke<MCPServerConfig[]>('list_mcp_servers'),
				tauriInvoke<LLMModel[]>('list_models'),
				tauriInvoke<PromptSummary[]>('list_prompts'),
				tauriInvoke<SkillSummary[]>('list_skills'),
				tauriInvoke<ProviderInfo[]>('list_providers')
			]);
			[agents, mcpServers, models, prompts, skills] = allProviders;
			// Filter to custom providers only (not builtins)
			customProviders = allProviders[5].filter((p) => !p.isBuiltin);
		} catch (err) {
			error = `${$i18n('ie_load_entities_failed')}: ${getErrorMessage(err)}`;
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
			preview = await tauriInvoke<ExportPreviewData>('prepare_export_preview', { selection });

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
			error = `${$i18n('ie_prepare_preview_failed')}: ${getErrorMessage(err)}`;
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
				mcpServers: selection.mcpServers.filter((id) => !mcpSanitization[id]?.excludeFromExport)
			};

			// Remove excluded servers from sanitization config
			const filteredSanitization: Record<string, MCPSanitizationConfig> = {};
			for (const [id, config] of Object.entries(mcpSanitization)) {
				if (!config.excludeFromExport) {
					filteredSanitization[id] = config;
				}
			}

			// Generate export file content
			const exportData = await tauriInvoke<string>('generate_export_file', {
				selection: filteredSelection,
				options,
				sanitization: sanitizeMcp ? filteredSanitization : {}
			});

			const defaultFilename = `zileo-export-${new Date().toISOString().slice(0, 10)}.json`;

			if (!isTauriRuntime()) {
				downloadBrowserFile(defaultFilename, exportData, 'application/json');
				onexport?.(true);
				resetWizard();
				return;
			}

			// Show native save dialog
			const filePath = await saveDialog({
				defaultPath: defaultFilename,
				filters: [
					{
						name: 'JSON',
						extensions: ['json']
					}
				],
				title: $i18n('ie_save_export_title')
			});

			// User cancelled
			if (!filePath) {
				exporting = false;
				return;
			}

			// Save file to selected path
			await tauriInvoke('save_export_to_file', {
				path: filePath,
				content: exportData
			});

			onexport?.(true);
			resetWizard();
		} catch (err) {
			error = `${$i18n('ie_export_failed')}: ${getErrorMessage(err)}`;
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
		selectedSkills = [];
		selectedCustomProviders = [];
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
	onMount(() => {
		loadEntities();
	});
</script>

<div class="export-panel">
	{#if error}
		<div class="alert alert-error" role="alert">
			<span>{error}</span>
		</div>
	{/if}

	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>{$i18n('common_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if currentStep === 'selection'}
		<!-- Step 1: Entity Selection -->
		<Card>
			{#snippet body()}
				<div class="step-content">
					<ImportExportSteps steps={exportSteps} />
					<div class="step-block">
						<h3 class="step-title">{$i18n('ie_select_entities_title')}</h3>
						<p class="step-description">{$i18n('ie_select_entities_description')}</p>

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
							<EntitySelector
								entityType="skill"
								items={skills}
								selected={selectedSkills}
								onchange={(ids) => (selectedSkills = ids)}
							/>
							<EntitySelector
								entityType="custom_provider"
								items={customProviders}
								selected={selectedCustomProviders}
								onchange={(ids) => (selectedCustomProviders = ids)}
							/>
						</div>
					</div>
				</div>
			{/snippet}
			{#snippet footer()}
				<div class="footer-actions">
					<Button variant="primary" onclick={nextStep} disabled={!hasSelection}>
						{$i18n('ie_next_options')}
					</Button>
				</div>
			{/snippet}
		</Card>
	{:else if currentStep === 'options'}
		<!-- Step 2: Export Options -->
		<Card>
			{#snippet body()}
				<div class="step-content">
					<ImportExportSteps steps={exportSteps} />
					<div class="step-block">
						<h3 class="step-title">{$i18n('ie_export_options_title')}</h3>
						<p class="step-description">{$i18n('ie_export_options_description')}</p>

						<div class="toggle-stack">
							<div class="toggle-row">
								<span class="toggle-text">
									<strong id="ie-opt-timestamps">{$i18n('ie_include_timestamps')}</strong>
									<span>{$i18n('ie_include_timestamps_description')}</span>
								</span>
								<Switch
									checked={includeTimestamps}
									onchange={(v) => (includeTimestamps = v)}
									labelledBy="ie-opt-timestamps"
								/>
							</div>
							<div class="toggle-row">
								<span class="toggle-text">
									<strong id="ie-opt-sanitize">{$i18n('ie_sanitize_mcp')}</strong>
									<span>{$i18n('ie_sanitize_mcp_description')}</span>
								</span>
								<Switch
									checked={sanitizeMcp}
									onchange={(v) => (sanitizeMcp = v)}
									labelledBy="ie-opt-sanitize"
								/>
							</div>
						</div>

						<div class="alert alert-info" role="note">
							<Info size={18} aria-hidden="true" />
							<span>{$i18n('ie_references_by_name_note')}</span>
						</div>
					</div>
				</div>
			{/snippet}
			{#snippet footer()}
				<div class="footer-actions">
					<Button variant="ghost" onclick={previousStep}>
						{$i18n('common_cancel')}
					</Button>
					<Button variant="primary" onclick={nextStep}>
						{$i18n('ie_next_preview')}
					</Button>
				</div>
			{/snippet}
		</Card>
	{:else if currentStep === 'preview' && preview !== null}
		<!-- Step 3: Preview -->
		{@const exportPreview = preview}
		<Card>
			{#snippet body()}
				<div class="step-content">
					<ImportExportSteps steps={exportSteps} />
					<ExportPreview
						preview={exportPreview}
						{mcpSanitization}
						{nonReimportableMcp}
						onMcpSanitizationChange={handleMcpSanitizationChange}
					/>
				</div>
			{/snippet}
			{#snippet footer()}
				<div class="footer-actions">
					<Button variant="ghost" onclick={previousStep} disabled={exporting}>
						{$i18n('common_cancel')}
					</Button>
					<Button variant="primary" onclick={generateExport} disabled={exporting}>
						<Download size={16} aria-hidden="true" />
						<span>{exporting ? $i18n('ie_exporting') : $i18n('ie_export_file')}</span>
					</Button>
				</div>
			{/snippet}
		</Card>
	{/if}
</div>

<style>
	.export-panel {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	/* Alerts: a tinted band with a 1px semantic border (info note / errors). */
	.alert {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		border: 1px solid;
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.alert :global(svg) {
		flex-shrink: 0;
	}

	.alert-info {
		background: var(--color-info-light);
		border-color: color-mix(in srgb, var(--color-info) 30%, transparent);
		color: var(--color-info);
	}

	.alert-error {
		background: var(--color-error-light);
		border-color: var(--color-error-border);
		color: var(--color-error);
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

	.step-block {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
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

	/* Toggle rows: text block on the left, Switch on the right (mirrors the
	   shared settings convention used across the validation/agents surfaces). */
	.toggle-stack {
		display: flex;
		flex-direction: column;
	}

	.toggle-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
		padding: var(--spacing-sm) 0;
	}

	.toggle-row + .toggle-row {
		border-top: 1px solid var(--color-border-light);
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

	.footer-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}

	@media (max-width: 768px) {
		.entity-selectors {
			grid-template-columns: 1fr;
		}
	}
</style>
