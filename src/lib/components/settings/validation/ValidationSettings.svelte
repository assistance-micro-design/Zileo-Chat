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
  import { Button } from '$lib/components/ui';
  import { i18n } from '$lib/i18n';
  import {
    validationSettingsStore,
    settings,
    isLoading,
    isSaving
  } from '$lib/stores/validation-settings';
  import { loadServers } from '$lib/stores/mcp';
  import type {
    ValidationMode,
    UpdateValidationSettingsRequest,
    AvailableToolInfo
  } from '$types/validation';
  import type { MCPServer } from '$types/mcp';

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
  let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
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

  // Note: runningMcpServers could be used for filtering display if needed
  // Currently we show all servers with status indicator

  // Load settings and resources on mount
  onMount(async () => {
    await Promise.all([
      validationSettingsStore.loadSettings(),
      loadAvailableResources()
    ]);
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
      console.error('Failed to load resources:', err);
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
    if (message?.type === 'success') {
      message = null;
    }
  }

  // Handle mode selection
  function selectMode(mode: ValidationMode): void {
    localMode = mode;
    markChanged();
  }

  // Handle save
  async function handleSave(): Promise<void> {
    message = null;
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
      message = { type: 'success', text: $i18n('validation_saved') };
      hasChanges = false;
      setTimeout(() => {
        if (message?.type === 'success') message = null;
      }, 3000);
    } catch (err) {
      message = { type: 'error', text: $i18n('validation_save_failed').replace('{error}', String(err)) };
    }
  }

  // Handle reset to defaults
  async function handleReset(): Promise<void> {
    message = null;
    try {
      await validationSettingsStore.resetToDefaults();
      message = { type: 'success', text: $i18n('validation_reset_success') };
      hasChanges = false;
      setTimeout(() => {
        if (message?.type === 'success') message = null;
      }, 3000);
    } catch (err) {
      message = { type: 'error', text: $i18n('validation_reset_failed').replace('{error}', String(err)) };
    }
  }
</script>

