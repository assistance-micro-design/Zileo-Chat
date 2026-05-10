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

<script lang="ts">
	/**
	 * ValidationSettings component
	 * Manages global validation settings configuration
	 *
	 * Functional options:
	 * - Mode (Auto/Manual/Selective)
	 * - Selective: Sub-Agent operations, Tools, MCP servers
	 * - Risk Thresholds (autoApproveLow, alwaysConfirmHigh)
	 */
	import { onMount } from 'svelte';
	import { tauriInvoke } from '$lib/tauri';
	import { Button, ErrorBanner } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import {
		validationSettingsStore,
		settings,
		isLoading,
		isSaving
	} from '$lib/stores/validation-settings';
	import { loadServers } from '$lib/stores/mcp';
	import { toastStore } from '$lib/stores/toast';
	import { auditLogStore } from '$lib/stores/audit-log';
	import { dispatchSettingsRefresh } from '$lib/utils/settings-refresh';
	import type { ToastType } from '$types/background-workflow';
	import ValidationInfoCard from './ValidationInfoCard.svelte';
	import {
		RETENTION_MAX,
		RETENTION_MIN,
		TIMEOUT_MAX,
		TIMEOUT_MIN,
		clampRetention,
		clampTimeout,
		createValidationSettingsUpdateRequest,
		getAutoManualModeDisplay,
		modeOptions,
		splitAvailableTools,
		timeoutBehaviorOptions
	} from './ValidationSettings.helpers';
	import type { ValidationMode, TimeoutBehavior, AvailableToolInfo } from '$types/validation';
	import type { MCPServer } from '$types/mcp';

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	// Local form state (copied from store on load)
	let localMode = $state<ValidationMode>('selective');
	let localSubAgentsValidation = $state(true);
	let localToolsValidation = $state(false);
	let localMcpValidation = $state(false);
	let localRiskThresholds = $state({
		autoApproveLow: true,
		alwaysConfirmHigh: false
	});

	// Timeout + audit local state.
	// Bounds mirror the backend constants (validation::VALIDATION_TIMEOUT_MIN/MAX_SECS,
	// audit::RETENTION_MIN/MAX_DAYS).
	let localTimeoutSeconds = $state(60);
	let localTimeoutBehavior = $state<TimeoutBehavior>('reject');
	let localEnableLogging = $state(true);
	let localRetentionDays = $state(30);
	let purging = $state(false);

	async function handlePurgeNow(): Promise<void> {
		purging = true;
		errorMessage = null;
		try {
			// Route through the audit-log store so its in-memory state stays in sync
			// (refreshes the entries list). We then fire the global settings:refresh
			// event so an open audit-log page reloads its stats too.
			const deleted = await auditLogStore.purgeNow();
			dispatchSettingsRefresh();
			notify(
				'success',
				$i18n('validation_audit_purge_success').replace('{count}', String(deleted))
			);
		} catch (err) {
			errorMessage = $i18n('validation_audit_purge_failed').replace(
				'{error}',
				getErrorMessage(err)
			);
		} finally {
			purging = false;
		}
	}

	// Available tools and MCP servers
	let availableTools = $state<AvailableToolInfo[]>([]);
	let mcpServers = $state<MCPServer[]>([]);
	let loadingResources = $state(false);

	// UI state
	let errorMessage = $state<string | null>(null);
	let hasChanges = $state(false);

	// Derived: available tools split by validation category
	const splitTools = $derived(splitAvailableTools(availableTools));
	const basicTools = $derived(splitTools.basicTools);
	const subAgentTools = $derived(splitTools.subAgentTools);

	// Load settings and resources on mount
	onMount(async () => {
		try {
			await Promise.all([validationSettingsStore.loadSettings(), loadAvailableResources()]);
		} catch (err) {
			errorMessage = $i18n('validation_load_resources_failed').replace(
				'{error}',
				getErrorMessage(err)
			);
		}
	});

	// Load available tools and MCP servers
	async function loadAvailableResources(): Promise<void> {
		loadingResources = true;
		try {
			const [tools, servers] = await Promise.all([
				tauriInvoke<AvailableToolInfo[]>('list_available_tools'),
				loadServers(true) // Force refresh
			]);
			availableTools = tools;
			mcpServers = servers;
		} catch (err) {
			errorMessage = $i18n('validation_load_resources_failed').replace(
				'{error}',
				getErrorMessage(err)
			);
		} finally {
			loadingResources = false;
		}
	}

	// Sync local state when store settings change
	$effect(() => {
		const s = $settings;
		if (s) {
			localMode = s.mode;
			localSubAgentsValidation = s.selectiveConfig.subAgents;
			localToolsValidation = s.selectiveConfig.tools;
			localMcpValidation = s.selectiveConfig.mcp;
			localRiskThresholds = { ...s.riskThresholds };
			localTimeoutSeconds = clampTimeout(s.timeoutSeconds);
			localTimeoutBehavior = s.timeoutBehavior;
			localEnableLogging = s.audit.enableLogging;
			localRetentionDays = clampRetention(s.audit.retentionDays);
			hasChanges = false;
		}
	});

	// Track changes
	function markChanged(): void {
		hasChanges = true;
	}

	// Handle mode selection
	function selectMode(mode: ValidationMode): void {
		localMode = mode;
		markChanged();
	}

	// Handle save
	async function handleSave(): Promise<void> {
		errorMessage = null;
		try {
			const updateRequest = createValidationSettingsUpdateRequest({
				mode: localMode,
				subAgentsValidation: localSubAgentsValidation,
				toolsValidation: localToolsValidation,
				mcpValidation: localMcpValidation,
				riskThresholds: localRiskThresholds,
				timeoutSeconds: localTimeoutSeconds,
				timeoutBehavior: localTimeoutBehavior,
				enableLogging: localEnableLogging,
				retentionDays: localRetentionDays
			});
			await validationSettingsStore.updateSettings(updateRequest);
			notify('success', $i18n('validation_saved'));
			hasChanges = false;
		} catch (err) {
			errorMessage = $i18n('validation_save_failed').replace('{error}', getErrorMessage(err));
		}
	}

	// Handle reset to defaults
	async function handleReset(): Promise<void> {
		errorMessage = null;
		try {
			await validationSettingsStore.resetToDefaults();
			notify('success', $i18n('validation_reset_success'));
			hasChanges = false;
		} catch (err) {
			errorMessage = $i18n('validation_reset_failed').replace('{error}', getErrorMessage(err));
		}
	}
