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

MemoryList - Memory table with CRUD operations.
Toolbar (search, type filter, export/import, add) and table share a
single card; row actions are quiet icon buttons.
-->

<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { tauriInvoke, saveDialog, isTauriRuntime } from '$lib/tauri';
	import {
		Button,
		Input,
		Select,
		Badge,
		StatusIndicator,
		Modal,
		DeleteConfirmModal
	} from '$lib/components/ui';
	import type { Memory, MemoryType, ChunkSearchResult } from '$types/memory';
	import type { ImportResult } from '$types/embedding';
	import MemoryForm from './MemoryForm.svelte';
	import { Trash2, Pencil, Eye, Download, Upload, Plus, Search } from '@lucide/svelte';
	import { i18n, t } from '$lib/i18n';
	import { getErrorMessage } from '$lib/utils/error';
	import { downloadBrowserFile } from '$lib/utils/browser-download';
	import { toastStore } from '$lib/stores/toast';
	import type { ToastType } from '$types/background-workflow';
	import {
		buildMemoryTypeOptions,
		formatDate,
		formatDateShort,
		formatImportFailureMessage,
		formatScope,
		getExportMetadata,
		getTypeVariant,
		truncate
	} from './MemoryList.helpers';

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	/** Props */
	interface Props {
		/** Callback when memories change */
		onchange?: () => void;
	}

	let { onchange }: Props = $props();

	/** Memory list state */
	let memories = $state<Memory[]>([]);
	let loading = $state(true);
	let searching = $state(false);

	/** Filter state */
	let typeFilter = $state<MemoryType | ''>('');
	let searchQuery = $state('');

	/** Modal state */
	let showFormModal = $state(false);
	let formMode = $state<'add' | 'edit'>('add');
	let editingMemory = $state<Memory | undefined>(undefined);
	let showViewModal = $state(false);
	let viewingMemory = $state<Memory | null>(null);

	/** Action state */
	let actionLoading = $state(false);

	/** Delete confirmation state */
	let showDeleteConfirm = $state(false);
	let memoryToDelete = $state<Memory | null>(null);
	let deleting = $state(false);

	/** Memory type options (reactive to locale) */
	const typeOptions = $derived(
		buildMemoryTypeOptions({
			all: t('memory_type_all'),
			userPref: t('memory_type_user_pref'),
			context: t('memory_type_context'),
			knowledge: t('memory_type_knowledge'),
			decision: t('memory_type_decision')
		})
	);

	/** Localized labels for memory types (badges show readable names) */
	const typeLabels = $derived<Record<string, string>>({
		user_pref: t('memory_type_user_pref'),
		context: t('memory_type_context'),
		knowledge: t('memory_type_knowledge'),
		decision: t('memory_type_decision')
	});

	function typeLabel(type: string): string {
		return typeLabels[type] ?? type;
	}

	/**
	 * Loads memories from backend (both workflow and general scope)
	 */
	async function loadMemories(): Promise<void> {
		loading = true;
		try {
			const filter = typeFilter || undefined;
			// Pass workflowId as null to get ALL memories (both workflow-scoped and general)
			memories = await tauriInvoke<Memory[]>('list_memories', {
				typeFilter: filter,
				workflowId: null
			});
		} catch (err) {
			notify('error', t('memory_failed_load').replace('{error}', getErrorMessage(err)));
		} finally {
			loading = false;
		}
	}

	/**
	 * Searches memories semantically using vector search with text fallback.
	 *
	 * `search_memories` returns one row per matching chunk; the dashboard
	 * shows whole memories, so we dedupe by `parentMemoryId` (keeping the
	 * first occurrence which carries the highest score thanks to the
	 * ORDER BY score DESC on the backend) and synthesize a `Memory` shape
	 * from each chunk's parent fields. The chunk text is used as a preview.
	 */
	async function handleSearch(): Promise<void> {
		if (!searchQuery.trim()) {
			await loadMemories();
			return;
		}

		searching = true;
		try {
			const results = await tauriInvoke<ChunkSearchResult[]>('search_memories', {
				query: searchQuery,
				limit: 50,
				typeFilter: typeFilter || undefined,
				workflowId: null,
				threshold: 0.7
			});
			const seen: Record<string, true> = {};
			memories = results
				.filter((r) => {
					if (seen[r.parentMemoryId]) return false;
					seen[r.parentMemoryId] = true;
					return true;
				})
				.map<Memory>((r) => ({
					id: r.parentMemoryId,
					type: r.memoryType,
					content: r.content,
					workflow_id: r.workflowId ?? undefined,
					metadata: r.metadata,
					importance: r.importance,
					expires_at: r.expiresAt ?? undefined,
					created_at: r.createdAt
				}));
		} catch (err) {
			notify('error', t('memory_search_failed').replace('{error}', getErrorMessage(err)));
		} finally {
			searching = false;
		}
	}

	/**
	 * Opens the add memory modal
	 */
	function openAddModal(): void {
		formMode = 'add';
		editingMemory = undefined;
		showFormModal = true;
	}

	/**
	 * Opens the edit memory modal
	 */
	function openEditModal(memory: Memory): void {
		formMode = 'edit';
		editingMemory = memory;
		showFormModal = true;
	}

	/**
	 * Closes the form modal
	 */
	function closeFormModal(): void {
		showFormModal = false;
		editingMemory = undefined;
	}

	/**
	 * Opens the view memory modal
	 */
	function openViewModal(memory: Memory): void {
		viewingMemory = memory;
		showViewModal = true;
	}

	/**
	 * Closes the view modal
	 */
	function closeViewModal(): void {
		showViewModal = false;
		viewingMemory = null;
	}

	/**
	 * Handles memory form save
	 */
	async function handleFormSave(): Promise<void> {
		closeFormModal();
		await loadMemories();
		onchange?.();
	}

	/**
	 * Requests delete confirmation for a memory
	 */
	function handleDeleteRequest(memory: Memory): void {
		memoryToDelete = memory;
		showDeleteConfirm = true;
	}

	/**
	 * Confirms and executes memory deletion
	 */
	async function confirmDelete(): Promise<void> {
		if (!memoryToDelete) return;
		deleting = true;
		try {
			await tauriInvoke('delete_memory', { memoryId: memoryToDelete.id });
			memories = memories.filter((m) => m.id !== memoryToDelete!.id);
			notify('success', t('memory_deleted'));
			showDeleteConfirm = false;
			memoryToDelete = null;
			onchange?.();
		} catch (err) {
			notify('error', t('memory_failed_delete_memory').replace('{error}', getErrorMessage(err)));
		} finally {
			deleting = false;
		}
	}

	/**
	 * Cancels delete confirmation
	 */
	function cancelDelete(): void {
		showDeleteConfirm = false;
		memoryToDelete = null;
	}

	/**
	 * Exports memories using native Tauri save dialog
	 */
	async function handleExport(format: 'json' | 'csv'): Promise<void> {
		actionLoading = true;
		try {
			const exportMetadata = getExportMetadata(format);
			const data = await tauriInvoke<string>('export_memories', {
				format: exportMetadata.exportFormat,
				typeFilter: typeFilter || undefined
			});

			if (!isTauriRuntime()) {
				downloadBrowserFile(exportMetadata.defaultFilename, data, exportMetadata.mimeType);
				notify('success', t('memory_exported').replace('{count}', String(memories.length)));
				return;
			}

			const filePath = await saveDialog({
				defaultPath: exportMetadata.defaultFilename,
				filters: [{ name: exportMetadata.filterName, extensions: [exportMetadata.extension] }],
				title: t('memory_export_title')
			});

			if (!filePath) {
				actionLoading = false;
				return;
			}

			await tauriInvoke('save_export_to_file', { path: filePath, content: data });

			notify('success', t('memory_exported').replace('{count}', String(memories.length)));
		} catch (err) {
			notify('error', t('memory_export_failed').replace('{error}', getErrorMessage(err)));
		} finally {
			actionLoading = false;
		}
	}

	/**
	 * Imports memories from file
	 */
	async function handleImport(): Promise<void> {
		const input = document.createElement('input');
		input.type = 'file';
		input.accept = '.json';

		input.onchange = async (e) => {
			const file = (e.target as HTMLInputElement).files?.[0];
			if (!file) return;

			actionLoading = true;
			try {
				const text = await file.text();
				const result = await tauriInvoke<ImportResult>('import_memories', { data: text });

				if (result.imported > 0) {
					notify('success', t('memory_imported').replace('{count}', String(result.imported)));
					await loadMemories();
					onchange?.();
				}

				if (result.failed > 0) {
					notify(
						'error',
						formatImportFailureMessage(t('memory_import_failed'), result.failed, result.errors)
					);
				}
			} catch (err) {
				notify('error', t('memory_import_failed_generic').replace('{error}', getErrorMessage(err)));
			} finally {
				actionLoading = false;
			}
		};

		input.click();
	}

	/**
	 * Handle type filter change
	 */
	function handleTypeChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		typeFilter = event.currentTarget.value as MemoryType | '';
		loadMemories();
	}

	/**
	 * Handle search with debounce
	 */
	let searchTimeout: ReturnType<typeof setTimeout> | undefined;
	function handleSearchInput(event: Event & { currentTarget: HTMLInputElement }): void {
		searchQuery = event.currentTarget.value;
		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			handleSearch();
		}, 300);
	}

	// Load memories on mount
	onMount(() => {
		loadMemories();
	});

	// Cancel any pending debounced search so we do not fire after the component is gone.
	onDestroy(() => {
		clearTimeout(searchTimeout);
	});
