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
	 * Effect to check API key status when provider changes
	 */
	$effect(() => {
		checkApiKeyStatus();
	});
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
				<option value="OpenAI">OpenAI</option>
				<option value="Anthropic">Anthropic</option>
			</select>
		</label>

		<label>
			Model:
			<input type="text" bind:value={settings.model} />
		</label>

		{#if settings.provider !== 'Ollama'}
			<div class="api-key-section">
				<label>
					API Key:
					<input
						type="password"
						bind:value={settings.apiKey}
						placeholder={hasStoredKey ? '(key stored securely)' : 'Enter API key'}
						disabled={saving}
					/>
				</label>

				<div class="api-key-actions">
					<button onclick={saveApiKey} disabled={saving || !settings.apiKey.trim()}>
						{saving ? 'Saving...' : 'Save API Key'}
					</button>

					{#if hasStoredKey}
						<button class="danger" onclick={deleteApiKey} disabled={saving}>
							Delete Stored Key
						</button>
					{/if}
				</div>

				{#if hasStoredKey}
					<p class="status-text success">API key is securely stored</p>
				{:else}
					<p class="status-text warning">No API key configured</p>
				{/if}
			</div>
		{:else}
			<p class="info-text">Ollama runs locally and does not require an API key.</p>
		{/if}

		{#if message}
			<p class="message {message.type}">{message.text}</p>
		{/if}
	</section>

	<section class="settings-section">
		<h2>Security Information</h2>
		<p class="info-text">
			API keys are stored securely using your operating system's keychain (Linux: libsecret, macOS:
			Keychain, Windows: Credential Manager) with additional AES-256 encryption for defense in
			depth.
		</p>
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

	h2 {
		margin-top: 0;
		margin-bottom: 1rem;
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
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
	}

	label input:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.api-key-section {
		margin-top: 1rem;
		padding-top: 1rem;
		border-top: 1px solid var(--color-border);
	}

	.api-key-actions {
		display: flex;
		gap: 0.5rem;
		margin-bottom: 0.5rem;
	}

	button {
		padding: 0.75rem 1.5rem;
		background: var(--color-accent);
		color: white;
		border: none;
		border-radius: 0.375rem;
		cursor: pointer;
		font-size: 0.875rem;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	button.danger {
		background: var(--color-error);
	}

	.status-text {
		font-size: 0.875rem;
		margin: 0.5rem 0;
	}

	.status-text.success {
		color: var(--color-success);
	}

	.status-text.warning {
		color: var(--color-text-secondary);
	}

	.info-text {
		font-size: 0.875rem;
		color: var(--color-text-secondary);
		line-height: 1.5;
	}

	.message {
		padding: 0.75rem;
		border-radius: 0.375rem;
		font-size: 0.875rem;
		margin-top: 1rem;
	}

	.message.success {
		background: color-mix(in srgb, var(--color-success) 15%, transparent);
		color: var(--color-success);
	}

	.message.error {
		background: color-mix(in srgb, var(--color-error) 15%, transparent);
		color: var(--color-error);
	}
</style>