</script>

<!-- Shared snippet: renders a list of tool badges -->
{#snippet toolBadgeList(tools: AvailableToolInfo[], badgeClass: string)}
	{#if tools.length > 0}
		<div class="item-list">
			{#each tools as tool (tool.name)}
				<span class="item-badge {badgeClass}">{tool.name}</span>
			{/each}
		</div>
	{/if}
{/snippet}

<!-- Shared snippet: renders MCP server badges with loading/empty/status states -->
{#snippet mcpBadgeList(badgeClass: string)}
	{#if loadingResources}
		<span class="loading-text">{$i18n('common_loading')}</span>
	{:else if mcpServers.length > 0}
		<div class="item-list">
			{#each mcpServers as server (server.name)}
				<span class="item-badge {badgeClass}" class:running={server.status === 'running'}>
					{server.name}
					{#if server.status === 'running'}
						<span class="status-dot running"></span>
					{:else}
						<span class="status-dot stopped"></span>
					{/if}
				</span>
			{/each}
		</div>
	{:else}
		<span class="no-items">{$i18n('validation_no_mcp_servers')}</span>
	{/if}
{/snippet}

<div class="validation-settings">
	{#if errorMessage}
		<ErrorBanner message={errorMessage} onDismiss={() => (errorMessage = null)} />
	{/if}

	{#if $isLoading}
		<div class="loading-state">
			<span class="spinner"></span>
			<span>{$i18n('validation_loading')}</span>
		</div>
	{:else}
		<!-- Mode Selector -->
		<div class="settings-section">
			<h3 class="section-title">{$i18n('validation_mode_title')}</h3>
			<div class="card-selector" role="group" aria-label={$i18n('validation_mode_title')}>
				{#each modeOptions as option (option.value)}
					<button
						type="button"
						class="selector-card"
						class:selected={localMode === option.value}
						onclick={() => selectMode(option.value)}
					>
						<span class="selector-card-title">{$i18n(option.labelKey)}</span>
						<span class="selector-card-description">{$i18n(option.descKey)}</span>
					</button>
				{/each}
			</div>
			{#if localMode === 'auto'}
				<div class="mode-banner warning">
					<span class="mode-banner-icon">!</span>
					<div class="mode-banner-content">
						<span class="mode-banner-title">{$i18n('validation_auto_multi_workflow_title')}</span>
						<span class="mode-banner-text">{$i18n('validation_auto_multi_workflow_desc')}</span>
					</div>
				</div>
			{/if}
			{#if localMode === 'manual' || localMode === 'selective'}
				<div class="mode-banner info">
					<span class="mode-banner-icon">i</span>
					<div class="mode-banner-content">
						<span class="mode-banner-title">{$i18n('validation_single_workflow_title')}</span>
						<span class="mode-banner-text">{$i18n('validation_single_workflow_desc')}</span>
					</div>
				</div>
			{/if}
		</div>

		<!-- Auto/Manual Mode Information (merged - identical structure, different variant) -->
		{#if localMode === 'auto' || localMode === 'manual'}
			{@const modeDisplay = getAutoManualModeDisplay(localMode)}
			{@const variant = modeDisplay.variant}
			{@const icon = modeDisplay.icon}
			{@const statusKey = modeDisplay.statusKey}
			{@const sectionTitleKey = modeDisplay.sectionTitleKey}
			{@const sectionHelpKey = modeDisplay.sectionHelpKey}

			<div class="settings-section">
				<h3 class="section-title">{$i18n(sectionTitleKey)}</h3>
				<p class="section-help">{$i18n(sectionHelpKey)}</p>

				<div class="info-cards">
					<ValidationInfoCard {variant} {icon} titleKey="validation_sub_agents" {statusKey}>
						{@render toolBadgeList(subAgentTools, variant)}
					</ValidationInfoCard>

					<ValidationInfoCard {variant} {icon} titleKey="validation_tools" {statusKey}>
						{@render toolBadgeList(basicTools, variant)}
					</ValidationInfoCard>

					<ValidationInfoCard {variant} {icon} titleKey="validation_mcp" {statusKey}>
						{@render mcpBadgeList(variant)}
					</ValidationInfoCard>
				</div>
			</div>
		{/if}

		<!-- Selective Configuration -->
		{#if localMode === 'selective'}
			<div class="settings-section">
				<h3 class="section-title">{$i18n('validation_selective_title')}</h3>
				<p class="section-help">{$i18n('validation_selective_help')}</p>

				<div class="checkbox-group">
					<!-- Sub-Agents Validation -->
					<label class="checkbox-item">
						<input type="checkbox" bind:checked={localSubAgentsValidation} onchange={markChanged} />
						<div class="checkbox-content">
							<span class="checkbox-label">{$i18n('validation_sub_agents')}</span>
							<span class="checkbox-description">{$i18n('validation_sub_agents_desc')}</span>
							{@render toolBadgeList(subAgentTools, localSubAgentsValidation ? 'enabled' : '')}
						</div>
					</label>

					<!-- Tools Validation -->
					<label class="checkbox-item">
						<input type="checkbox" bind:checked={localToolsValidation} onchange={markChanged} />
						<div class="checkbox-content">
							<span class="checkbox-label">{$i18n('validation_tools')}</span>
							<span class="checkbox-description">{$i18n('validation_tools_desc')}</span>
							{@render toolBadgeList(basicTools, localToolsValidation ? 'enabled' : '')}
						</div>
					</label>

					<!-- MCP Servers Validation -->
					<label class="checkbox-item">
						<input type="checkbox" bind:checked={localMcpValidation} onchange={markChanged} />
						<div class="checkbox-content">
							<span class="checkbox-label">{$i18n('validation_mcp')}</span>
							<span class="checkbox-description">{$i18n('validation_mcp_desc')}</span>
							{@render mcpBadgeList(localMcpValidation ? 'enabled' : '')}
						</div>
					</label>
				</div>
			</div>
		{/if}

		<!-- Risk Thresholds -->
		<div class="settings-section">
			<h3 class="section-title">{$i18n('validation_risk_title')}</h3>
			<div class="checkbox-group">
				<label class="checkbox-item">
					<input
						type="checkbox"
						bind:checked={localRiskThresholds.autoApproveLow}
						onchange={markChanged}
					/>
					<div class="checkbox-content">
						<span class="checkbox-label">{$i18n('validation_risk_auto_approve_low')}</span>
						<span class="checkbox-description"
							>{$i18n('validation_risk_auto_approve_low_desc')}</span
						>
					</div>
				</label>
				<label class="checkbox-item">
					<input
						type="checkbox"
						bind:checked={localRiskThresholds.alwaysConfirmHigh}
						onchange={markChanged}
					/>
					<div class="checkbox-content">
						<span class="checkbox-label">{$i18n('validation_risk_always_confirm_high')}</span>
						<span class="checkbox-description warning"
							>{$i18n('validation_risk_always_confirm_high_desc')}</span
						>
					</div>
				</label>
			</div>
		</div>

		<!-- Timeout Settings -->
		<div class="settings-section">
			<h3 class="section-title">{$i18n('validation_timeout_title')}</h3>
			<p class="section-help">{$i18n('validation_timeout_help')}</p>

			<label class="slider-row">
				<span class="slider-label">
					{$i18n('validation_timeout_seconds_label')}
					<span class="slider-value">{localTimeoutSeconds}s</span>
				</span>
				<input
					type="range"
					min={TIMEOUT_MIN}
					max={TIMEOUT_MAX}
					step="5"
					bind:value={localTimeoutSeconds}
					oninput={() => {
						localTimeoutSeconds = clampTimeout(localTimeoutSeconds);
						markChanged();
					}}
				/>
				<span class="slider-bounds">
					<span>{TIMEOUT_MIN}s</span><span>{TIMEOUT_MAX}s</span>
				</span>
			</label>

			<fieldset class="radio-group">
				<legend class="radio-group-legend">{$i18n('validation_timeout_behavior_label')}</legend>
				{#each timeoutBehaviorOptions as opt (opt.value)}
					<label class="radio-item">
						<input
							type="radio"
							name="timeout-behavior"
							value={opt.value}
							checked={localTimeoutBehavior === opt.value}
							onchange={() => {
								localTimeoutBehavior = opt.value;
								markChanged();
							}}
						/>
						<span class="radio-label">{$i18n(opt.labelKey)}</span>
					</label>
				{/each}
			</fieldset>
		</div>

		<!-- Audit Logging -->
		<div class="settings-section">
			<h3 class="section-title">{$i18n('validation_audit_title')}</h3>
			<p class="section-help">{$i18n('validation_audit_help')}</p>

			<label class="checkbox-item">
				<input type="checkbox" bind:checked={localEnableLogging} onchange={markChanged} />
				<div class="checkbox-content">
					<span class="checkbox-label">{$i18n('validation_audit_enable_label')}</span>
					<span class="checkbox-description">{$i18n('validation_audit_enable_desc')}</span>
				</div>
			</label>

			<label class="slider-row" class:disabled={!localEnableLogging}>
				<span class="slider-label">
					{$i18n('validation_audit_retention_label')}
					<span class="slider-value">
						{localRetentionDays}
						{$i18n('validation_audit_retention_days_unit')}
					</span>
				</span>
				<input
					type="range"
					min={RETENTION_MIN}
					max={RETENTION_MAX}
					step="1"
					bind:value={localRetentionDays}
					disabled={!localEnableLogging}
					oninput={() => {
						localRetentionDays = clampRetention(localRetentionDays);
						markChanged();
					}}
				/>
				<span class="slider-bounds">
					<span>{RETENTION_MIN}</span><span>{RETENTION_MAX}</span>
				</span>
			</label>

			<div class="audit-actions">
				<Button
					variant="secondary"
					onclick={handlePurgeNow}
					disabled={purging || !localEnableLogging}
				>
					{purging ? $i18n('validation_audit_purging') : $i18n('validation_audit_purge_button')}
				</Button>
				<a class="audit-link" href="/settings/audit-log">
					{$i18n('validation_audit_view_log_link')}
				</a>
			</div>
		</div>

		<!-- Actions -->
		<div class="settings-actions">
			<Button variant="secondary" onclick={handleReset} disabled={$isSaving}>
				{$i18n('validation_reset_button')}
			</Button>
			<Button variant="primary" onclick={handleSave} disabled={$isSaving || !hasChanges}>
				{$isSaving ? $i18n('validation_saving') : $i18n('validation_save_changes')}
			</Button>
		</div>
	{/if}
</div>

<style>
	/* Timeout slider + audit section styling */
	.slider-row {
		display: grid;
		grid-template-columns: 1fr;
		gap: var(--spacing-xs);
	}
	.slider-row.disabled {
		opacity: 0.55;
	}
	.slider-label {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		font-weight: 500;
	}
	.slider-value {
		font-variant-numeric: tabular-nums;
		color: var(--color-primary);
	}
	.slider-bounds {
		display: flex;
		justify-content: space-between;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}
	.radio-group {
		border: none;
		padding: 0;
		margin: 0;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}
	.radio-group-legend {
		font-weight: 500;
		margin-bottom: var(--spacing-xs);
	}
	.radio-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-radius: var(--radius-sm);
		cursor: pointer;
	}
	.radio-item:hover {
		background: var(--color-bg-hover);
	}
	.audit-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		margin-top: var(--spacing-sm);
	}
	.audit-link {
		color: var(--color-primary);
		text-decoration: none;
		font-size: 0.875rem;
	}
	.audit-link:hover {
		text-decoration: underline;
	}

	.validation-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xl);
	}

	.loading-state {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
		color: var(--color-text-secondary);
	}

	.spinner {
		width: 20px;
		height: 20px;
		border: 2px solid var(--color-border);
		border-top-color: var(--color-primary);
		border-radius: 50%;
		animation: spin 0.8s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.settings-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.section-title {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.section-help {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	/* Card Selector */
	.card-selector {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--spacing-md);
	}

	@media (max-width: 768px) {
		.card-selector {
			grid-template-columns: 1fr;
		}
	}

	.selector-card {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition:
			border-color var(--transition-fast),
			background-color var(--transition-fast);
		text-align: left;
	}

	.selector-card:hover {
		border-color: var(--color-primary);
		background: var(--color-bg-hover);
	}

	.selector-card.selected {
		border-color: var(--color-primary);
		background: color-mix(in srgb, var(--color-primary) 10%, transparent);
	}

	.selector-card-title {
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.selector-card-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	/* Mode Banners */
	.mode-banner {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		margin-top: var(--spacing-sm);
	}

	.mode-banner.warning {
		background: color-mix(in srgb, var(--color-warning) 10%, transparent);
		border: 1px solid var(--color-warning);
	}

	.mode-banner.info {
		background: color-mix(in srgb, var(--color-primary) 10%, transparent);
		border: 1px solid var(--color-primary);
	}

	.mode-banner-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 20px;
		height: 20px;
		border-radius: var(--border-radius-full);
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-semibold);
		flex-shrink: 0;
	}

	.mode-banner.warning .mode-banner-icon {
		background: var(--color-warning);
		color: white;
	}

	.mode-banner.info .mode-banner-icon {
		background: var(--color-primary);
		color: white;
	}

	.mode-banner-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.mode-banner-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.mode-banner-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	/* Checkbox Group */
	.checkbox-group {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.checkbox-item {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-md);
		cursor: pointer;
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-md);
		transition: background-color var(--transition-fast);
	}

	.checkbox-item:hover {
		background: var(--color-bg-hover);
	}

	.checkbox-item input[type='checkbox'] {
		width: 18px;
		height: 18px;
		accent-color: var(--color-primary);
		cursor: pointer;
		margin-top: 2px;
		flex-shrink: 0;
	}

	.checkbox-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.checkbox-label {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.checkbox-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.checkbox-description.warning {
		color: var(--color-warning);
	}

	/* Info Cards container (for Auto/Manual modes) */
	.info-cards {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	/* Item list (tools, MCP servers) - used by snippets rendered in this component */
	.item-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
		margin-top: var(--spacing-xs);
	}

	.item-badge {
		display: inline-flex;
		align-items: center;
		gap: 4px;
		padding: 2px 8px;
		font-size: var(--font-size-xs);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		color: var(--color-text-secondary);
	}

	.item-badge.approved {
		background: color-mix(in srgb, var(--color-success) 15%, transparent);
		color: var(--color-success);
	}

	.item-badge.validation-required {
		background: color-mix(in srgb, var(--color-warning) 15%, transparent);
		color: var(--color-warning);
	}

	.item-badge.enabled {
		background: color-mix(in srgb, var(--color-primary) 15%, transparent);
		color: var(--color-primary);
	}

	.item-badge.running {
		background: color-mix(in srgb, var(--color-success) 15%, transparent);
		color: var(--color-success);
	}

	.status-dot {
		width: 6px;
		height: 6px;
		border-radius: 50%;
	}

	.status-dot.running {
		background: var(--color-success);
	}

	.status-dot.stopped {
		background: var(--color-text-tertiary);
	}

	.loading-text,
	.no-items {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		font-style: italic;
	}

	/* Actions */
	.settings-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding-top: var(--spacing-lg);
		border-top: 1px solid var(--color-border);
	}
</style>
