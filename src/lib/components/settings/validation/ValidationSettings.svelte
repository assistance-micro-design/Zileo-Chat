<script lang="ts">
  /**
   * ValidationSettings component
   * Manages global validation settings configuration
   *
   * Currently functional options:
   * - Mode (Auto/Manual/Selective)
   * - Selective: Sub-Agent operations only
   * - Risk Thresholds (autoApproveLow, alwaysConfirmHigh)
   */
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui';
  import {
    validationSettingsStore,
    settings,
    isLoading,
    isSaving
  } from '$lib/stores/validation-settings';
  import type {
    ValidationMode,
    UpdateValidationSettingsRequest
  } from '$types/validation';

  // Local form state (copied from store on load)
  let localMode = $state<ValidationMode>('selective');
  let localSubAgentsValidation = $state(true);
  let localRiskThresholds = $state({
    autoApproveLow: true,
    alwaysConfirmHigh: false
  });

  // UI state
  let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
  let hasChanges = $state(false);

  // Mode options for card selector
  const modeOptions: Array<{ value: ValidationMode; label: string; description: string }> = [
    {
      value: 'auto',
      label: 'Auto',
      description: 'Execute all operations without confirmation'
    },
    {
      value: 'manual',
      label: 'Manual',
      description: 'Request confirmation for every operation'
    },
    {
      value: 'selective',
      label: 'Selective',
      description: 'Choose which operations require confirmation'
    }
  ];

  // Load settings on mount
  onMount(async () => {
    await validationSettingsStore.loadSettings();
  });

  // Sync local state when store settings change
  $effect(() => {
    const s = $settings;
    if (s) {
      localMode = s.mode;
      localSubAgentsValidation = s.selectiveConfig.subAgents;
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
          // Keep other fields at default (not functional yet)
          tools: false,
          mcp: false,
          fileOps: false,
          dbOps: false
        },
        riskThresholds: localRiskThresholds
      };
      await validationSettingsStore.updateSettings(updateRequest);
      message = { type: 'success', text: 'Settings saved successfully' };
      hasChanges = false;
      setTimeout(() => {
        if (message?.type === 'success') message = null;
      }, 3000);
    } catch (err) {
      message = { type: 'error', text: `Failed to save: ${err}` };
    }
  }

  // Handle reset to defaults
  async function handleReset(): Promise<void> {
    message = null;
    try {
      await validationSettingsStore.resetToDefaults();
      message = { type: 'success', text: 'Settings reset to defaults' };
      hasChanges = false;
      setTimeout(() => {
        if (message?.type === 'success') message = null;
      }, 3000);
    } catch (err) {
      message = { type: 'error', text: `Failed to reset: ${err}` };
    }
  }
</script>

<div class="validation-settings">
  {#if $isLoading}
    <div class="loading-state">
      <span class="spinner"></span>
      <span>Loading settings...</span>
    </div>
  {:else}
    <!-- Mode Selector -->
    <div class="settings-section">
      <h3 class="section-title">Validation Mode</h3>
      <div class="card-selector" role="group" aria-label="Validation mode">
        {#each modeOptions as option}
          <button
            type="button"
            class="selector-card"
            class:selected={localMode === option.value}
            onclick={() => selectMode(option.value)}
          >
            <span class="selector-card-title">{option.label}</span>
            <span class="selector-card-description">{option.description}</span>
          </button>
        {/each}
      </div>
    </div>

    <!-- Selective Configuration (conditional) -->
    {#if localMode === 'selective'}
      <div class="settings-section">
        <h3 class="section-title">Selective Configuration</h3>
        <p class="section-help">Choose which operations require validation</p>
        <div class="checkbox-group">
          <label class="checkbox-item">
            <input
              type="checkbox"
              bind:checked={localSubAgentsValidation}
              onchange={markChanged}
            />
            <div class="checkbox-content">
              <span class="checkbox-label">Sub-Agent Operations</span>
              <span class="checkbox-description">Validate spawn, delegate, and parallel execution of sub-agents</span>
            </div>
          </label>
        </div>
      </div>
    {/if}

    <!-- Risk Thresholds -->
    <div class="settings-section">
      <h3 class="section-title">Risk Thresholds</h3>
      <div class="checkbox-group">
        <label class="checkbox-item">
          <input
            type="checkbox"
            bind:checked={localRiskThresholds.autoApproveLow}
            onchange={markChanged}
          />
          <div class="checkbox-content">
            <span class="checkbox-label">Auto-approve LOW risk operations</span>
            <span class="checkbox-description">Skip validation for low-risk operations</span>
          </div>
        </label>
        <label class="checkbox-item">
          <input
            type="checkbox"
            bind:checked={localRiskThresholds.alwaysConfirmHigh}
            onchange={markChanged}
          />
          <div class="checkbox-content">
            <span class="checkbox-label">Always confirm HIGH risk operations</span>
            <span class="checkbox-description warning">Require validation even in Auto mode</span>
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
        Reset to Defaults
      </Button>
      <Button
        variant="primary"
        onclick={handleSave}
        disabled={$isSaving || !hasChanges}
      >
        {$isSaving ? 'Saving...' : 'Save Changes'}
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
