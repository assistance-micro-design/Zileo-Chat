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

	 * Functional options:
	 * - Mode (Auto/Manual/Selective)
	 * - Selective: Sub-Agent operations, Tools, MCP servers
	 * - Risk Thresholds (autoApproveLow, alwaysConfirmHigh)
	 *
	 * Everything is edited inside a single card whose sticky save bar carries
	 * the explicit Reset / Save actions plus the unsaved-changes hint.
	 */
	import { onMount } from 'svelte';
	import { tauriInvoke } from '$lib/tauri';
	import { Button, Card, ErrorBanner, Input, Select, Switch } from '$lib/components/ui';
	import { ExternalLink, Info, Trash2, TriangleAlert } from '@lucide/svelte';
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
			dispatchSettingsRefresh({ source: 'validation' });
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

	// Timeout behavior options with localized labels for the native select
	const behaviorSelectOptions = $derived(
		timeoutBehaviorOptions.map((opt) => ({ value: opt.value, label: $i18n(opt.labelKey) }))
	);

	// Auto/Manual overview display (null in selective mode). Derived in the
	// script because {@const} would not re-evaluate when the mode flips between
	// auto and manual without leaving the {#if} block.
	const modeDisplay = $derived(
		localMode === 'auto' || localMode === 'manual' ? getAutoManualModeDisplay(localMode) : null
	);

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

	/** Parses a number input, keeping the previous value on a non-numeric entry. */
	function parseIntOr(raw: string, fallback: number): number {
		const parsed = Number.parseInt(raw, 10);
		return Number.isNaN(parsed) ? fallback : parsed;
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

<!-- Shared snippet: renders a list of tool badges (auto/manual mode overview) -->
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
		<Card>
			{#snippet body()}
				<div class="validation-form">
					<!-- Mode Selector -->
					<section class="form-section">
						<h4 class="group-title">{$i18n('validation_mode_title')}</h4>
						<div class="mode-grid" role="group" aria-label={$i18n('validation_mode_title')}>
							{#each modeOptions as option (option.value)}
								<button
									type="button"
									class="mode-card"
									class:selected={localMode === option.value}
									onclick={() => selectMode(option.value)}
								>
									<strong>{$i18n(option.labelKey)}</strong>
									<span>{$i18n(option.descKey)}</span>
								</button>
							{/each}
						</div>
						{#if localMode === 'auto'}
							<div class="mode-banner warning" role="note">
								<TriangleAlert size={18} aria-hidden="true" />
								<div class="mode-banner-content">
									<strong>{$i18n('validation_auto_multi_workflow_title')}</strong>
									<span>{$i18n('validation_auto_multi_workflow_desc')}</span>
								</div>
							</div>
						{/if}
						{#if localMode === 'manual' || localMode === 'selective'}
							<div class="mode-banner info" role="note">
								<Info size={18} aria-hidden="true" />
								<div class="mode-banner-content">
									<strong>{$i18n('validation_single_workflow_title')}</strong>
									<span>{$i18n('validation_single_workflow_desc')}</span>
								</div>
							</div>
						{/if}

						<!-- Selective sub-options: dashed enclosure, checkbox on the right -->
						{#if localMode === 'selective'}
							<div class="selective-box">
								<label class="toggle-row">
									<span class="toggle-text">
										<strong>{$i18n('validation_sub_agents')}</strong>
										<span>{$i18n('validation_sub_agents_desc')}</span>
									</span>
									<input
										type="checkbox"
										class="form-checkbox"
										bind:checked={localSubAgentsValidation}
										onchange={markChanged}
									/>
								</label>
								<label class="toggle-row">
									<span class="toggle-text">
										<strong>{$i18n('validation_tools')}</strong>
										<span>{$i18n('validation_tools_desc')}</span>
									</span>
									<input
										type="checkbox"
										class="form-checkbox"
										bind:checked={localToolsValidation}
										onchange={markChanged}
									/>
								</label>
								<label class="toggle-row">
									<span class="toggle-text">
										<strong>{$i18n('validation_mcp')}</strong>
										<span>{$i18n('validation_mcp_desc')}</span>
									</span>
									<input
										type="checkbox"
										class="form-checkbox"
										bind:checked={localMcpValidation}
										onchange={markChanged}
									/>
								</label>
							</div>
						{/if}
					</section>

					<!-- Auto/Manual Mode Information (merged - identical structure, different variant) -->
					{#if modeDisplay}
						<section class="form-section">
							<h4 class="group-title">{$i18n(modeDisplay.sectionTitleKey)}</h4>
							<p class="section-help">{$i18n(modeDisplay.sectionHelpKey)}</p>

							<div class="info-cards">
								<ValidationInfoCard
									variant={modeDisplay.variant}
									icon={modeDisplay.icon}
									titleKey="validation_sub_agents"
									statusKey={modeDisplay.statusKey}
								>
									{@render toolBadgeList(subAgentTools, modeDisplay.variant)}
								</ValidationInfoCard>

								<ValidationInfoCard
									variant={modeDisplay.variant}
									icon={modeDisplay.icon}
									titleKey="validation_tools"
									statusKey={modeDisplay.statusKey}
								>
									{@render toolBadgeList(basicTools, modeDisplay.variant)}
								</ValidationInfoCard>

								<ValidationInfoCard
									variant={modeDisplay.variant}
									icon={modeDisplay.icon}
									titleKey="validation_mcp"
									statusKey={modeDisplay.statusKey}
								>
									{@render mcpBadgeList(modeDisplay.variant)}
								</ValidationInfoCard>
							</div>
						</section>
					{/if}

					<!-- Risk Thresholds -->
					<section class="form-section">
						<h4 class="group-title">{$i18n('validation_risk_title')}</h4>
						<div class="toggle-stack">
							<div class="toggle-row">
								<span class="toggle-text">
									<strong id="validation-risk-low">
										{$i18n('validation_risk_auto_approve_low')}
									</strong>
									<span>{$i18n('validation_risk_auto_approve_low_desc')}</span>
								</span>
								<Switch
									checked={localRiskThresholds.autoApproveLow}
									onchange={(v) => {
										localRiskThresholds.autoApproveLow = v;
										markChanged();
									}}
									labelledBy="validation-risk-low"
								/>
							</div>
							<div class="toggle-row">
								<span class="toggle-text">
									<strong id="validation-risk-high">
										{$i18n('validation_risk_always_confirm_high')}
									</strong>
									<span class="warning">{$i18n('validation_risk_always_confirm_high_desc')}</span>
								</span>
								<Switch
									checked={localRiskThresholds.alwaysConfirmHigh}
									onchange={(v) => {
										localRiskThresholds.alwaysConfirmHigh = v;
										markChanged();
									}}
									labelledBy="validation-risk-high"
								/>
							</div>
						</div>
					</section>

					<!-- Timeout Settings -->
					<section class="form-section">
						<h4 class="group-title">{$i18n('validation_timeout_title')}</h4>
						<div class="field-grid">
							<div class="timeout-field">
								<Input
									type="number"
									label={$i18n('validation_timeout_seconds_label')}
									value={String(localTimeoutSeconds)}
									min={TIMEOUT_MIN}
									max={TIMEOUT_MAX}
									step={5}
									help={$i18n('validation_timeout_range', {
										min: TIMEOUT_MIN,
										max: TIMEOUT_MAX
									})}
									oninput={(e) => {
										localTimeoutSeconds = parseIntOr(e.currentTarget.value, localTimeoutSeconds);
										markChanged();
									}}
									onblur={() => {
										localTimeoutSeconds = clampTimeout(localTimeoutSeconds);
									}}
								/>
							</div>
							<Select
								label={$i18n('validation_timeout_behavior_label')}
								options={behaviorSelectOptions}
								value={localTimeoutBehavior}
								onchange={(e) => {
									localTimeoutBehavior = e.currentTarget.value as TimeoutBehavior;
									markChanged();
								}}
							/>
						</div>
					</section>

					<!-- Audit Logging -->
					<section class="form-section">
						<h4 class="group-title">{$i18n('validation_audit_title')}</h4>
						<div class="toggle-row">
							<span class="toggle-text">
								<strong id="validation-audit-logging"
									>{$i18n('validation_audit_enable_label')}</strong
								>
								<span>{$i18n('validation_audit_enable_desc')}</span>
							</span>
							<Switch
								checked={localEnableLogging}
								onchange={(v) => {
									localEnableLogging = v;
									markChanged();
								}}
								labelledBy="validation-audit-logging"
							/>
						</div>
						<div class="audit-row" class:disabled={!localEnableLogging}>
							<div class="retention-field">
								<Input
									type="number"
									label={$i18n('validation_audit_retention_label')}
									value={String(localRetentionDays)}
									min={RETENTION_MIN}
									max={RETENTION_MAX}
									step={1}
									disabled={!localEnableLogging}
									help={$i18n('validation_audit_retention_range', {
										min: RETENTION_MIN,
										max: RETENTION_MAX
									})}
									oninput={(e) => {
										localRetentionDays = parseIntOr(e.currentTarget.value, localRetentionDays);
										markChanged();
									}}
									onblur={() => {
										localRetentionDays = clampRetention(localRetentionDays);
									}}
								/>
							</div>
							<div class="audit-buttons">
								<Button
									variant="outline"
									size="sm"
									onclick={handlePurgeNow}
									disabled={purging || !localEnableLogging}
								>
									<Trash2 size={14} aria-hidden="true" />
									<span>
										{purging
											? $i18n('validation_audit_purging')
											: $i18n('validation_audit_purge_button')}
									</span>
								</Button>
								<a class="btn btn-ghost btn-sm" href="/settings/audit-log">
									{$i18n('validation_audit_view_log_link')}
									<ExternalLink size={14} aria-hidden="true" />
								</a>
							</div>
						</div>
					</section>

					<!-- Sticky save bar: stays visible while the long form scrolls beneath
					     it. Opaque card surface, no backdrop blur: a blurred sticky bar
					     forces WebKitGTK to re-blur the content scrolling behind it on
					     every frame. -->
					<div class="form-actions">
						{#if hasChanges && !$isSaving}
							<span class="dirty-hint" role="status">
								<TriangleAlert size={14} aria-hidden="true" />
								{$i18n('settings_unsaved_changes')}
							</span>
						{/if}
						<Button variant="ghost" onclick={handleReset} disabled={$isSaving}>
							{$i18n('validation_reset_button')}
						</Button>
						<Button variant="primary" onclick={handleSave} disabled={$isSaving || !hasChanges}>
							{$isSaving ? $i18n('validation_saving') : $i18n('validation_save_changes')}
						</Button>
					</div>
				</div>
			{/snippet}
		</Card>
	{/if}
</div>

<style>
	.validation-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.validation-form {
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

	.form-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.group-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-accent-deep);
		margin: 0;
	}

	.section-help {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	/* Mode cards */
	.mode-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--spacing-md);
	}

	@media (max-width: 768px) {
		.mode-grid {
			grid-template-columns: 1fr;
		}
	}

	.mode-card {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		padding: var(--spacing-md);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-lg);
		cursor: pointer;
		transition:
			border-color var(--transition-fast),
			box-shadow var(--transition-fast);
		text-align: left;
	}

	.mode-card:hover {
		border-color: var(--color-accent-deep);
	}

	.mode-card.selected {
		border-color: var(--color-accent-deep);
		box-shadow: var(--glow-accent-soft);
		background: var(--color-accent-light);
	}

	.mode-card strong {
		display: block;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
		margin-bottom: 2px;
	}

	.mode-card span {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	/* Mode banners */
	.mode-banner {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
	}

	.mode-banner :global(svg) {
		flex-shrink: 0;
	}

	.mode-banner.warning {
		background: var(--color-warning-light);
		border: 1px solid color-mix(in srgb, var(--color-warning) 35%, transparent);
		color: var(--color-warning);
	}

	.mode-banner.info {
		background: var(--color-info-light);
		border: 1px solid color-mix(in srgb, var(--color-info) 35%, transparent);
		color: var(--color-info);
	}

	.mode-banner-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.mode-banner-content strong {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.mode-banner-content span {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	/* Selective sub-options enclosure */
	.selective-box {
		display: flex;
		flex-direction: column;
		border: 1px dashed var(--color-border);
		border-radius: var(--border-radius-md);
		padding: 0 var(--spacing-md);
	}

	/* Toggle rows: text block on the left, control on the right */
	.toggle-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
		padding: var(--spacing-md) 0;
	}

	.toggle-row + .toggle-row {
		border-top: 1px solid var(--color-border-light);
	}

	label.toggle-row {
		cursor: pointer;
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

	.toggle-text span.warning {
		color: var(--color-warning);
	}

	.toggle-stack {
		display: flex;
		flex-direction: column;
	}

	.toggle-stack .toggle-row,
	.form-section > .toggle-row {
		padding: var(--spacing-sm) 0;
	}

	/* Timeout fields */
	.field-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: var(--spacing-md);
	}

	@media (max-width: 768px) {
		.field-grid {
			grid-template-columns: 1fr;
		}
	}

	.timeout-field :global(input) {
		width: 140px;
	}

	/* Audit retention + actions row */
	.audit-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		flex-wrap: wrap;
	}

	.audit-row.disabled {
		opacity: 0.55;
	}

	.retention-field :global(input) {
		width: 110px;
	}

	.audit-buttons {
		margin-left: auto;
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.audit-buttons :global(button) {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.audit-buttons a {
		gap: var(--spacing-xs);
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

	/* Sticky save bar: stays visible while the long form scrolls beneath it. */
	.form-actions {
		position: sticky;
		bottom: 0;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding: var(--spacing-md) 0;
		border-top: 1px solid var(--color-border);
		background: var(--surface-1);
	}

	.dirty-hint {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		margin-right: auto;
		font-size: var(--font-size-xs);
		color: var(--color-warning);
	}
</style>
