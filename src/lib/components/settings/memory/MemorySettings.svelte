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
	import { listen, type UnlistenFn } from '@tauri-apps/api/event';
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
		MemoryTokenStats,
		ReindexJobStatus
	} from '$types/embedding';
	import { EMBEDDING_MODELS, DEFAULT_EMBEDDING_CONFIG } from '$types/embedding';
	import { i18n, t } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { LocalStorage, STORAGE_KEYS } from '$lib/services/localStorage.service';
	import { toastStore } from '$lib/stores/toast';
	import { RefreshCw, DatabaseZap, Trash2 } from '@lucide/svelte';
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

	/** Reindex job state — driven by `reindex-progress` Tauri events. */
	let reindexJobId = $state<string | null>(null);
	let reindexStarting = $state(false);
	let reindexProgress = $state<ReindexJobStatus | null>(null);

	/** Purge state */
	let purging = $state(false);

	interface PurgeResult {
		memoriesPurged: number;
		chunksPurged: number;
	}
	const reindexRunning = $derived(reindexProgress?.status === 'running');
	const reindexPct = $derived(
		reindexProgress && reindexProgress.total > 0
			? Math.round((reindexProgress.processed / reindexProgress.total) * 100)
			: 0
	);

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

	function notifyToast(type: 'success' | 'error' | 'info', title: string, message = ''): void {
		toastStore.add({ type, title, message, persistent: false, duration: 5000 });
	}

	/**
	 * Maps backend statuses to user-facing toasts and clears the persisted
	 * job id when the job is terminal.
	 */
	function handleTerminalStatus(status: ReindexJobStatus): void {
		LocalStorage.remove(STORAGE_KEYS.REINDEX_JOB_ID);
		reindexJobId = null;
		if (status.status === 'completed') {
			notifyToast(
				'success',
				t('memory_reindex_complete')
					.replace('{chunks}', String(status.chunksCreated))
					.replace('{memories}', String(status.processed))
			);
		} else if (status.status === 'cancelled') {
			notifyToast(
				'info',
				t('memory_reindex_cancelled')
					.replace('{processed}', String(status.processed))
					.replace('{total}', String(status.total))
			);
		} else if (status.status === 'error') {
			notifyToast(
				'error',
				t('memory_reindex_error').replace('{error}', status.errorMessage ?? 'unknown')
			);
		}
		// Refresh stats so the dashboard reflects the new chunk indexing.
		reload().catch(() => undefined);
	}

	/**
	 * Restores reindex UI state from localStorage on mount.
	 *
	 * Three outcomes: (a) backend reports a still-running job → reattach
	 * the listener and show live progress; (b) backend reports a terminal
	 * status that wasn't read yet → surface a retroactive toast; (c)
	 * backend returns null (unknown — purged or app restart) → cleanup.
	 */
	async function restoreReindexFromStorage(): Promise<void> {
		const persisted = LocalStorage.get<string | null>(STORAGE_KEYS.REINDEX_JOB_ID, null);
		if (!persisted) return;
		try {
			const status = await tauriInvoke<ReindexJobStatus | null>('get_reindex_job_status', {
				jobId: persisted
			});
			if (!status) {
				// App restart or 10-min retention purge: nothing to show.
				LocalStorage.remove(STORAGE_KEYS.REINDEX_JOB_ID);
				return;
			}
			reindexJobId = persisted;
			reindexProgress = status;
			if (status.status !== 'running') {
				// Job finished while we were away — emit the retroactive toast.
				if (status.status === 'completed') {
					notifyToast('success', t('memory_reindex_restored'));
				} else {
					handleTerminalStatus(status);
				}
				LocalStorage.remove(STORAGE_KEYS.REINDEX_JOB_ID);
				reindexJobId = null;
			}
		} catch (err) {
			notifyToast('error', t('memory_reindex_error').replace('{error}', getErrorMessage(err)));
			LocalStorage.remove(STORAGE_KEYS.REINDEX_JOB_ID);
		}
	}

	/**
	 * Starts a new reindex job. The backend spawns a background task and
	 * returns the job_id; persists it so the user can leave the page
	 * without losing the progress thread.
	 */
	async function handleReindex(): Promise<void> {
		reindexStarting = true;
		try {
			const jobId = await tauriInvoke<string>('reindex_memory_chunks', { force: false });
			reindexJobId = jobId;
			LocalStorage.set(STORAGE_KEYS.REINDEX_JOB_ID, jobId);
			// Reset visible progress; the first `reindex-progress` event will
			// replace this with the real totals.
			reindexProgress = {
				jobId,
				status: 'running',
				processed: 0,
				total: 0,
				chunksCreated: 0,
				startedAt: new Date().toISOString()
			};
		} catch (err) {
			notifyToast('error', t('memory_reindex_error').replace('{error}', getErrorMessage(err)));
		} finally {
			reindexStarting = false;
		}
	}

	/**
	 * Triggers cancellation of the running job. The backend acknowledges
	 * via a final `reindex-progress` event with status="cancelled".
	 */
	async function handleCancelReindex(): Promise<void> {
		if (!reindexJobId) return;
		try {
			await tauriInvoke('cancel_reindex_job', { jobId: reindexJobId });
		} catch (err) {
			notifyToast('error', t('memory_reindex_error').replace('{error}', getErrorMessage(err)));
		}
	}

	/**
	 * Drops every memory whose `expires_at` is in the past plus its chunks.
	 * Idempotent — already-purged or unexpiring memories are left alone.
	 */
	async function handlePurgeExpired(): Promise<void> {
		purging = true;
		try {
			const result = await tauriInvoke<PurgeResult>('purge_expired_memories');
			if (result.memoriesPurged === 0) {
				notifyToast('info', t('memory_purge_empty'));
			} else {
				notifyToast(
					'success',
					t('memory_purge_done')
						.replace('{memories}', String(result.memoriesPurged))
						.replace('{chunks}', String(result.chunksPurged))
				);
				await reload();
			}
		} catch (err) {
			notifyToast('error', t('memory_purge_error').replace('{error}', getErrorMessage(err)));
		} finally {
			purging = false;
		}
	}

	// Mount: load config + stats, then restore any in-flight reindex.
	onMount(() => {
		loadConfig();
		void restoreReindexFromStorage();

		let unlistenFn: UnlistenFn | undefined;
		void listen<ReindexJobStatus>('reindex-progress', (event) => {
			// Strict filter: events from other jobs (rare but possible if the
			// user re-runs before the previous purge) are ignored.
			if (!reindexJobId || event.payload.jobId !== reindexJobId) return;
			reindexProgress = event.payload;
			if (event.payload.status !== 'running') {
				handleTerminalStatus(event.payload);
			}
		}).then((fn) => {
			unlistenFn = fn;
		});

		return () => {
			unlistenFn?.();
		};
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

		<!-- Operations section: Test + Reindex (only when config exists) -->
		{#if configExists}
			<section class="operations-section" aria-label={$i18n('memory_operations_title')}>
				<header class="section-header">
					<h2 class="section-title">{$i18n('memory_operations_title')}</h2>
				</header>
				<div class="operations-grid">
					<EmbeddingTestCard {configExists} />

					<!-- Purge Expired Card -->
					<Card>
						{#snippet header()}
							<div class="card-header-text">
								<div class="title-row">
									<Trash2 size={18} aria-hidden="true" />
									<h3 class="card-title">{$i18n('memory_purge_title')}</h3>
								</div>
								<p class="card-subtitle">{$i18n('memory_purge_subtitle')}</p>
							</div>
						{/snippet}
						{#snippet body()}
							<div class="reindex-body">
								<Button variant="secondary" onclick={handlePurgeExpired} disabled={purging}>
									<Trash2 size={16} />
									<span>
										{purging ? $i18n('memory_purge_running') : $i18n('memory_purge_button')}
									</span>
								</Button>
							</div>
						{/snippet}
					</Card>

					<!-- Reindex Card -->
					<Card>
						{#snippet header()}
							<div class="card-header-text">
								<div class="title-row">
									<DatabaseZap size={18} aria-hidden="true" />
									<h3 class="card-title">{$i18n('memory_reindex_button')}</h3>
								</div>
								<p class="card-subtitle">{$i18n('memory_reindex_subtitle')}</p>
							</div>
						{/snippet}
						{#snippet body()}
							<div class="reindex-body">
								{#if reindexRunning && reindexProgress}
									<p class="reindex-status">
										{$i18n('memory_reindex_progress')
											.replace('{current}', String(reindexProgress.processed))
											.replace('{total}', String(reindexProgress.total))}
									</p>
									<progress
										class="reindex-progress"
										value={reindexProgress.processed}
										max={Math.max(reindexProgress.total, 1)}
										aria-valuenow={reindexProgress.processed}
										aria-valuemax={reindexProgress.total}
										aria-label={$i18n('memory_reindex_button')}
									></progress>
									<p class="reindex-meta">
										{reindexPct}% · {reindexProgress.chunksCreated} chunks
									</p>
									<Button variant="ghost" onclick={handleCancelReindex} disabled={!reindexJobId}>
										{$i18n('memory_reindex_cancel_button')}
									</Button>
								{:else}
									<Button variant="secondary" onclick={handleReindex} disabled={reindexStarting}>
										<RefreshCw size={16} />
										<span>
											{reindexStarting
												? $i18n('memory_reindex_starting')
												: $i18n('memory_reindex_button')}
										</span>
									</Button>
								{/if}
							</div>
						{/snippet}
					</Card>
				</div>
			</section>

			<!-- Memory Statistics Card -->
			<MemoryStatsCard {stats} {tokenStats} />
		{/if}
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

	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.card-header-text {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
	}

	.title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-primary);
	}

	.card-subtitle {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.operations-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.section-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		padding-bottom: var(--spacing-2xs);
		border-bottom: 1px solid var(--color-border);
	}

	.section-title {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
		margin: 0;
	}

	.operations-grid {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
		gap: var(--spacing-lg);
		align-items: start;
	}

	.reindex-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.reindex-status {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.reindex-progress {
		width: 100%;
		height: 8px;
		appearance: none;
		border: none;
		border-radius: 4px;
		overflow: hidden;
		background: var(--color-bg-tertiary);
	}

	.reindex-progress::-webkit-progress-bar {
		background: var(--color-bg-tertiary);
		border-radius: 4px;
	}

	.reindex-progress::-webkit-progress-value {
		background: var(--color-accent);
		border-radius: 4px;
	}

	.reindex-progress::-moz-progress-bar {
		background: var(--color-accent);
		border-radius: 4px;
	}

	.reindex-meta {
		margin: 0;
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

		.operations-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
