<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

MemorySettings - Embedding configuration form for Memory Tool.
Allows users to configure embedding provider, model, and chunking settings.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button, Input, Select, Card, StatusIndicator, Textarea } from '$lib/components/ui';
	import { TokenStatsCard } from './index';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type {
		EmbeddingConfig,
		EmbeddingProviderType,
		MemoryStats,
		EmbeddingTestResult,
		MemoryTokenStats
	} from '$types/embedding';

	/** Props */
	interface Props {
		/** Callback when config is saved */
		onsave?: () => void;
	}

	let { onsave }: Props = $props();

	/** Config state */
	let config = $state<EmbeddingConfig>({
		provider: 'mistral',
		model: 'mistral-embed',
		dimension: 1024,
		max_tokens: 8192,
		chunk_size: 512,
		chunk_overlap: 50,
		strategy: 'fixed'
	});

	/** Stats state */
	let stats = $state<MemoryStats | null>(null);

	/** UI state */
	let loading = $state(true);
	let saving = $state(false);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);

	/** Test embedding state */
	let testText = $state('');
	let testingEmbedding = $state(false);
	let testResult = $state<EmbeddingTestResult | null>(null);

	/** Token stats state */
	let tokenStats = $state<MemoryTokenStats | null>(null);

	/** Provider options */
	const providerOptions: SelectOption[] = [
		{ value: 'mistral', label: 'Mistral AI' },
		{ value: 'ollama', label: 'Ollama (Local)' }
	];

	/** Strategy options */
	const strategyOptions: SelectOption[] = [
		{ value: 'fixed', label: 'Fixed' },
		{ value: 'semantic', label: 'Semantic' },
		{ value: 'recursive', label: 'Recursive' }
	];

	/** Model options based on selected provider */
	const modelOptions = $derived.by(() => {
		const models: Record<EmbeddingProviderType, { value: string; label: string; dimension: number }[]> = {
			mistral: [{ value: 'mistral-embed', label: 'Mistral Embed (1024D)', dimension: 1024 }],
			ollama: [
				{ value: 'nomic-embed-text', label: 'Nomic Embed Text (768D)', dimension: 768 },
				{ value: 'mxbai-embed-large', label: 'MxBai Embed Large (1024D)', dimension: 1024 }
			]
		};

		return models[config.provider as EmbeddingProviderType] || [];
	});

	/**
	 * Loads the current embedding configuration
	 */
	async function loadConfig(): Promise<void> {
		loading = true;
		try {
			const [loadedConfig, loadedStats, loadedTokenStats] = await Promise.all([
				invoke<EmbeddingConfig>('get_embedding_config'),
				invoke<MemoryStats>('get_memory_stats'),
				invoke<MemoryTokenStats>('get_memory_token_stats', { typeFilter: null })
			]);
			config = loadedConfig;
			stats = loadedStats;
			tokenStats = loadedTokenStats;
		} catch (err) {
			message = { type: 'error', text: `Failed to load config: ${err}` };
		} finally {
			loading = false;
		}
	}

	/**
	 * Saves the embedding configuration
	 */
	async function handleSave(): Promise<void> {
		saving = true;
		message = null;

		try {
			await invoke('save_embedding_config', { config });
			message = { type: 'success', text: 'Configuration saved successfully' };
			onsave?.();
		} catch (err) {
			message = { type: 'error', text: `Failed to save: ${err}` };
		} finally {
			saving = false;
		}
	}

	/**
	 * Handle provider change
	 */
	function handleProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		const provider = event.currentTarget.value as EmbeddingProviderType;
		config.provider = provider;

		// Set default model for the selected provider
		const models = modelOptions;
		if (models.length > 0) {
			config.model = models[0].value;
			config.dimension = models[0].dimension;
		}
	}

	/**
	 * Handle model change
	 */
	function handleModelChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		const model = event.currentTarget.value;
		config.model = model;

		// Update dimension based on model
		const selectedModel = modelOptions.find((m) => m.value === model);
		if (selectedModel) {
			config.dimension = selectedModel.dimension;
		}
	}

	/**
	 * Handle strategy change
	 */
	function handleStrategyChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		config.strategy = event.currentTarget.value as 'fixed' | 'semantic' | 'recursive';
	}

	/**
	 * Tests embedding generation with sample text
	 */
	async function handleTestEmbedding(): Promise<void> {
		if (!testText.trim()) {
			message = { type: 'error', text: 'Please enter test text' };
			return;
		}

		testingEmbedding = true;
		testResult = null;
		message = null;

		try {
			testResult = await invoke<EmbeddingTestResult>('test_embedding', { text: testText });
			if (testResult.success) {
				message = { type: 'success', text: `Embedding generated in ${testResult.duration_ms}ms` };
			} else {
				message = { type: 'error', text: testResult.error || 'Unknown error' };
			}
		} catch (err) {
			message = { type: 'error', text: `Test failed: ${err}` };
		} finally {
			testingEmbedding = false;
		}
	}

	// Load config on mount
	$effect(() => {
		loadConfig();
	});
