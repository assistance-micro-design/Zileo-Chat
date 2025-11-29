<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

MemorySettings - Embedding configuration form for Memory Tool.
Allows users to configure embedding provider, model, and chunking settings.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button, Input, Select, Card, StatusIndicator } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type {
		EmbeddingConfig,
		EmbeddingProviderType,
		MemoryStats
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
			const [loadedConfig, loadedStats] = await Promise.all([
				invoke<EmbeddingConfig>('get_embedding_config'),
				invoke<MemoryStats>('get_memory_stats')
			]);
			config = loadedConfig;
			stats = loadedStats;
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

	@media (max-width: 768px) {
		.form-row {
			grid-template-columns: 1fr;
		}

		.stats-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
