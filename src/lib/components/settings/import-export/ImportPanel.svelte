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

ImportPanel - Main import wizard for configuration entities.
Orchestrates the multi-step import process:
1. Upload file
2. Preview entities
3. Resolve conflicts (if any)
4. Fill missing MCP env vars (if any)
5. Execute import
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button, Card, Badge } from '$lib/components/ui';
	import ImportPreview from './ImportPreview.svelte';
	import ConflictResolver from './ConflictResolver.svelte';
	import MCPEnvEditor from './MCPEnvEditor.svelte';
	import { i18n } from '$lib/i18n';
	import type {
		ImportValidation,
		ImportSelection,
		ImportConflict,
		ConflictResolution,
		MCPAdditions,
		ConfigImportResult,
		ExportPackage
	} from '$types/importExport';
	import { MAX_IMPORT_FILE_SIZE } from '$types/importExport';
	import { Upload, CheckCircle, AlertCircle } from '@lucide/svelte';

	/** Props */
	interface Props {
		/** Import completion callback */
		onimport?: (success: boolean) => void | Promise<void>;
	}

	let { onimport }: Props = $props();

	/** Wizard step state */
	type WizardStep = 'upload' | 'preview' | 'conflicts' | 'mcp_env' | 'executing' | 'complete';
	let currentStep = $state<WizardStep>('upload');

	/** Import data state */
	let importData = $state<ExportPackage | null>(null);
	let validation = $state<ImportValidation | null>(null);
	let selection = $state<ImportSelection>({
		agents: [],
		mcpServers: [],
		models: [],
		prompts: []
	});
	let resolutions = $state<Record<string, ConflictResolution>>({});
	let mcpAdditionsMap = $state<Record<string, MCPAdditions>>({});

	/** UI state */
	let loading = $state(false);
	let error = $state<string | null>(null);
	let result = $state<ConfigImportResult | null>(null);

	/**
	 * Filter conflicts to only include those for selected entities.
	 * This ensures step 3 "Configure" only shows conflicts for items selected in step 2 "Preview".
	 * Selection is now by NAME (not ID) since IDs are not exported.
	 */
	const filteredConflicts = $derived(() => {
		if (!validation) return [];
		return validation.conflicts.filter((conflict) => {
			switch (conflict.entityType) {
				case 'agent':
					return selection.agents.includes(conflict.entityName);
				case 'mcp':
					return selection.mcpServers.includes(conflict.entityName);
				case 'model':
					return selection.models.includes(conflict.entityName);
				case 'prompt':
					return selection.prompts.includes(conflict.entityName);
				default:
					return false;
			}
		});
	});

	/**
	 * Filter missing MCP env to only include those for selected MCP servers.
	 * Selection is now by NAME (not ID) since IDs are not exported.
	 */
	const filteredMissingMcpEnv = $derived(() => {
		if (!validation) return {};
		const filtered: Record<string, string[]> = {};
		// missingMcpEnv is now keyed by server NAME (not ID)
		for (const [serverName, keys] of Object.entries(validation.missingMcpEnv)) {
			if (selection.mcpServers.includes(serverName)) {
				filtered[serverName] = keys;
			}
		}
		return filtered;
	});

	/**
	 * Handle file upload
	 */
	async function handleFileUpload(): Promise<void> {
		const input = document.createElement('input');
		input.type = 'file';
		input.accept = '.json';

		input.onchange = async (e) => {
			const file = (e.target as HTMLInputElement).files?.[0];
			if (!file) return;

			error = null;

			// Validate file size
			if (file.size > MAX_IMPORT_FILE_SIZE) {
				error = $i18n('ie_file_too_large').replace('{size}', String(MAX_IMPORT_FILE_SIZE / (1024 * 1024)));
				return;
			}

			loading = true;
			try {
				const text = await file.text();
				const data = JSON.parse(text) as ExportPackage;
				importData = data;

				// Validate import
				validation = await invoke<ImportValidation>('validate_import', { data: text });

				if (!validation.valid) {
					error = `${$i18n('ie_invalid_import_file')}: ${validation.errors.join(', ')}`;
					loading = false;
					return;
				}

				// Initialize selection with all entities - using NAME as identifier (not ID)
				selection = {
					agents: validation.entities.agents.map((a) => a.name),
					mcpServers: validation.entities.mcpServers.map((s) => s.name),
					models: validation.entities.models.map((m) => m.name),
					prompts: validation.entities.prompts.map((p) => p.name)
				};

				// Initialize MCP additions for servers with missing env
				mcpAdditionsMap = {};
				for (const [serverId, missingKeys] of Object.entries(validation.missingMcpEnv)) {
					if (missingKeys.length > 0) {
						mcpAdditionsMap[serverId] = {
							addEnv: {},
							addArgs: []
						};
					}
				}

				currentStep = 'preview';
			} catch (err) {
				error = `${$i18n('ie_parse_failed')}: ${err}`;
			} finally {
				loading = false;
			}
		};

		input.click();
	}

	/**
	 * Handle selection change
	 */
	function handleSelectionChange(newSelection: ImportSelection): void {
		selection = newSelection;
	}

	/**
	 * Handle resolution change
	 */
	function handleResolutionChange(newResolutions: Record<string, ConflictResolution>): void {
		resolutions = newResolutions;
	}

	/**
	 * Handle MCP additions change
	 */
	function handleMCPAdditionsChange(serverId: string, additions: MCPAdditions): void {
		mcpAdditionsMap = {
			...mcpAdditionsMap,
			[serverId]: additions
		};
	}

	/**
	 * Proceed to next step
	 */
	function handleNext(): void {
		if (!validation) return;

		if (currentStep === 'preview') {
			// Check if there are conflicts for selected entities
			if (filteredConflicts().length > 0) {
				currentStep = 'conflicts';
			} else if (Object.keys(filteredMissingMcpEnv()).length > 0) {
				currentStep = 'mcp_env';
			} else {
				executeImport();
			}
		} else if (currentStep === 'conflicts') {
			// Check if there are missing MCP env vars for selected servers
			if (Object.keys(filteredMissingMcpEnv()).length > 0) {
				currentStep = 'mcp_env';
			} else {
				executeImport();
			}
		} else if (currentStep === 'mcp_env') {
			executeImport();
		}
	}

	/**
	 * Go back to previous step
	 */
	function handleBack(): void {
		if (currentStep === 'conflicts') {
			currentStep = 'preview';
		} else if (currentStep === 'mcp_env') {
			if (filteredConflicts().length > 0) {
				currentStep = 'conflicts';
			} else {
				currentStep = 'preview';
			}
		}
	}

	/**
	 * Execute import
	 */
	async function executeImport(): Promise<void> {
		if (!importData || !validation) return;

		currentStep = 'executing';
		loading = true;
		error = null;

		try {
			const importDataStr = JSON.stringify(importData);

			result = await invoke<ConfigImportResult>('execute_import', {
				data: importDataStr,
				selection,
				resolutions,
				mcpAdditions: mcpAdditionsMap
			});

			currentStep = 'complete';
			// CRITICAL: Await the callback to ensure stores are refreshed before UI updates
			await onimport?.(result.success);
		} catch (err) {
			error = `${$i18n('ie_import_failed')}: ${err}`;
			currentStep = 'preview';
		} finally {
			loading = false;
		}
	}

	/**
	 * Reset wizard
	 */
	function handleReset(): void {
		currentStep = 'upload';
		importData = null;
		validation = null;
		selection = { agents: [], mcpServers: [], models: [], prompts: [] };
		resolutions = {};
		mcpAdditionsMap = {};
		error = null;
		result = null;
	}

	/**
	 * Generate composite key for conflict resolution.
	 * Uses entityType:entityName to avoid collisions between different entity types.
	 * NOTE: entityName is the unique identifier (IDs are not exported).
	 */
	function getConflictKey(conflict: ImportConflict): string {
		return `${conflict.entityType}:${conflict.entityName}`;
	}

	/**
	 * Check if next button should be enabled
	 */
	const canProceed = $derived(() => {
		if (!validation) return false;

		if (currentStep === 'preview') {
			// At least one entity must be selected
			const hasSelection =
				selection.agents.length > 0 ||
				selection.mcpServers.length > 0 ||
				selection.models.length > 0 ||
				selection.prompts.length > 0;
			return hasSelection;
		}

		if (currentStep === 'conflicts') {
			// All filtered conflicts must be resolved
			const conflicts = filteredConflicts();
			return conflicts.every((c) => resolutions[getConflictKey(c)]);
		}

		if (currentStep === 'mcp_env') {
			// All required env vars must be filled for selected MCP servers
			const missingEnv = filteredMissingMcpEnv();
			return Object.entries(missingEnv).every(([serverId, keys]) => {
				const additions = mcpAdditionsMap[serverId];
				if (!additions) return false;
				// Check sensitive keys are filled
				const sensitiveKeys = keys.filter((key) =>
					['API_KEY', 'SECRET', 'TOKEN', 'PASSWORD', 'CREDENTIAL', 'PRIVATE_KEY'].some((pattern) =>
						key.toUpperCase().includes(pattern)
					)
				);
				return sensitiveKeys.every((key) => additions.addEnv[key]?.trim());
			});
		}

		return false;
	});

