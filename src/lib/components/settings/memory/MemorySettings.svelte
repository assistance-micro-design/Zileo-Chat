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
Decomposed into EmbeddingConfigCard, EmbeddingTestCard, MemoryStatsCard.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriInvoke } from '$lib/tauri';
	import { Button, Select, Card, StatusIndicator, Modal, ErrorBanner, DeleteConfirmModal } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type {
		EmbeddingConfig,
		EmbeddingProviderType,
		MemoryStats,
		MemoryTokenStats
	} from '$types/embedding';
	import { EMBEDDING_MODELS } from '$types/embedding';
	import { i18n, t } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import EmbeddingConfigCard from './EmbeddingConfigCard.svelte';
	import EmbeddingTestCard from './EmbeddingTestCard.svelte';
	import MemoryStatsCard from './MemoryStatsCard.svelte';

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
	let errorMessage = $state<string | null>(null);
	let modalError = $state<string | null>(null);
	let configExists = $state(false);

	/** Modal state */
	let showConfigModal = $state(false);

	/** Delete confirmation state */
	let showDeleteConfirm = $state(false);
	let deleteDeleting = $state(false);

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
	const modelOptions = $derived(
		EMBEDDING_MODELS[editConfig.provider as EmbeddingProviderType] || []
	);

	/**
	 * Loads the current embedding configuration
	 */
	async function loadConfig(): Promise<void> {
		loading = true;
		try {
			const [loadedConfig, loadedStats, loadedTokenStats] = await Promise.all([
				tauriInvoke<EmbeddingConfig>('get_embedding_config'),
				tauriInvoke<MemoryStats>('get_memory_stats'),
				tauriInvoke<MemoryTokenStats>('get_memory_token_stats', { typeFilter: null })
			]);
			config = loadedConfig;
			editConfig = { ...loadedConfig };
			stats = loadedStats;
			tokenStats = loadedTokenStats;
			configExists = Boolean(loadedConfig.provider && loadedConfig.model);
		} catch (err) {
			errorMessage = t('memory_failed_load').replace('{error}', getErrorMessage(err));
			configExists = false;
		} finally {
			loading = false;
		}
	}

	/**
	 * Refreshes only the memory statistics (called when memories change)
	 */
	export async function reload(): Promise<void> {
		try {
			const [loadedStats, loadedTokenStats] = await Promise.all([
				tauriInvoke<MemoryStats>('get_memory_stats'),
				tauriInvoke<MemoryTokenStats>('get_memory_token_stats', { typeFilter: null })
			]);
			stats = loadedStats;
			tokenStats = loadedTokenStats;
		} catch (err) {
			errorMessage = t('memory_failed_refresh_stats').replace('{error}', getErrorMessage(err));
		}
	}

	/**
	 * Opens the config modal for adding/editing
	 */
	function openConfigModal(): void {
		editConfig = { ...config };
		modalError = null;
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
		modalError = null;

		try {
			await tauriInvoke('save_embedding_config', { config: editConfig });
			config = { ...editConfig };
			configExists = true;
			showConfigModal = false;
			errorMessage = null;
			onsave?.();
		} catch (err) {
			modalError = t('memory_failed_save').replace('{error}', getErrorMessage(err));
		} finally {
			saving = false;
		}
	}

	/**
	 * Requests delete confirmation for embedding configuration
	 */
	function handleDeleteRequest(): void {
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes configuration deletion (resets to defaults)
	 */
	async function confirmDelete(): Promise<void> {
		deleteDeleting = true;
		try {
			await tauriInvoke('save_embedding_config', { config: defaultConfig });
			config = { ...defaultConfig };
			editConfig = { ...defaultConfig };
			configExists = false;
			errorMessage = null;
			showDeleteConfirm = false;
		} catch (err) {
			errorMessage = t('memory_failed_delete').replace('{error}', getErrorMessage(err));
		} finally {
			deleteDeleting = false;
		}
	}

	/**
	 * Cancels delete confirmation
	 */
	function cancelDelete(): void {
		showDeleteConfirm = false;
	}

	/**
	 * Handle provider change in modal
	 */
	function handleProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		const provider = event.currentTarget.value as EmbeddingProviderType;
		editConfig.provider = provider;

		const providerModels = EMBEDDING_MODELS[provider] || [];
		const firstModel = providerModels[0];
		if (firstModel) {
			editConfig.model = firstModel.value;
			editConfig.dimension = firstModel.dimension;
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

	// Load config on mount
	onMount(() => {
		loadConfig();
	});
</script>

<div class="memory-settings">
	{#if errorMessage}
		<ErrorBanner message={errorMessage} onDismiss={() => (errorMessage = null)} />
	{/if}

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
		<EmbeddingConfigCard
			{config}
			{configExists}
			{providerOptions}
			{strategyOptions}
			onOpenConfigModal={openConfigModal}
			onDelete={handleDeleteRequest}
		/>

		<!-- Embedding Test Card -->
		<EmbeddingTestCard {configExists} />

		<!-- Memory Statistics Card -->
		<MemoryStatsCard {stats} {tokenStats} />
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

			{#if modalError}
				<div class="modal-error">
					{modalError}
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

<!-- Delete Configuration Confirmation Modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="memory_config_delete_title"
	confirmMessageKey="memory_confirm_delete_config"
	deleting={deleteDeleting}
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
/>

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
		font-size: var(--font-size-base);
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

	.modal-error {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		text-align: center;
		background: var(--color-error-light);
		color: var(--color-error);
	}

	@media (max-width: 768px) {
		.form-row {
			grid-template-columns: 1fr;
		}
	}
</style>