<div class="validation-settings">
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
    </div>

    <!-- Auto Mode Information -->
    {#if localMode === 'auto'}
      <div class="settings-section">
        <h3 class="section-title">{$i18n('validation_auto_title')}</h3>
        <p class="section-help">{$i18n('validation_auto_help')}</p>

        <div class="info-cards">
          <!-- Sub-Agents (auto-approved) -->
          <div class="info-card approved">
            <div class="info-card-header">
              <span class="info-card-icon">✓</span>
              <span class="info-card-title">{$i18n('validation_sub_agents')}</span>
            </div>
            <span class="info-card-status">{$i18n('validation_auto_approved')}</span>
            {#if subAgentTools.length > 0}
              <div class="item-list">
                {#each subAgentTools as tool (tool.name)}
                  <span class="item-badge approved">{tool.name}</span>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Local Tools (auto-approved) -->
          <div class="info-card approved">
            <div class="info-card-header">
              <span class="info-card-icon">✓</span>
              <span class="info-card-title">{$i18n('validation_tools')}</span>
            </div>
            <span class="info-card-status">{$i18n('validation_auto_approved')}</span>
            {#if basicTools.length > 0}
              <div class="item-list">
                {#each basicTools as tool (tool.name)}
                  <span class="item-badge approved">{tool.name}</span>
                {/each}
              </div>
            {/if}
          </div>

          <!-- MCP Servers (auto-approved) -->
          <div class="info-card approved">
            <div class="info-card-header">
              <span class="info-card-icon">✓</span>
              <span class="info-card-title">{$i18n('validation_mcp')}</span>
            </div>
            <span class="info-card-status">{$i18n('validation_auto_approved')}</span>
            {#if loadingResources}
              <span class="loading-text">{$i18n('common_loading')}</span>
            {:else if mcpServers.length > 0}
              <div class="item-list">
                {#each mcpServers as server (server.name)}
                  <span class="item-badge approved" class:running={server.status === 'running'}>
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
          </div>
        </div>
      </div>
    {/if}

    <!-- Manual Mode Information -->
    {#if localMode === 'manual'}
      <div class="settings-section">
        <h3 class="section-title">{$i18n('validation_manual_title')}</h3>
        <p class="section-help">{$i18n('validation_manual_help')}</p>

        <div class="info-cards">
          <!-- Sub-Agents (requires validation) -->
          <div class="info-card validation-required">
            <div class="info-card-header">
              <span class="info-card-icon">⚠</span>
              <span class="info-card-title">{$i18n('validation_sub_agents')}</span>
            </div>
            <span class="info-card-status">{$i18n('validation_requires_approval')}</span>
            {#if subAgentTools.length > 0}
              <div class="item-list">
                {#each subAgentTools as tool (tool.name)}
                  <span class="item-badge validation-required">{tool.name}</span>
                {/each}
              </div>
            {/if}
          </div>

          <!-- Local Tools (requires validation) -->
          <div class="info-card validation-required">
            <div class="info-card-header">
              <span class="info-card-icon">⚠</span>
              <span class="info-card-title">{$i18n('validation_tools')}</span>
            </div>
            <span class="info-card-status">{$i18n('validation_requires_approval')}</span>
            {#if basicTools.length > 0}
              <div class="item-list">
                {#each basicTools as tool (tool.name)}
                  <span class="item-badge validation-required">{tool.name}</span>
                {/each}
              </div>
            {/if}
          </div>

          <!-- MCP Servers (requires validation) -->
          <div class="info-card validation-required">
            <div class="info-card-header">
              <span class="info-card-icon">⚠</span>
              <span class="info-card-title">{$i18n('validation_mcp')}</span>
            </div>
            <span class="info-card-status">{$i18n('validation_requires_approval')}</span>
            {#if loadingResources}
              <span class="loading-text">{$i18n('common_loading')}</span>
            {:else if mcpServers.length > 0}
              <div class="item-list">
                {#each mcpServers as server (server.name)}
                  <span class="item-badge validation-required" class:running={server.status === 'running'}>
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
          </div>
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
              {#if subAgentTools.length > 0}
                <div class="item-list">
                  {#each subAgentTools as tool (tool.name)}
                    <span class="item-badge" class:enabled={localSubAgentsValidation}>{tool.name}</span>
                  {/each}
                </div>
              {/if}
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
              {#if basicTools.length > 0}
                <div class="item-list">
                  {#each basicTools as tool (tool.name)}
                    <span class="item-badge" class:enabled={localToolsValidation}>{tool.name}</span>
                  {/each}
                </div>
              {/if}
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
              {#if loadingResources}
                <span class="loading-text">{$i18n('common_loading')}</span>
              {:else if mcpServers.length > 0}
                <div class="item-list">
                  {#each mcpServers as server (server.name)}
                    <span class="item-badge" class:enabled={localMcpValidation} class:running={server.status === 'running'}>
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

    <!-- Message -->
    {#if message}
      <div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
        {message.text}
      </div>
    {/if}

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
    font-size: var(--font-size-md);
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

  /* Info Cards (for Auto/Manual modes) */
  .info-cards {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
  }

  .info-card {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
    padding: var(--spacing-md);
    border-radius: var(--border-radius-md);
    border: 1px solid var(--color-border);
    background: var(--color-bg-secondary);
  }

  .info-card.approved {
    border-color: var(--color-success);
    background: color-mix(in srgb, var(--color-success) 5%, var(--color-bg-secondary));
  }

  .info-card.validation-required {
    border-color: var(--color-warning);
    background: color-mix(in srgb, var(--color-warning) 5%, var(--color-bg-secondary));
  }

  .info-card-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
  }

  .info-card-icon {
    font-size: var(--font-size-md);
  }

  .info-card.approved .info-card-icon {
    color: var(--color-success);
  }

  .info-card.validation-required .info-card-icon {
    color: var(--color-warning);
  }

  .info-card-title {
    font-weight: var(--font-weight-semibold);
    color: var(--color-text-primary);
  }

  .info-card-status {
    font-size: var(--font-size-sm);
    color: var(--color-text-secondary);
  }

  /* Item list (tools, MCP servers) */
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

  /* Message */
  .message {
    padding: var(--spacing-md);
    border-radius: var(--border-radius-md);
    font-size: var(--font-size-sm);
  }

  .message.success {
    background: color-mix(in srgb, var(--color-success) 15%, transparent);
    color: var(--color-success);
    border: 1px solid var(--color-success);
  }

  .message.error {
    background: color-mix(in srgb, var(--color-error) 15%, transparent);
    color: var(--color-error);
    border: 1px solid var(--color-error);
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
