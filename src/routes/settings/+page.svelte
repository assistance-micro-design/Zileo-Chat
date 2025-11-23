<script lang="ts">
  /** Settings page - basic placeholder for Phase 0 */
  let settings = $state({
    provider: 'Mistral',
    model: 'mistral-large',
    apiKey: '',
  });

  function saveSettings() {
    alert('Settings saved (not implemented in base)');
  }
</script>

<div class="settings-page">
  <h1>Settings</h1>

  <section class="settings-section">
    <h2>LLM Provider</h2>

    <label>
      Provider:
      <select bind:value={settings.provider}>
        <option value="Mistral">Mistral</option>
        <option value="Ollama">Ollama (local)</option>
      </select>
    </label>

    <label>
      Model:
      <input type="text" bind:value={settings.model} />
    </label>

    {#if settings.provider === 'Mistral'}
      <label>
        API Key:
        <input type="password" bind:value={settings.apiKey} />
      </label>
    {/if}

    <button onclick={saveSettings}>Save Settings</button>
  </section>
</div>

<style>
  .settings-page {
    max-width: 800px;
    margin: 0 auto;
    padding: 2rem;
  }

  .settings-section {
    background: var(--color-bg-secondary);
    padding: 1.5rem;
    border-radius: 0.5rem;
    margin-bottom: 1.5rem;
  }

  label {
    display: block;
    margin-bottom: 1rem;
  }

  label input,
  label select {
    display: block;
    width: 100%;
    margin-top: 0.5rem;
    padding: 0.75rem;
    border: 1px solid var(--color-border);
    border-radius: 0.375rem;
  }

  button {
    padding: 0.75rem 1.5rem;
    background: var(--color-accent);
    color: white;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
  }
</style>