</script>

<div class="memory-list">
	<div class="card">
		<!-- Toolbar: search, type filter, export/import, add -->
		<div class="list-toolbar">
			<div class="search-box">
				<Search size={14} class="search-icon" />
				<Input
					type="search"
					placeholder={$i18n('memory_search_placeholder')}
					value={searchQuery}
					oninput={handleSearchInput}
				/>
			</div>
			<Select
				options={typeOptions}
				value={typeFilter}
				onchange={handleTypeChange}
				ariaLabel={$i18n('memory_table_type')}
			/>
			<Button
				variant="outline"
				size="sm"
				onclick={() => handleExport('json')}
				disabled={actionLoading}
			>
				<Download size={14} aria-hidden="true" />
				<span>{$i18n('memory_export_json')}</span>
			</Button>
			<Button
				variant="outline"
				size="sm"
				onclick={() => handleExport('csv')}
				disabled={actionLoading}
			>
				<Download size={14} aria-hidden="true" />
				<span>{$i18n('memory_export_csv')}</span>
			</Button>
			<Button variant="outline" size="sm" onclick={handleImport} disabled={actionLoading}>
				<Upload size={14} aria-hidden="true" />
				<span>{$i18n('memory_import')}</span>
			</Button>
			<Button variant="primary" size="sm" onclick={openAddModal}>
				<Plus size={14} aria-hidden="true" />
				<span>{$i18n('memory_add')}</span>
			</Button>
		</div>

		{#if loading || searching}
			<div class="list-state">
				<StatusIndicator status="running" />
				<span>{searching ? $i18n('memory_searching') : $i18n('memory_loading')}</span>
			</div>
		{:else if memories.length === 0}
			<div class="list-state empty">
				<h3>{$i18n('memory_no_memories')}</h3>
				<p>
					{searchQuery ? $i18n('memory_no_match') : $i18n('memory_no_created')}
				</p>
				{#if !searchQuery}
					<Button variant="primary" onclick={openAddModal}>
						{$i18n('memory_add_first')}
					</Button>
				{/if}
			</div>
		{:else}
			<div class="table-scroll">
				<table class="table">
					<thead>
						<tr>
							<th>{$i18n('memory_table_type')}</th>
							<th>{$i18n('memory_table_scope')}</th>
							<th>{$i18n('memory_table_content')}</th>
							<th>{$i18n('memory_table_date')}</th>
							<th>{$i18n('memory_table_actions')}</th>
						</tr>
					</thead>
					<tbody>
						{#each memories as memory (memory.id)}
							<tr>
								<td>
									<Badge variant={getTypeVariant(memory.type as MemoryType)}>
										{typeLabel(memory.type)}
									</Badge>
								</td>
								<td class="cell-scope" title={memory.workflow_id || $i18n('memory_scope_general')}>
									{formatScope(memory.workflow_id, t('memory_scope_general'))}
								</td>
								<td class="cell-content">
									{truncate(memory.content, 100)}
								</td>
								<td class="cell-date">
									{formatDateShort(memory.created_at)}
								</td>
								<td>
									<div class="entity-actions">
										<Button
											variant="ghost"
											size="sm"
											onclick={() => openViewModal(memory)}
											ariaLabel={$i18n('memory_modal_view')}
										>
											<Eye size={14} />
										</Button>
										<Button
											variant="ghost"
											size="sm"
											onclick={() => openEditModal(memory)}
											ariaLabel={$i18n('common_edit')}
										>
											<Pencil size={14} />
										</Button>
										<Button
											variant="ghost"
											size="sm"
											onclick={() => handleDeleteRequest(memory)}
											ariaLabel={$i18n('common_delete')}
										>
											<Trash2 size={14} />
										</Button>
									</div>
								</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</div>
</div>

<!-- Add/Edit Modal -->
<Modal
	open={showFormModal}
	title={formMode === 'add' ? $i18n('memory_modal_add') : $i18n('memory_modal_edit')}
	onclose={closeFormModal}
>
	{#snippet body()}
		<MemoryForm
			mode={formMode}
			memory={editingMemory}
			onsave={handleFormSave}
			oncancel={closeFormModal}
		/>
	{/snippet}
</Modal>

<!-- View Modal -->
<Modal open={showViewModal} title={$i18n('memory_modal_view')} onclose={closeViewModal}>
	{#snippet body()}
		{#if viewingMemory}
			<div class="view-content">
				<div class="view-field">
					<span class="field-label">{$i18n('memory_field_type')}</span>
					<Badge variant={getTypeVariant(viewingMemory.type as MemoryType)}>
						{typeLabel(viewingMemory.type)}
					</Badge>
				</div>
				<div class="view-field">
					<span class="field-label">{$i18n('memory_field_content')}</span>
					<pre class="content-preview">{viewingMemory.content}</pre>
				</div>
				<div class="view-field">
					<span class="field-label">{$i18n('memory_field_created')}</span>
					<span>{formatDate(viewingMemory.created_at)}</span>
				</div>
				{#if Object.keys(viewingMemory.metadata).length > 0}
					<div class="view-field">
						<span class="field-label">{$i18n('memory_field_metadata')}</span>
						<pre class="metadata-preview">{JSON.stringify(viewingMemory.metadata, null, 2)}</pre>
					</div>
				{/if}
			</div>
		{/if}
	{/snippet}
	{#snippet footer()}
		<Button variant="ghost" onclick={closeViewModal}>
			{$i18n('common_close')}
		</Button>
	{/snippet}
</Modal>

<!-- Delete Confirmation Modal -->
<DeleteConfirmModal
	open={showDeleteConfirm}
	titleKey="memory_delete_title"
	confirmMessageKey="memory_confirm_delete"
	{deleting}
	deletingLabelKey="memory_deleting"
	onConfirm={confirmDelete}
	onCancel={cancelDelete}
/>

<!-- Reindex moved to Settings -> Memory (see MemorySettings.svelte) -->

<style>
	.memory-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.list-toolbar {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		padding: var(--spacing-md) var(--spacing-lg);
	}

	.list-toolbar :global(.form-group) {
		width: auto;
	}

	.list-toolbar :global(.form-select) {
		width: auto;
	}

	.search-box {
		position: relative;
		flex: 1;
		min-width: 220px;
	}

	.search-box :global(.search-icon) {
		position: absolute;
		left: 10px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-text-tertiary);
		z-index: 1;
		pointer-events: none;
	}

	.search-box :global(input) {
		padding-left: 2rem;
	}

	.list-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-2xl);
		text-align: center;
		border-top: 1px solid var(--color-border-light);
	}

	.list-state h3 {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.list-state p {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	/* Table */

	.table-scroll {
		max-height: 420px;
		overflow-y: auto;
		border-top: 1px solid var(--color-border-light);
	}

	.table-scroll :global(thead th) {
		position: sticky;
		top: 0;
		z-index: 1;
	}

	.cell-scope {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		white-space: nowrap;
	}

	.cell-content {
		max-width: 0;
		width: 60%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cell-date {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		white-space: nowrap;
	}

	.entity-actions {
		display: flex;
		gap: var(--spacing-xs);
	}

	.view-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.view-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.field-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
	}

	.content-preview,
	.metadata-preview {
		background: var(--color-bg-secondary);
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		white-space: pre-wrap;
		word-break: break-word;
		font-family: var(--font-mono);
		font-size: var(--font-size-sm);
		max-height: 300px;
		overflow-y: auto;
		margin: 0;
	}

	@media (max-width: 768px) {
		.list-toolbar {
			flex-direction: column;
			align-items: stretch;
		}

		.search-box {
			min-width: 0;
		}

		.table-scroll {
			max-height: 300px;
		}
	}
</style>
