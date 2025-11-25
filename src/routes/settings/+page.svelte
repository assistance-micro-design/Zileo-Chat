<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import type { LLMProvider } from '$types/security';

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

  /** Navigation sections */
  const sections = [
    { id: 'providers', label: 'Providers', icon: 'globe' },
    { id: 'models', label: 'Models', icon: 'cpu' },
    { id: 'theme', label: 'Theme', icon: 'palette' }
  ] as const;

  /**
   * Checks if the current provider has a stored API key
   */
  async function checkApiKeyStatus() {
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
  async function saveApiKey() {
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
  async function deleteApiKey() {
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
  function scrollToSection(sectionId: string) {
    activeSection = sectionId;
    const element = document.getElementById(sectionId);
    if (element) {
      element.scrollIntoView({ behavior: 'smooth', block: 'start' });
    }
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
  <aside class="sidebar">
    <div class="sidebar-header">
      <h2 class="sidebar-title">Settings</h2>
    </div>

    <nav class="sidebar-nav">
      {#each sections as section}
        <button
          type="button"
          class="nav-item"
          class:active={activeSection === section.id}
          onclick={() => scrollToSection(section.id)}
        >
          {#if section.icon === 'globe'}
            <svg class="nav-icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="12" r="10" />
              <path d="M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" />
              <path d="M2 12h20" />
            </svg>
          {:else if section.icon === 'cpu'}
            <svg class="nav-icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect width="16" height="16" x="4" y="4" rx="2" />
              <rect width="6" height="6" x="9" y="9" rx="1" />
              <path d="M15 2v2" />
              <path d="M15 20v2" />
              <path d="M2 15h2" />
              <path d="M2 9h2" />
              <path d="M20 15h2" />
              <path d="M20 9h2" />
              <path d="M9 2v2" />
              <path d="M9 20v2" />
            </svg>
          {:else if section.icon === 'palette'}
            <svg class="nav-icon" xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="13.5" cy="6.5" r=".5" fill="currentColor" />
              <circle cx="17.5" cy="10.5" r=".5" fill="currentColor" />
              <circle cx="8.5" cy="7.5" r=".5" fill="currentColor" />
              <circle cx="6.5" cy="12.5" r=".5" fill="currentColor" />
              <path d="M12 2C6.5 2 2 6.5 2 12s4.5 10 10 10c.926 0 1.648-.746 1.648-1.688 0-.437-.18-.835-.437-1.125-.29-.289-.438-.652-.438-1.125a1.64 1.64 0 0 1 1.668-1.668h1.996c3.051 0 5.555-2.503 5.555-5.555C21.965 6.012 17.461 2 12 2z" />
            </svg>
          {/if}
          <span class="nav-text">{section.label}</span>
        </button>
      {/each}
    </nav>

    <div class="sidebar-footer">
      <div class="security-info">
        <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10" />
          <path d="m9 12 2 2 4-4" />
        </svg>
        <span class="text-xs text-secondary">AES-256 Encrypted</span>
      </div>
    </div>
  </aside>

  <!-- Settings Content -->
  <main class="content-area">
    <!-- Providers Section -->
    <section id="providers" class="settings-section">
      <h2 class="section-title">Providers</h2>

      <div class="grid grid-cols-2 gap-lg">
        <!-- Mistral Provider Card -->
        <div class="card">
          <div class="card-header">
            <div class="flex items-center gap-md flex-1">
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="icon-accent">
                <path d="m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z" />
                <path d="M5 3v4" />
                <path d="M19 17v4" />
                <path d="M3 5h4" />
                <path d="M17 19h4" />
              </svg>
              <div>
                <h3 class="card-title">Mistral</h3>
                <p class="card-description">API Provider</p>
              </div>
            </div>
            <label class="flex items-center gap-sm">
              <input type="checkbox" class="form-checkbox" checked disabled />
              <span class="text-sm text-secondary">Active</span>
            </label>
          </div>
          <div class="card-body">
            <div class="form-group">
              <label class="form-label" for="mistral-api-key">API Key</label>
              <input
                id="mistral-api-key"
                type="password"
                class="form-input"
                placeholder={hasStoredKey && settings.provider === 'Mistral' ? '(key stored securely)' : 'sk-...'}
                bind:value={settings.apiKey}
                disabled={saving || settings.provider !== 'Mistral'}
              />
              <span class="form-help">Your Mistral API key</span>
            </div>
            {#if settings.provider === 'Mistral' && hasStoredKey}
              <div class="status-badge success">
                <span class="status-indicator status-completed"></span>
                <span class="text-sm">Key stored securely</span>
              </div>
            {/if}
          </div>
          <div class="card-footer">
            <button
              class="btn btn-primary btn-sm"
              onclick={() => { settings.provider = 'Mistral'; }}
              disabled={settings.provider === 'Mistral'}
            >
              {settings.provider === 'Mistral' ? 'Selected' : 'Select'}
            </button>
          </div>
        </div>

        <!-- Ollama Provider Card -->
        <div class="card">
          <div class="card-header">
            <div class="flex items-center gap-md flex-1">
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="icon-success">
                <rect width="20" height="8" x="2" y="2" rx="2" ry="2" />
                <rect width="20" height="8" x="2" y="14" rx="2" ry="2" />
                <line x1="6" x2="6.01" y1="6" y2="6" />
                <line x1="6" x2="6.01" y1="18" y2="18" />
              </svg>
              <div>
                <h3 class="card-title">Ollama</h3>
                <p class="card-description">Local Provider</p>
              </div>
            </div>
            <label class="flex items-center gap-sm">
              <input type="checkbox" class="form-checkbox" checked disabled />
              <span class="text-sm text-secondary">Active</span>
            </label>
          </div>
          <div class="card-body">
            <div class="form-group">
              <label class="form-label" for="ollama-endpoint">Endpoint URL</label>
              <input id="ollama-endpoint" type="url" class="form-input" value="http://localhost:11434" disabled />
            </div>
            <div class="status-badge success">
              <span class="status-indicator status-completed"></span>
              <span class="text-sm">No API key required</span>
            </div>
          </div>
          <div class="card-footer">
            <button
              class="btn btn-primary btn-sm"
              onclick={() => { settings.provider = 'Ollama'; }}
              disabled={settings.provider === 'Ollama'}
            >
              {settings.provider === 'Ollama' ? 'Selected' : 'Select'}
            </button>
          </div>
        </div>
      </div>

      <!-- API Key Actions -->
      {#if settings.provider !== 'Ollama'}
        <div class="card mt-lg">
          <div class="card-header">
            <h3 class="card-title">API Key Management</h3>
          </div>
          <div class="card-body">
            <div class="flex gap-md">
              <button
                class="btn btn-primary"
                onclick={saveApiKey}
                disabled={saving || !settings.apiKey.trim()}
              >
                {saving ? 'Saving...' : 'Save API Key'}
              </button>
              {#if hasStoredKey}
                <button
                  class="btn btn-danger"
                  onclick={deleteApiKey}
                  disabled={saving}
                >
                  Delete Stored Key
                </button>
              {/if}
            </div>
            {#if message}
              <div class="message-toast {message.type}">
                {message.text}
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </section>

    <!-- Models Section -->
    <section id="models" class="settings-section">
      <h2 class="section-title">Models</h2>

      <div class="card">
        <div class="card-header">
          <h3 class="card-title">Model Configuration</h3>
        </div>
        <div class="card-body">
          <div class="form-group">
            <label class="form-label" for="model-provider">Provider</label>
            <select id="model-provider" class="form-select" bind:value={settings.provider}>
              <option value="Mistral">Mistral</option>
              <option value="Ollama">Ollama</option>
              <option value="OpenAI">OpenAI</option>
              <option value="Anthropic">Anthropic</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label" for="model-name">Model</label>
            <input id="model-name" type="text" class="form-input" bind:value={settings.model} />
            <span class="form-help">Model identifier (e.g., mistral-large, llama3)</span>
          </div>

          <div class="model-info">
            <h4 class="text-sm font-semibold mb-sm">Selected Model</h4>
            <div class="grid grid-cols-2 gap-md">
              <div>
                <span class="text-sm text-secondary">Provider</span>
                <p class="font-semibold">{settings.provider}</p>
              </div>
              <div>
                <span class="text-sm text-secondary">Model</span>
                <p class="font-semibold">{settings.model}</p>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>

    <!-- Theme Section -->
    <section id="theme" class="settings-section">
      <h2 class="section-title">Theme</h2>

      <div class="grid grid-cols-2 gap-lg">
        <!-- Light Theme Card -->
        <label class="theme-card selected">
          <div class="theme-preview light">
            <div class="theme-header">
              <input type="radio" name="theme" class="form-radio" checked />
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <circle cx="12" cy="12" r="4" />
                <path d="M12 2v2" />
                <path d="M12 20v2" />
                <path d="m4.93 4.93 1.41 1.41" />
                <path d="m17.66 17.66 1.41 1.41" />
                <path d="M2 12h2" />
                <path d="M20 12h2" />
                <path d="m6.34 17.66-1.41 1.41" />
                <path d="m19.07 4.93-1.41 1.41" />
              </svg>
              <div>
                <h3 class="theme-title">Light Mode</h3>
                <p class="theme-description">Bright and clean interface</p>
              </div>
            </div>
            <div class="theme-colors">
              <div class="color-swatch" style="background: #94EFEE;"></div>
              <div class="color-swatch" style="background: #FE7254;"></div>
              <div class="color-swatch" style="background: #ffffff; border: 1px solid #dee2e6;"></div>
            </div>
          </div>
        </label>

        <!-- Dark Theme Card -->
        <label class="theme-card">
          <div class="theme-preview dark">
            <div class="theme-header">
              <input type="radio" name="theme" class="form-radio" />
              <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />
              </svg>
              <div>
                <h3 class="theme-title">Dark Mode</h3>
                <p class="theme-description">Easy on the eyes</p>
              </div>
            </div>
            <div class="theme-colors">
              <div class="color-swatch" style="background: #94EFEE;"></div>
              <div class="color-swatch" style="background: #FE7254;"></div>
              <div class="color-swatch" style="background: #2b2d31;"></div>
            </div>
          </div>
        </label>
      </div>
    </section>

    <!-- Security Info -->
    <section class="settings-section">
      <div class="card">
        <div class="card-header">
          <div class="flex items-center gap-md">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="icon-success">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10" />
              <path d="m9 12 2 2 4-4" />
            </svg>
            <h3 class="card-title">Security Information</h3>
          </div>
        </div>
        <div class="card-body">
          <p class="text-sm text-secondary">
            API keys are stored securely using your operating system's keychain
            (Linux: libsecret, macOS: Keychain, Windows: Credential Manager) with
            additional AES-256 encryption for defense in depth.
          </p>
        </div>
      </div>
    </section>
  </main>
</div>

<style>
  .settings-page {
    display: flex;
    height: 100%;
  }

  /* Settings Sidebar */
  .sidebar {
    width: 240px;
    background: var(--color-bg-secondary);
    border-right: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
  }

  .sidebar-title {
    font-size: var(--font-size-lg);
    font-weight: var(--font-weight-semibold);
  }

  .security-info {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-sm);
    background: var(--color-success-light);
    border-radius: var(--border-radius-md);
    color: var(--color-success);
  }

  /* Settings Content */
  .settings-section {
    margin-bottom: var(--spacing-2xl);
  }

  .section-title {
    font-size: var(--font-size-2xl);
    font-weight: var(--font-weight-semibold);
    margin-bottom: var(--spacing-lg);
  }

  .mt-lg {
    margin-top: var(--spacing-lg);
  }

  .mb-sm {
    margin-bottom: var(--spacing-sm);
  }

  /* Icons */
  .icon-accent {
    color: var(--color-accent);
  }

  .icon-success {
    color: var(--color-success);
  }

  /* Status Badge */
  .status-badge {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-md);
    border-radius: var(--border-radius-md);
    margin-top: var(--spacing-md);
  }

  .status-badge.success {
    background: var(--color-success-light);
    color: var(--color-success);
  }

  /* Message Toast */
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

  /* Model Info */
  .model-info {
    padding: var(--spacing-md);
    background: var(--color-bg-secondary);
    border-radius: var(--border-radius-md);
    margin-top: var(--spacing-md);
  }

  /* Theme Cards */
  .theme-card {
    cursor: pointer;
    display: block;
  }

  .theme-card.selected .theme-preview {
    border: 2px solid var(--color-accent);
  }

  .theme-preview {
    background: var(--color-bg-primary);
    border: 1px solid var(--color-border);
    border-radius: var(--border-radius-lg);
    overflow: hidden;
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

  .theme-header {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    padding: var(--spacing-lg);
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
</style>
