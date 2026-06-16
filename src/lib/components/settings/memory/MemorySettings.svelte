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
Two rows of two cards: config + connectivity test, then statistics +
operations (reindex, purge). The config is edited in place; saving is
explicit via the card footer. Chunking parameters are fixed constants in
`tools/memory/chunker.rs` (512/50); the dimension is locked at 1024 by
the HNSW index schema.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { tauriInvoke, tauriListen, type TauriUnlistenFn } from '$lib/tauri';
	import {
		Button,
		Card,
		StatusIndicator,
		ErrorBanner,
		ProgressBar,
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
	import { RefreshCw, Trash2 } from '@lucide/svelte';
	import EmbeddingConfigCard from './EmbeddingConfigCard.svelte';
	import EmbeddingTestCard from './EmbeddingTestCard.svelte';
	import MemoryStatsCard from './MemoryStatsCard.svelte';

	/** Props */
	interface Props {
		/** Callback when config is saved */
		onsave?: () => void;
	}

	let { onsave }: Props = $props();

	/** Config state — edited in place, persisted on explicit save. */
	let editConfig = $state<EmbeddingConfig>({ ...DEFAULT_EMBEDDING_CONFIG });

	/** Stats state */
	let stats = $state<MemoryStats | null>(null);
	let tokenStats = $state<MemoryTokenStats | null>(null);

	/** UI state */
	let loading = $state(true);
	let saving = $state(false);
	let errorMessage = $state<string | null>(null);
	let configExists = $state(false);

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
	 * the card keeps the defaults and flags `configExists = false` so the
	 * status badge reads "not configured".
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
				editConfig = { ...loadedConfig };
				configExists = true;
			} else {
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
	 * Saves the embedding configuration
	 */
	async function handleSave(): Promise<void> {
		saving = true;

		try {
			await tauriInvoke('save_embedding_config', { config: editConfig });
			configExists = true;
			errorMessage = null;
			notifyToast('success', t('memory_config_saved'));
			onsave?.();
		} catch (err) {
			notifyToast('error', t('memory_failed_save').replace('{error}', getErrorMessage(err)));
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
			editConfig = { ...DEFAULT_EMBEDDING_CONFIG };
			configExists = false;
			errorMessage = null;
			showDeleteConfirm = false;
			notifyToast('success', t('memory_config_deleted'));
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
	 * Handle provider change
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
	 * Handle model change
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
			const jobId = await tauriInvoke<string>('reindex_memory_chunks');
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

		// The listen() promise may resolve after the component unmounts (fast
		// navigation out of Settings > Memory). Guard with a cancellation flag so
		// the listener is torn down immediately on late resolution instead of
		// leaking an orphan handler that writes into destroyed $state.
		let cancelled = false;
		let unlistenFn: TauriUnlistenFn | undefined;
		void tauriListen<ReindexJobStatus>('reindex-progress', (event) => {
			// Strict filter: events from other jobs (rare but possible if the
			// user re-runs before the previous purge) are ignored.
			if (!reindexJobId || event.payload.jobId !== reindexJobId) return;
			reindexProgress = event.payload;
			if (event.payload.status !== 'running') {
				handleTerminalStatus(event.payload);
			}
		}).then((fn) => {
			if (cancelled) fn();
			else unlistenFn = fn;
		});

		return () => {
			cancelled = true;
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
		<!-- Row 1: embedding configuration + connectivity test -->
		<div class="row-grid">
			<EmbeddingConfigCard
				config={editConfig}
				{configExists}
				{saving}
				{providerOptions}
				{modelOptions}
				onProviderChange={handleProviderChange}
				onModelChange={handleModelChange}
				onSave={handleSave}
				onDelete={handleDeleteRequest}
			/>
			<EmbeddingTestCard {configExists} />
		</div>

		<!-- Row 2: statistics + operations -->
		<div class="row-grid">
			<MemoryStatsCard {stats} {tokenStats} />

			<Card title={$i18n('memory_operations_title')}>
				{#snippet body()}
					<div class="operations-body">
						<div class="operation">
							<div class="operation-head">
								<span class="operation-title">{$i18n('memory_reindex_button')}</span>
								{#if !reindexRunning}
									<Button
										variant="outline"
										size="sm"
										onclick={handleReindex}
										disabled={reindexStarting || !configExists}
									>
										<RefreshCw size={14} aria-hidden="true" />
										<span>
											{reindexStarting
												? $i18n('memory_reindex_starting')
												: $i18n('memory_reindex_action')}
										</span>
									</Button>
								{/if}
							</div>
							<p class="operation-help">{$i18n('memory_reindex_subtitle')}</p>
							{#if reindexRunning && reindexProgress}
								<div class="operation-progress">
									<div class="progress-track">
										<ProgressBar
											value={reindexProgress.processed}
											max={Math.max(reindexProgress.total, 1)}
											label={$i18n('memory_reindex_button')}
										/>
									</div>
									<span class="progress-text">
										{$i18n('memory_reindex_progress')
											.replace('{current}', String(reindexProgress.processed))
											.replace('{total}', String(reindexProgress.total))}
									</span>
									<Button
										variant="ghost"
										size="sm"
										onclick={handleCancelReindex}
										disabled={!reindexJobId}
									>
										{$i18n('memory_reindex_cancel_button')}
									</Button>
								</div>
							{/if}
						</div>

						<div class="operation separated">
							<div class="operation-head">
								<span class="operation-title">{$i18n('memory_purge_title')}</span>
								<Button variant="outline" size="sm" onclick={handlePurgeExpired} disabled={purging}>
									<Trash2 size={14} aria-hidden="true" />
									<span>
										{purging ? $i18n('memory_purge_running') : $i18n('memory_purge_button')}
									</span>
								</Button>
							</div>
							<p class="operation-help">{$i18n('memory_purge_subtitle')}</p>
						</div>
					</div>
				{/snippet}
			</Card>
		</div>
	{/if}
</div>

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

	.row-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-md);
		align-items: start;
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	.operations-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.operation {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
	}

	.operation.separated {
		border-top: 1px solid var(--color-border-light);
		padding-top: var(--spacing-md);
	}

	.operation-head {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
	}

	.operation-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.operation-help {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.operation-progress {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-top: var(--spacing-sm);
	}

	.progress-track {
		flex: 1;
		min-width: 0;
	}

	.progress-text {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		white-space: nowrap;
	}

	@media (max-width: 900px) {
		.row-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
