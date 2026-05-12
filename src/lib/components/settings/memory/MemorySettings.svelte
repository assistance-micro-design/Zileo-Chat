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
Decomposed into EmbeddingConfigCard, EmbeddingTestCard, MemoryStatsCard.
Chunking parameters are no longer exposed: they are fixed constants in
`tools/memory/chunker.rs` (512/50). The dimension is locked at 1024 by the
HNSW index schema.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriInvoke } from '$lib/tauri';
	import {
		Button,
		Select,
		Card,
		StatusIndicator,
		Modal,
		ErrorBanner,
		DeleteConfirmModal
	} from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type {
		EmbeddingConfig,
		EmbeddingProviderType,
		MemoryStats,
		MemoryTokenStats
	} from '$types/embedding';
	import { EMBEDDING_MODELS, DEFAULT_EMBEDDING_CONFIG } from '$types/embedding';
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

	/** Config state */
	let config = $state<EmbeddingConfig>({ ...DEFAULT_EMBEDDING_CONFIG });
	let editConfig = $state<EmbeddingConfig>({ ...DEFAULT_EMBEDDING_CONFIG });

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

	/** Model options based on selected provider */
	const modelOptions = $derived(
		EMBEDDING_MODELS[editConfig.provider as EmbeddingProviderType] || []
	);

	/**
	 * Loads the current embedding configuration.
	 *
	 * `get_embedding_config` returns `null` when no row exists; in that case
	 * we keep `config = defaults` for the modal and flag `configExists = false`
	 * so the UI shows the "no config" empty state instead of editable fields.
	 */
	async function loadConfig(): Promise<void> {
		loading = true;
		try {
			const [loadedConfig, loadedStats, loadedTokenStats] = await Promise.all([
				tauriInvoke<EmbeddingConfig | null>('get_embedding_config'),
				tauriInvoke<MemoryStats>('get_memory_stats'),
				tauriInvoke<MemoryTokenStats>('get_memory_token_stats', { typeFilter: null })
			]);
			if (loadedConfig) {
				config = loadedConfig;
				editConfig = { ...loadedConfig };
				configExists = true;
			} else {
				config = { ...DEFAULT_EMBEDDING_CONFIG };
				editConfig = { ...DEFAULT_EMBEDDING_CONFIG };
				configExists = false;
			}
			stats = loadedStats;
			tokenStats = loadedTokenStats;
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
	 * Confirms and executes configuration deletion.
	 *
	 * Calls `delete_embedding_config` (drops the DB row and clears the
	 * in-memory embedding service) instead of saving defaults — saving
	 * defaults left `configExists = true` and never released the service.
	 */
	async function confirmDelete(): Promise<void> {
		deleteDeleting = true;
		try {
			await tauriInvoke('delete_embedding_config');
			config = { ...DEFAULT_EMBEDDING_CONFIG };
			editConfig = { ...DEFAULT_EMBEDDING_CONFIG };
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
		}
	}

	/**
	 * Handle model change in modal
	 */
	function handleModelChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		editConfig.model = event.currentTarget.value;
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
<Modal open={showConfigModal} title={$i18n('memory_embedding_config')} onclose={closeConfigModal}>
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