</script>

<div class="memory-settings">
	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>Loading configuration...</span>
				</div>
			{/snippet}
		</Card>
	{:else}
		<!-- Embedding Configuration -->
		<Card>
			{#snippet header()}
				<h3 class="card-title">Embedding Model</h3>
			{/snippet}
			{#snippet body()}
				<div class="config-form">
					<div class="form-row">
						<Select
							label="Provider"
							options={providerOptions}
							value={config.provider}
							onchange={handleProviderChange}
							help="Select from enabled LLM providers"
						/>

						<Select
							label="Embedding Model"
							options={modelOptions}
							value={config.model}
							onchange={handleModelChange}
							help={config.provider === 'mistral'
								? 'mistral-embed produces 1024D vectors'
								: 'nomic-embed-text (768D) for speed, mxbai-embed-large (1024D) for accuracy'}
						/>
					</div>

					<div class="form-row">
						<Input
							type="text"
							label="Dimensions"
							value={String(config.dimension)}
							disabled
							help="Vector dimensions (set by model)"
						/>
					</div>
				</div>
			{/snippet}
		</Card>

		<!-- Chunking Settings -->
		<Card>
			{#snippet header()}
				<h3 class="card-title">Chunking Settings</h3>
			{/snippet}
			{#snippet body()}
				<div class="config-form">
					<div class="form-row">
						<div class="slider-input">
							<span class="slider-label">
								Chunk Size: {config.chunk_size} characters
							</span>
							<input
								type="range"
								min="100"
								max="2000"
								step="50"
								bind:value={config.chunk_size}
								class="slider"
								aria-label="Chunk size in characters"
							/>
							<span class="slider-help">Characters per chunk (default: 512)</span>
						</div>

						<div class="slider-input">
							<span class="slider-label">
								Overlap: {config.chunk_overlap} characters
							</span>
							<input
								type="range"
								min="0"
								max="500"
								step="10"
								bind:value={config.chunk_overlap}
								class="slider"
								aria-label="Chunk overlap in characters"
							/>
							<span class="slider-help">Overlap between chunks (default: 50)</span>
						</div>
					</div>

					<div class="form-row">
						<Select
							label="Strategy"
							options={strategyOptions}
							value={config.strategy || 'fixed'}
							onchange={handleStrategyChange}
							help="Chunking strategy for text processing"
						/>
					</div>
				</div>
			{/snippet}
		</Card>

		<!-- Statistics -->
		{#if stats}
			<Card>
				{#snippet header()}
					<h3 class="card-title">Statistics</h3>
				{/snippet}
				{#snippet body()}
					<div class="stats-grid">
						<div class="stat-item">
							<span class="stat-value">{stats?.total ?? 0}</span>
							<span class="stat-label">Total Memories</span>
						</div>
						<div class="stat-item">
							<span class="stat-value">{stats?.with_embeddings ?? 0}</span>
							<span class="stat-label">With Embeddings</span>
						</div>
						<div class="stat-item">
							<span class="stat-value">{stats?.without_embeddings ?? 0}</span>
							<span class="stat-label">Without Embeddings</span>
						</div>
					</div>

					{#if stats && Object.keys(stats.by_type).length > 0}
						<div class="type-breakdown">
							<h4>By Type</h4>
							<div class="type-list">
								{#each Object.entries(stats?.by_type ?? {}) as [type, count]}
									<div class="type-item">
										<span class="type-name">{type}</span>
										<span class="type-count">{count}</span>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				{/snippet}
			</Card>
		{/if}

		<!-- Test Embedding Section -->
		<Card>
			{#snippet header()}
				<h3 class="card-title">Test Embedding</h3>
			{/snippet}
			{#snippet body()}
				<div class="test-section">
					<Textarea
						label="Test Text"
						value={testText}
						placeholder="Enter text to test embedding generation..."
						rows={3}
						oninput={(e) => (testText = e.currentTarget.value)}
					/>
					<div class="test-actions">
						<Button
							variant="secondary"
							onclick={handleTestEmbedding}
							disabled={!testText.trim() || testingEmbedding}
						>
							{testingEmbedding ? 'Testing...' : 'Test Embedding'}
						</Button>
					</div>

					{#if testResult}
						<div
							class="test-result"
							class:success={testResult.success}
							class:error={!testResult.success}
						>
							{#if testResult.success}
								<div class="result-row">
									<span class="result-label">Dimension:</span>
									<span class="result-value">{testResult.dimension}</span>
								</div>
								<div class="result-row">
									<span class="result-label">Duration:</span>
									<span class="result-value">{testResult.duration_ms}ms</span>
								</div>
								<div class="result-row">
									<span class="result-label">Provider:</span>
									<span class="result-value">{testResult.provider}</span>
								</div>
								<div class="result-row">
									<span class="result-label">Model:</span>
									<span class="result-value">{testResult.model}</span>
								</div>
								<div class="result-row">
									<span class="result-label">Preview:</span>
									<span class="result-value preview"
										>[{testResult.preview
											.slice(0, 3)
											.map((v) => v.toFixed(4))
											.join(', ')}...]</span
									>
								</div>
							{:else}
								<p class="error-text">{testResult.error}</p>
							{/if}
						</div>
					{/if}
				</div>
			{/snippet}
		</Card>

		<!-- Token Statistics Section -->
		{#if tokenStats}
			<TokenStatsCard stats={tokenStats} />
		{/if}

		<!-- Actions -->
		<div class="actions">
			{#if message}
				<div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
					{message.text}
				</div>
			{/if}

			<Button
				variant="primary"
				onclick={handleSave}
				disabled={saving}
			>
				{saving ? 'Saving...' : 'Save Configuration'}
			</Button>
		</div>
	{/if}
</div>

<style>
	.memory-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.config-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.form-row {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
	}

	.slider-input {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.slider-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.slider {
		width: 100%;
		height: 8px;
		border-radius: 4px;
		background: var(--color-bg-tertiary);
		outline: none;
		cursor: pointer;
	}

	.slider::-webkit-slider-thumb {
		-webkit-appearance: none;
		appearance: none;
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: var(--color-accent);
		cursor: pointer;
	}

	.slider::-moz-range-thumb {
		width: 20px;
		height: 20px;
		border-radius: 50%;
		background: var(--color-accent);
		cursor: pointer;
		border: none;
	}

	.slider-help {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.stats-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--spacing-lg);
		margin-bottom: var(--spacing-lg);
	}

	.stat-item {
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.stat-value {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-bold);
		color: var(--color-accent);
	}

	.stat-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.type-breakdown {
		margin-top: var(--spacing-md);
	}

	.type-breakdown h4 {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-sm);
	}

	.type-list {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-sm);
	}

	.type-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-sm);
	}

	.type-name {
		color: var(--color-text-secondary);
	}

	.type-count {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.actions {
		display: flex;
		flex-direction: column;
		align-items: flex-end;
		gap: var(--spacing-md);
	}

	.message {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		width: 100%;
		text-align: center;
	}

	.message.success {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.message.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}

	.test-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.test-actions {
		display: flex;
		justify-content: flex-start;
	}

	.test-result {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.test-result.success {
		background: var(--color-success-light);
		border: 1px solid var(--color-success);
	}

	.test-result.error {
		background: var(--color-error-light);
		border: 1px solid var(--color-error);
	}

	.result-row {
		display: flex;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-xs);
	}

	.result-label {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
		min-width: 80px;
	}

	.result-value {
		color: var(--color-text-primary);
	}

	.result-value.preview {
		font-family: var(--font-mono);
		font-size: var(--font-size-xs);
	}

	.error-text {
		color: var(--color-error);
		margin: 0;
	}

	@media (max-width: 768px) {
		.form-row {
			grid-template-columns: 1fr;
		}

		.stats-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