</script>

<div class="import-panel">
	<!-- Step Indicator -->
	<div class="step-indicator">
		<div class="step" class:active={currentStep === 'upload'} class:completed={currentStep !== 'upload'}>
			<div class="step-number">1</div>
			<div class="step-label">{$i18n('ie_step_upload')}</div>
		</div>
		<div class="step-divider"></div>
		<div
			class="step"
			class:active={currentStep === 'preview'}
			class:completed={['conflicts', 'mcp_env', 'executing', 'complete'].includes(currentStep)}
		>
			<div class="step-number">2</div>
			<div class="step-label">{$i18n('ie_step_preview_label')}</div>
		</div>
		{#if validation && (filteredConflicts().length > 0 || Object.keys(filteredMissingMcpEnv()).length > 0)}
			<div class="step-divider"></div>
			<div
				class="step"
				class:active={currentStep === 'conflicts' || currentStep === 'mcp_env'}
				class:completed={['executing', 'complete'].includes(currentStep)}
			>
				<div class="step-number">3</div>
				<div class="step-label">{$i18n('ie_step_configure')}</div>
			</div>
		{/if}
		<div class="step-divider"></div>
		<div
			class="step"
			class:active={currentStep === 'executing'}
			class:completed={currentStep === 'complete'}
		>
			<div class="step-number">{validation && (filteredConflicts().length > 0 || Object.keys(filteredMissingMcpEnv()).length > 0) ? '4' : '3'}</div>
			<div class="step-label">{$i18n('ie_step_import_label')}</div>
		</div>
	</div>

	<!-- Error Message -->
	{#if error}
		<Card>
			{#snippet body()}
				<div class="error-message">
					<AlertCircle size={24} />
					<p>{error}</p>
				</div>
			{/snippet}
		</Card>
	{/if}

	<!-- Step Content -->
	<div class="step-content">
		{#if currentStep === 'upload'}
			<Card title={$i18n('ie_import_config_title')}>
				{#snippet body()}
					<div class="upload-content">
						<p class="upload-description">
							{$i18n('ie_import_config_description')}
						</p>
						<Button variant="primary" onclick={handleFileUpload} disabled={loading}>
							<Upload size={20} />
							<span>{loading ? $i18n('common_loading') : $i18n('ie_select_file')}</span>
						</Button>
						<p class="upload-help">
							{$i18n('ie_max_file_size').replace('{size}', String(MAX_IMPORT_FILE_SIZE / (1024 * 1024)))}
						</p>
					</div>
				{/snippet}
			</Card>
		{:else if currentStep === 'preview' && validation}
			{@const previewValidation = validation}
			<div class="preview-content">
				{#if previewValidation.warnings.length > 0}
					<Card>
						{#snippet body()}
							<div class="warnings">
								<Badge variant="warning">{$i18n('ie_warnings')}</Badge>
								<ul class="warning-list">
									{#each previewValidation.warnings as warning}
										<li>{warning}</li>
									{/each}
								</ul>
							</div>
						{/snippet}
					</Card>
				{/if}
				<ImportPreview
					validation={previewValidation}
					{selection}
					onSelectionChange={handleSelectionChange}
				/>
			</div>
		{:else if currentStep === 'conflicts' && validation}
			<ConflictResolver
				conflicts={filteredConflicts()}
				{resolutions}
				onResolve={handleResolutionChange}
			/>
		{:else if currentStep === 'mcp_env' && validation}
			<div class="mcp-env-content">
				{#each Object.entries(filteredMissingMcpEnv()) as [serverName, missingKeys]}
					{@const server = validation.entities.mcpServers.find((s) => s.name === serverName)}
					{#if server && missingKeys.length > 0}
						<MCPEnvEditor
							serverId={serverName}
							serverName={server.name}
							{missingKeys}
							additions={mcpAdditionsMap[serverName] || { addEnv: {}, addArgs: [] }}
							onchange={(additions) => handleMCPAdditionsChange(serverName, additions)}
						/>
					{/if}
				{/each}
			</div>
		{:else if currentStep === 'executing'}
			<Card>
				{#snippet body()}
					<div class="executing-content">
						<div class="spinner"></div>
						<h3>{$i18n('ie_importing_config')}</h3>
						<p>{$i18n('ie_importing_wait')}</p>
					</div>
				{/snippet}
			</Card>
		{:else if currentStep === 'complete' && result}
			{@const completeResult = result}
			<Card>
				{#snippet body()}
					<div class="complete-content">
						{#if completeResult.success}
							<CheckCircle size={48} class="success-icon" />
							<h3>{$i18n('ie_import_complete')}</h3>
							<div class="result-summary">
								<div class="result-row">
									<span class="result-label">{$i18n('ie_imported')}</span>
									<div class="result-counts">
										{#if completeResult.imported.agents > 0}
											<Badge variant="success">{completeResult.imported.agents} {$i18n('ie_entity_agents')}</Badge>
										{/if}
										{#if completeResult.imported.mcpServers > 0}
											<Badge variant="success">{completeResult.imported.mcpServers} {$i18n('ie_entity_mcp_servers')}</Badge>
										{/if}
										{#if completeResult.imported.models > 0}
											<Badge variant="success">{completeResult.imported.models} {$i18n('ie_entity_models')}</Badge>
										{/if}
										{#if completeResult.imported.prompts > 0}
											<Badge variant="success">{completeResult.imported.prompts} {$i18n('ie_entity_prompts')}</Badge>
										{/if}
									</div>
								</div>
								{#if completeResult.skipped.agents > 0 || completeResult.skipped.mcpServers > 0 || completeResult.skipped.models > 0 || completeResult.skipped.prompts > 0}
									<div class="result-row">
										<span class="result-label">{$i18n('ie_skipped')}</span>
										<div class="result-counts">
											{#if completeResult.skipped.agents > 0}
												<Badge variant="warning">{completeResult.skipped.agents} {$i18n('ie_entity_agents')}</Badge>
											{/if}
											{#if completeResult.skipped.mcpServers > 0}
												<Badge variant="warning">{completeResult.skipped.mcpServers} {$i18n('ie_entity_mcp_servers')}</Badge>
											{/if}
											{#if completeResult.skipped.models > 0}
												<Badge variant="warning">{completeResult.skipped.models} {$i18n('ie_entity_models')}</Badge>
											{/if}
											{#if completeResult.skipped.prompts > 0}
												<Badge variant="warning">{completeResult.skipped.prompts} {$i18n('ie_entity_prompts')}</Badge>
											{/if}
										</div>
									</div>
								{/if}
							</div>
							{#if completeResult.errors.length > 0}
								<div class="errors">
									<Badge variant="error">{$i18n('ie_errors')}</Badge>
									<ul class="error-list">
										{#each completeResult.errors as importError}
											<li>
												{importError.entityType}: {importError.entityId} - {importError.error}
											</li>
										{/each}
									</ul>
								</div>
							{/if}
						{:else}
							<AlertCircle size={48} class="error-icon" />
							<h3>{$i18n('ie_import_failed_title')}</h3>
							<p>{$i18n('ie_import_failed_description')}</p>
							{#if completeResult.errors.length > 0}
								<ul class="error-list">
									{#each completeResult.errors as importError}
										<li>
											{importError.entityType}: {importError.entityId} - {importError.error}
										</li>
									{/each}
								</ul>
							{/if}
						{/if}
					</div>
				{/snippet}
			</Card>
		{/if}
	</div>

	<!-- Actions -->
	{#if currentStep !== 'upload' && currentStep !== 'executing' && currentStep !== 'complete'}
		<div class="actions">
			<Button variant="ghost" onclick={handleBack} disabled={loading}>
				{$i18n('common_cancel')}
			</Button>
			<Button variant="primary" onclick={handleNext} disabled={loading || !canProceed()}>
				{currentStep === 'mcp_env' ? $i18n('ie_tab_import') : $i18n('ie_next')}
			</Button>
		</div>
	{:else if currentStep === 'complete'}
		<div class="actions">
			<Button variant="primary" onclick={handleReset}>
				{$i18n('ie_import_another')}
			</Button>
		</div>
	{/if}
</div>

<style>
	.import-panel {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.step-indicator {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-lg);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.step {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-xs);
		opacity: 0.5;
		transition: opacity 0.2s;
	}

	.step.active,
	.step.completed {
		opacity: 1;
	}

	.step-number {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background: var(--color-bg-tertiary);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
	}

	.step.active .step-number {
		background: var(--color-primary);
		color: white;
	}

	.step.completed .step-number {
		background: var(--color-success);
		color: white;
	}

	.step-label {
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		text-align: center;
	}

	.step-divider {
		width: 40px;
		height: 2px;
		background: var(--color-border);
		margin: 0 var(--spacing-xs);
	}

	.error-message {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--color-error-light);
		border: 1px solid var(--color-error);
		border-radius: var(--border-radius-md);
		color: var(--color-error);
	}

	.error-message p {
		margin: 0;
		flex: 1;
	}

	.step-content {
		min-height: 300px;
	}

	.upload-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
		padding: var(--spacing-2xl);
		text-align: center;
	}

	.upload-description {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		max-width: 500px;
	}

	.upload-help {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.preview-content,
	.mcp-env-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.warnings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.warning-list,
	.error-list {
		margin: 0;
		padding-left: var(--spacing-lg);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.executing-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
		padding: var(--spacing-2xl);
		text-align: center;
	}

	.spinner {
		width: 48px;
		height: 48px;
		border: 4px solid var(--color-border);
		border-top-color: var(--color-primary);
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.executing-content h3 {
		margin: 0;
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.executing-content p {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.complete-content {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
		padding: var(--spacing-2xl);
		text-align: center;
	}

	.complete-content h3 {
		margin: 0;
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.result-summary {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		width: 100%;
		max-width: 600px;
	}

	.result-row {
		display: flex;
		gap: var(--spacing-md);
		align-items: flex-start;
	}

	.result-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		min-width: 80px;
		text-align: left;
	}

	.result-counts {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
		flex: 1;
	}

	.errors {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		width: 100%;
		max-width: 600px;
	}

	.actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding-top: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
	}

	@media (max-width: 768px) {
		.step-indicator {
			overflow-x: auto;
		}

		.step-label {
			display: none;
		}

		.step-divider {
			width: 20px;
		}

		.actions {
			flex-direction: column-reverse;
		}
	}
</style>
