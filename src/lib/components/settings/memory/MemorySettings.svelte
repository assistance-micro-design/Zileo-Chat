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

<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

MemorySettings - Embedding configuration for Memory Tool.
Allows users to configure embedding provider, model, and chunking settings via modal.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button, Select, Card, StatusIndicator, Textarea, Badge, ProgressBar, Modal } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type {
		EmbeddingConfig,
		EmbeddingProviderType,
		MemoryStats,
		EmbeddingTestResult,
		MemoryTokenStats
	} from '$types/embedding';
	import { Settings, Pencil, Trash2, Plus } from '@lucide/svelte';
	import { i18n, t } from '$lib/i18n';

	/** Props */
	interface Props {
		/** Callback when config is saved */
		onsave?: () => void;
	}

	let { onsave }: Props = $props();

	/** Default config values */
	const defaultConfig: EmbeddingConfig = {
		provider: 'mistral',
		model: 'mistral-embed',
		dimension: 1024,
		max_tokens: 8192,
		chunk_size: 512,
		chunk_overlap: 50,
		strategy: 'fixed'
	};

	/** Config state */
	let config = $state<EmbeddingConfig>({ ...defaultConfig });
	let editConfig = $state<EmbeddingConfig>({ ...defaultConfig });

	/** Stats state */
	let stats = $state<MemoryStats | null>(null);
	let tokenStats = $state<MemoryTokenStats | null>(null);

	/** UI state */
	let loading = $state(true);
	let saving = $state(false);
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
	let configExists = $state(false);

	/** Modal state */
	let showConfigModal = $state(false);

	/** Test embedding state */
	let testText = $state('');
	let testingEmbedding = $state(false);
	let testResult = $state<EmbeddingTestResult | null>(null);

	/** Provider options (reactive to locale) */
	const providerOptions = $derived<SelectOption[]>([
		{ value: 'mistral', label: t('memory_provider_mistral') },
		{ value: 'ollama', label: t('memory_provider_ollama') }
	]);

	/** Strategy options (reactive to locale) */
	const strategyOptions = $derived<SelectOption[]>([
		{ value: 'fixed', label: t('memory_strategy_fixed') },
		{ value: 'semantic', label: t('memory_strategy_semantic') },
		{ value: 'recursive', label: t('memory_strategy_recursive') }
	]);

	/** Model options based on selected provider */
	const modelOptions = $derived.by(() => {
		const models: Record<EmbeddingProviderType, { value: string; label: string; dimension: number }[]> = {
			mistral: [{ value: 'mistral-embed', label: 'Mistral Embed (1024D)', dimension: 1024 }],
			ollama: [
				{ value: 'nomic-embed-text', label: 'Nomic Embed Text (768D)', dimension: 768 },
				{ value: 'mxbai-embed-large', label: 'MxBai Embed Large (1024D)', dimension: 1024 }
			]
		};

		return models[editConfig.provider as EmbeddingProviderType] || [];
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
			editConfig = { ...loadedConfig };
			stats = loadedStats;
			tokenStats = loadedTokenStats;
			// Config exists if backend returns a valid config (provider and model are set)
			configExists = Boolean(loadedConfig.provider && loadedConfig.model);
		} catch (err) {
			message = { type: 'error', text: t('memory_failed_load').replace('{error}', String(err)) };
			configExists = false;
		} finally {
			loading = false;
		}
	}

	/**
	 * Refreshes only the memory statistics (called when memories change)
	 */
	export async function refreshStats(): Promise<void> {
		try {
			const [loadedStats, loadedTokenStats] = await Promise.all([
				invoke<MemoryStats>('get_memory_stats'),
				invoke<MemoryTokenStats>('get_memory_token_stats', { typeFilter: null })
			]);
			stats = loadedStats;
			tokenStats = loadedTokenStats;
		} catch (err) {
			console.error('Failed to refresh stats:', err);
		}
	}

	/**
	 * Opens the config modal for adding/editing
	 */
	function openConfigModal(): void {
		editConfig = { ...config };
		message = null;
		showConfigModal = true;
	}

	/**
	 * Closes the config modal
	 */
	function closeConfigModal(): void {
		showConfigModal = false;
	}

	/**
	 * Saves the embedding configuration
	 */
	async function handleSave(): Promise<void> {
		saving = true;
		message = null;

		try {
			await invoke('save_embedding_config', { config: editConfig });
			config = { ...editConfig };
			configExists = true;
			message = { type: 'success', text: t('memory_config_saved') };
			showConfigModal = false;
			onsave?.();
		} catch (err) {
			message = { type: 'error', text: t('memory_failed_save').replace('{error}', String(err)) };
		} finally {
			saving = false;
		}
	}

	/**
	 * Deletes the embedding configuration (resets to defaults)
	 */
	async function handleDelete(): Promise<void> {
		if (!confirm(t('memory_confirm_delete_config'))) {
			return;
		}

		saving = true;
		try {
			// Save default config to reset
			await invoke('save_embedding_config', { config: defaultConfig });
			config = { ...defaultConfig };
			editConfig = { ...defaultConfig };
			configExists = false;
			message = { type: 'success', text: t('memory_config_deleted') };
		} catch (err) {
			message = { type: 'error', text: t('memory_failed_delete').replace('{error}', String(err)) };
		} finally {
			saving = false;
		}
	}

	/**
	 * Handle provider change in modal
	 */
	function handleProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		const provider = event.currentTarget.value as EmbeddingProviderType;
		editConfig.provider = provider;

		// Set default model for the selected provider
		const models: Record<EmbeddingProviderType, { value: string; label: string; dimension: number }[]> = {
			mistral: [{ value: 'mistral-embed', label: 'Mistral Embed (1024D)', dimension: 1024 }],
			ollama: [
				{ value: 'nomic-embed-text', label: 'Nomic Embed Text (768D)', dimension: 768 },
				{ value: 'mxbai-embed-large', label: 'MxBai Embed Large (1024D)', dimension: 1024 }
			]
		};
		const providerModels = models[provider] || [];
		if (providerModels.length > 0) {
			editConfig.model = providerModels[0].value;
			editConfig.dimension = providerModels[0].dimension;
		}
	}

	/**
	 * Handle model change in modal
	 */
	function handleModelChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		const model = event.currentTarget.value;
		editConfig.model = model;

		const selectedModel = modelOptions.find((m) => m.value === model);
		if (selectedModel) {
			editConfig.dimension = selectedModel.dimension;
		}
	}

	/**
	 * Handle strategy change in modal
	 */
	function handleStrategyChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		editConfig.strategy = event.currentTarget.value as 'fixed' | 'semantic' | 'recursive';
	}

	/**
	 * Format large numbers for display
	 */
	function formatNumber(n: number): string {
		if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`;
		if (n >= 1000) return `${(n / 1000).toFixed(1)}K`;
		return n.toString();
	}

	/**
	 * Get badge variant based on memory type
	 */
	function getTypeVariant(type: string): 'primary' | 'success' | 'warning' | 'error' {
		switch (type) {
			case 'knowledge':
				return 'warning';
			case 'context':
				return 'success';
			case 'decision':
				return 'error';
			case 'user_pref':
				return 'primary';
			default:
				return 'primary';
		}
	}

	/**
	 * Get provider display name
	 */
	function getProviderLabel(provider: string): string {
		return providerOptions.find((p) => p.value === provider)?.label || provider;
	}

	/**
	 * Get strategy display name
	 */
	function getStrategyLabel(strategy: string): string {
		return strategyOptions.find((s) => s.value === strategy)?.label || strategy;
	}

	/**
	 * Tests embedding generation with sample text
	 */
	async function handleTestEmbedding(): Promise<void> {
		if (!testText.trim()) {
			message = { type: 'error', text: t('memory_enter_test_text') };
			return;
		}

		testingEmbedding = true;
		testResult = null;
		message = null;

		try {
			testResult = await invoke<EmbeddingTestResult>('test_embedding', { text: testText });
			if (testResult.success) {
				message = { type: 'success', text: t('memory_embedding_generated').replace('{duration}', String(testResult.duration_ms)) };
			} else {
				message = { type: 'error', text: testResult.error || t('common_error') };
			}
		} catch (err) {
			message = { type: 'error', text: t('memory_test_failed').replace('{error}', String(err)) };
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
					<span>{$i18n('memory_loading_config')}</span>
				</div>
			{/snippet}
		</Card>
	{:else}
		<!-- Embedding Configuration Card -->
		<Card>
			{#snippet header()}
				<div class="card-header-row">
					<h3 class="card-title">{$i18n('memory_embedding_config')}</h3>
					{#if configExists}
						<div class="header-actions">
							<button type="button" class="icon-btn" onclick={openConfigModal} title={$i18n('common_edit')}>
								<Pencil size={16} />
							</button>
							<button type="button" class="icon-btn danger" onclick={handleDelete} title={$i18n('common_delete')}>
								<Trash2 size={16} />
							</button>
						</div>
					{/if}
				</div>
			{/snippet}
			{#snippet body()}
				{#if configExists}
					<div class="config-display">
						<div class="config-grid">
							<div class="config-item">
								<span class="config-label">{$i18n('memory_provider')}</span>
								<span class="config-value">{getProviderLabel(config.provider)}</span>
							</div>
							<div class="config-item">
								<span class="config-label">{$i18n('memory_model')}</span>
								<span class="config-value">{config.model}</span>
							</div>
							<div class="config-item">
								<span class="config-label">{$i18n('memory_dimensions')}</span>
								<span class="config-value">{config.dimension}D</span>
							</div>
							<div class="config-item">
								<span class="config-label">{$i18n('memory_strategy')}</span>
								<span class="config-value">{getStrategyLabel(config.strategy || 'fixed')}</span>
							</div>
							<div class="config-item">
								<span class="config-label">{$i18n('memory_chunk_size')}</span>
								<span class="config-value">{config.chunk_size} {$i18n('memory_chars')}</span>
							</div>
							<div class="config-item">
								<span class="config-label">{$i18n('memory_overlap')}</span>
								<span class="config-value">{config.chunk_overlap} {$i18n('memory_chars')}</span>
							</div>
						</div>
					</div>
				{:else}
					<div class="empty-state">
						<Settings size={48} strokeWidth={1} />
						<h4>{$i18n('memory_no_config')}</h4>
						<p>{$i18n('memory_no_config_description')}</p>
						<Button variant="primary" onclick={openConfigModal}>
							<Plus size={16} />
							{$i18n('memory_add_config')}
						</Button>
					</div>
				{/if}
			{/snippet}
		</Card>

		<!-- Test Embedding Section -->
		<Card>
			{#snippet header()}
				<h3 class="card-title">{$i18n('memory_test_title')}</h3>
			{/snippet}
			{#snippet body()}
				<div class="test-section">
					<Textarea
						label={$i18n('memory_test_text_label')}
						value={testText}
						placeholder={$i18n('memory_test_text_placeholder')}
						rows={3}
						oninput={(e) => (testText = e.currentTarget.value)}
					/>
					<div class="test-actions">
						<Button
							variant="secondary"
							onclick={handleTestEmbedding}
							disabled={!testText.trim() || testingEmbedding || !configExists}
						>
							{testingEmbedding ? $i18n('memory_testing') : $i18n('memory_test_button')}
						</Button>
						{#if !configExists}
							<span class="test-hint">{$i18n('memory_configure_first')}</span>
						{/if}
					</div>

					{#if testResult}
						<div
							class="test-result"
							class:success={testResult.success}
							class:error={!testResult.success}
						>
							{#if testResult.success}
								<div class="result-row">
									<span class="result-label">{$i18n('memory_dimension')}</span>
									<span class="result-value">{testResult.dimension}</span>
								</div>
								<div class="result-row">
									<span class="result-label">{$i18n('memory_duration')}</span>
									<span class="result-value">{testResult.duration_ms}ms</span>
								</div>
								<div class="result-row">
									<span class="result-label">{$i18n('memory_provider')}</span>
									<span class="result-value">{testResult.provider}</span>
								</div>
								<div class="result-row">
									<span class="result-label">{$i18n('memory_model')}</span>
									<span class="result-value">{testResult.model}</span>
								</div>
								<div class="result-row">
									<span class="result-label">{$i18n('memory_preview')}</span>
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

					{#if message && !showConfigModal}
						<div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
							{message.text}
						</div>
					{/if}
				</div>
			{/snippet}
		</Card>

		<!-- Memory Statistics -->
		{#if stats || tokenStats}
			<Card>
				{#snippet header()}
					<h3 class="card-title">{$i18n('memory_stats_title')}</h3>
				{/snippet}
				{#snippet body()}
					<div class="unified-stats">
						<!-- Summary Row -->
						<div class="summary-stats">
							<div class="summary-item">
								<span class="summary-value">{formatNumber(stats?.total ?? tokenStats?.total_memories ?? 0)}</span>
								<span class="summary-label">{$i18n('memory_total_memories')}</span>
							</div>
							<div class="summary-item">
								<span class="summary-value">{formatNumber(tokenStats?.total_chars ?? 0)}</span>
								<span class="summary-label">{$i18n('memory_total_characters')}</span>
							</div>
							<div class="summary-item">
								<span class="summary-value">{formatNumber(tokenStats?.total_estimated_tokens ?? 0)}</span>
								<span class="summary-label">{$i18n('memory_est_tokens')}</span>
							</div>
							<div class="summary-item">
								<span class="summary-value">{stats?.with_embeddings ?? 0}/{stats?.total ?? 0}</span>
								<span class="summary-label">{$i18n('memory_with_embeddings')}</span>
							</div>
						</div>

						<!-- Category Breakdown -->
						{#if tokenStats && tokenStats.categories.length > 0}
							<div class="categories-section">
								<h4 class="section-title">{$i18n('memory_by_category')}</h4>
								<div class="categories-list">
									{#each tokenStats.categories as cat}
										<div class="category-item">
											<div class="category-header">
												<Badge variant={getTypeVariant(cat.memory_type)}>{cat.memory_type}</Badge>
												<span class="category-count">{cat.count} {$i18n('memory_memories_count')}</span>
												<span class="embedding-status">{cat.with_embeddings}/{cat.count} {$i18n('memory_embedded')}</span>
											</div>
											<div class="category-details">
												<span class="token-count">{formatNumber(cat.estimated_tokens)} {$i18n('memory_tokens')}</span>
												<span class="char-count">({formatNumber(cat.total_chars)} {$i18n('memory_chars')})</span>
											</div>
											<ProgressBar
												value={tokenStats.total_chars > 0 ? (cat.total_chars / tokenStats.total_chars) * 100 : 0}
												showLabel={false}
											/>
										</div>
									{/each}
								</div>
							</div>
						{:else if stats && Object.keys(stats.by_type).length > 0}
							<div class="categories-section">
								<h4 class="section-title">{$i18n('memory_by_type')}</h4>
								<div class="type-list">
									{#each Object.entries(stats.by_type) as [type, count]}
										<div class="type-item">
											<Badge variant={getTypeVariant(type)}>{type}</Badge>
											<span class="type-count">{count}</span>
										</div>
									{/each}
								</div>
							</div>
						{/if}
					</div>
				{/snippet}
			</Card>
		{/if}
	{/if}
</div>

<!-- Configuration Modal -->
<Modal
	open={showConfigModal}
	title={$i18n('memory_embedding_config')}
	onclose={closeConfigModal}
>
	{#snippet body()}
		<div class="modal-form">
			<!-- Embedding Model Section -->
			<div class="modal-section">
				<h4 class="modal-section-title">{$i18n('memory_embedding_model')}</h4>
				<div class="form-row">
					<Select
						label={$i18n('memory_provider')}
						options={providerOptions}
						value={editConfig.provider}
						onchange={handleProviderChange}
						help={$i18n('memory_select_provider_help')}
					/>

					<Select
						label={$i18n('memory_model')}
						options={modelOptions}
						value={editConfig.model}
						onchange={handleModelChange}
						help={editConfig.provider === 'mistral'
							? $i18n('memory_mistral_help')
							: $i18n('memory_ollama_help')}
					/>
				</div>
				<div class="dimension-info">
					<span class="dimension-label">{$i18n('memory_vector_dimensions')}</span>
					<span class="dimension-value">{editConfig.dimension}D</span>
				</div>
			</div>

			<!-- Chunking Settings Section -->
			<div class="modal-section">
				<h4 class="modal-section-title">{$i18n('memory_chunking_settings')}</h4>
				<div class="form-row">
					<div class="slider-input">
						<span class="slider-label">
							{$i18n('memory_chunk_size_label').replace('{size}', String(editConfig.chunk_size))}
						</span>
						<input
							type="range"
							min="100"
							max="2000"
							step="50"
							bind:value={editConfig.chunk_size}
							class="slider"
							aria-label={$i18n('memory_chunk_size')}
						/>
						<span class="slider-help">{$i18n('memory_chunk_size_help')}</span>
					</div>

					<div class="slider-input">
						<span class="slider-label">
							{$i18n('memory_overlap_label').replace('{size}', String(editConfig.chunk_overlap))}
						</span>
						<input
							type="range"
							min="0"
							max="500"
							step="10"
							bind:value={editConfig.chunk_overlap}
							class="slider"
							aria-label={$i18n('memory_overlap')}
						/>
						<span class="slider-help">{$i18n('memory_overlap_help')}</span>
					</div>
				</div>

				<Select
					label={$i18n('memory_strategy')}
					options={strategyOptions}
					value={editConfig.strategy || 'fixed'}
					onchange={handleStrategyChange}
					help={$i18n('memory_strategy_help')}
				/>
			</div>

			{#if message && showConfigModal}
				<div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
					{message.text}
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet footer()}
		<div class="modal-actions">
			<Button variant="ghost" onclick={closeConfigModal} disabled={saving}>
				{$i18n('common_cancel')}
			</Button>
			<Button variant="primary" onclick={handleSave} disabled={saving}>
				{saving ? $i18n('common_saving') : $i18n('memory_save_config')}
			</Button>
		</div>
	{/snippet}
</Modal>

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

	.card-header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}

	.header-actions {
		display: flex;
		gap: var(--spacing-xs);
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xs);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-sm);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition: color 0.2s, background 0.2s;
	}

	.icon-btn:hover {
		color: var(--color-text-primary);
		background: var(--color-bg-hover);
	}

	.icon-btn.danger:hover {
		color: var(--color-error);
	}

	/* Config Display */
	.config-display {
		padding: var(--spacing-sm);
	}

	.config-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--spacing-md);
	}

	.config-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
	}

	.config-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.config-value {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	/* Empty State */
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-2xl);
		text-align: center;
		color: var(--color-text-secondary);
	}

	.empty-state h4 {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.empty-state p {
		font-size: var(--font-size-sm);
		margin: 0;
		max-width: 300px;
	}

	.empty-state :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	/* Modal Form */
	.modal-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.modal-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.modal-section-title {
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		padding-bottom: var(--spacing-sm);
		border-bottom: 1px solid var(--color-border);
	}

	.form-row {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
	}

	.dimension-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-sm);
	}

	.dimension-label {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.dimension-value {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-accent);
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

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
	}

	/* Test Section */
	.test-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.test-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.test-hint {
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
		font-style: italic;
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

	.message {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
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

	/* Statistics */
	.unified-stats {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.summary-stats {
		display: grid;
		grid-template-columns: repeat(4, 1fr);
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.summary-item {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
	}

	.summary-value {
		font-size: var(--font-size-xl);
		font-weight: var(--font-weight-bold);
		color: var(--color-accent);
	}

	.summary-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.categories-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.section-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-secondary);
	}

	.categories-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.category-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		background: var(--color-bg-tertiary);
		border-radius: var(--border-radius-sm);
	}

	.category-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.category-count {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.embedding-status {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-left: auto;
	}

	.category-details {
		display: flex;
		gap: var(--spacing-sm);
		font-size: var(--font-size-sm);
	}

	.token-count {
		color: var(--color-text-primary);
	}

	.char-count {
		color: var(--color-text-secondary);
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

	.type-count {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	@media (max-width: 768px) {
		.form-row {
			grid-template-columns: 1fr;
		}

		.config-grid {
			grid-template-columns: repeat(2, 1fr);
		}

		.summary-stats {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 480px) {
		.config-grid {
			grid-template-columns: 1fr;
		}

		.summary-stats {
			grid-template-columns: 1fr;
		}

		.category-header {
			flex-direction: column;
			align-items: flex-start;
		}

		.embedding-status {
			margin-left: 0;
		}
	}
</style>
