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
  import { invoke } from '@tauri-apps/api/core';
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
  import type { ToastType } from '$types/background-workflow';
  import ValidationInfoCard from './ValidationInfoCard.svelte';
  import type {
    ValidationMode,
    UpdateValidationSettingsRequest,
    AvailableToolInfo
  } from '$types/validation';
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

  // Available tools and MCP servers
  let availableTools = $state<AvailableToolInfo[]>([]);
  let mcpServers = $state<MCPServer[]>([]);
  let loadingResources = $state(false);

  // UI state
  let errorMessage = $state<string | null>(null);
  let hasChanges = $state(false);

  // Mode options for card selector (using translation keys)
  const modeOptions: Array<{ value: ValidationMode; labelKey: string; descKey: string }> = [
    {
      value: 'auto',
      labelKey: 'validation_mode_auto',
      descKey: 'validation_mode_auto_desc'
    },
    {
      value: 'manual',
      labelKey: 'validation_mode_manual',
      descKey: 'validation_mode_manual_desc'
    },
    {
      value: 'selective',
      labelKey: 'validation_mode_selective',
      descKey: 'validation_mode_selective_desc'
    }
  ];

  // Derived: basic tools (local tools that don't require context)
  let basicTools = $derived(availableTools.filter(t => t.category === 'basic'));

  // Derived: sub-agent tools
  let subAgentTools = $derived(availableTools.filter(t => t.category === 'sub_agent'));

  // Load settings and resources on mount
  onMount(async () => {
    try {
      await Promise.all([
        validationSettingsStore.loadSettings(),
        loadAvailableResources()
      ]);
    } catch (err) {
      errorMessage = $i18n('validation_load_resources_failed').replace('{error}', getErrorMessage(err));
    }
  });

  // Load available tools and MCP servers
  async function loadAvailableResources(): Promise<void> {
    loadingResources = true;
    try {
      const [tools, servers] = await Promise.all([
        invoke<AvailableToolInfo[]>('list_available_tools'),
        loadServers(true) // Force refresh
      ]);
      availableTools = tools;
      mcpServers = servers;
    } catch (err) {
      errorMessage = $i18n('validation_load_resources_failed').replace('{error}', getErrorMessage(err));
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
      const updateRequest: UpdateValidationSettingsRequest = {
        mode: localMode,
        selectiveConfig: {
          subAgents: localSubAgentsValidation,
          tools: localToolsValidation,
          mcp: localMcpValidation,
          fileOps: false,
          dbOps: false
        },
        riskThresholds: localRiskThresholds
      };
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
      {@const variant = localMode === 'auto' ? 'approved' : 'validation-required'}
      {@const icon = localMode === 'auto' ? '\u2713' : '\u26A0'}
      {@const statusKey = localMode === 'auto' ? 'validation_auto_approved' : 'validation_requires_approval'}
      {@const sectionTitleKey = localMode === 'auto' ? 'validation_auto_title' : 'validation_manual_title'}
      {@const sectionHelpKey = localMode === 'auto' ? 'validation_auto_help' : 'validation_manual_help'}

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
            <input
              type="checkbox"
              bind:checked={localSubAgentsValidation}
              onchange={markChanged}
            />
            <div class="checkbox-content">
              <span class="checkbox-label">{$i18n('validation_sub_agents')}</span>
              <span class="checkbox-description">{$i18n('validation_sub_agents_desc')}</span>
              {@render toolBadgeList(subAgentTools, localSubAgentsValidation ? 'enabled' : '')}
            </div>
          </label>

          <!-- Tools Validation -->
          <label class="checkbox-item">
            <input
              type="checkbox"
              bind:checked={localToolsValidation}
              onchange={markChanged}
            />
            <div class="checkbox-content">
              <span class="checkbox-label">{$i18n('validation_tools')}</span>
              <span class="checkbox-description">{$i18n('validation_tools_desc')}</span>
              {@render toolBadgeList(basicTools, localToolsValidation ? 'enabled' : '')}
            </div>
          </label>

          <!-- MCP Servers Validation -->
          <label class="checkbox-item">
            <input
              type="checkbox"
              bind:checked={localMcpValidation}
              onchange={markChanged}
            />
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
            <span class="checkbox-description">{$i18n('validation_risk_auto_approve_low_desc')}</span>
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
            <span class="checkbox-description warning">{$i18n('validation_risk_always_confirm_high_desc')}</span>
          </div>
        </label>
      </div>
    </div>

    <!-- Actions -->
    <div class="settings-actions">
      <Button
        variant="secondary"
        onclick={handleReset}
        disabled={$isSaving}
      >
        {$i18n('validation_reset_button')}
      </Button>
      <Button
        variant="primary"
        onclick={handleSave}
        disabled={$isSaving || !hasChanges}
      >
        {$isSaving ? $i18n('validation_saving') : $i18n('validation_save_changes')}
      </Button>
    </div>
  {/if}
</div>

<style>
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
    to { transform: rotate(360deg); }
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
    transition: border-color var(--transition-fast), background-color var(--transition-fast);
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

  .checkbox-item input[type="checkbox"] {
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

  .loading-text, .no-items {
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
