<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

MemoryList - Memory table with CRUD operations.
Displays memories with filtering, search, and action buttons.
-->

<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { Button, Card, Input, Select, Badge, StatusIndicator, Modal } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type { Memory, MemoryType, MemorySearchResult } from '$types/memory';
	import type { ExportFormat, ImportResult, RegenerateResult } from '$types/embedding';
	import MemoryForm from './MemoryForm.svelte';
	import { Trash2, Edit, Eye, Download, Upload, RefreshCw } from 'lucide-svelte';
	import { i18n, t } from '$lib/i18n';

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
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);

	/** Memory type options (reactive to locale) */
	const typeOptions = $derived<SelectOption[]>([
		{ value: '', label: t('memory_type_all') },
		{ value: 'user_pref', label: t('memory_type_user_pref') },
		{ value: 'context', label: t('memory_type_context') },
		{ value: 'knowledge', label: t('memory_type_knowledge') },
		{ value: 'decision', label: t('memory_type_decision') }
	]);

	/**
	 * Truncates text to specified length
	 */
	function truncate(text: string, maxLength: number): string {
		if (text.length <= maxLength) return text;
		return text.slice(0, maxLength) + '...';
	}

	/**
	 * Formats date for display
	 */
	function formatDate(dateStr: string): string {
		const date = new Date(dateStr);
		return date.toLocaleDateString(undefined, {
			year: 'numeric',
			month: 'short',
			day: 'numeric',
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	/**
	 * Gets badge variant for memory type
	 */
	function getTypeVariant(type: MemoryType): 'primary' | 'success' | 'warning' | 'error' {
		const variants: Record<MemoryType, 'primary' | 'success' | 'warning' | 'error'> = {
			user_pref: 'primary',
			context: 'success',
			knowledge: 'warning',
			decision: 'error'
		};
		return variants[type] || 'primary';
	}

	/**
	 * Formats scope (workflow_id or General)
	 */
	function formatScope(workflowId: string | undefined | null): string {
		if (!workflowId) return t('memory_scope_general');
		// Truncate long workflow IDs
		return workflowId.length > 12 ? workflowId.slice(0, 12) + '...' : workflowId;
	}

	/**
	 * Loads memories from backend (both workflow and general scope)
	 */
	async function loadMemories(): Promise<void> {
		loading = true;
		try {
			const filter = typeFilter || undefined;
			// Pass workflowId as null to get ALL memories (both workflow-scoped and general)
			memories = await invoke<Memory[]>('list_memories', { typeFilter: filter, workflowId: null });
		} catch (err) {
			message = { type: 'error', text: t('memory_failed_load').replace('{error}', String(err)) };
		} finally {
			loading = false;
		}
	}

	/**
	 * Searches memories semantically using vector search with text fallback
	 */
	async function handleSearch(): Promise<void> {
		if (!searchQuery.trim()) {
			await loadMemories();
			return;
		}

		searching = true;
		try {
			// Search all memories (both workflow-scoped and general)
			// Vector search will be used if embedding service is configured
			const results = await invoke<MemorySearchResult[]>('search_memories', {
				query: searchQuery,
				limit: 50,
				typeFilter: typeFilter || undefined,
				workflowId: null, // Search all scopes
				threshold: 0.7 // Similarity threshold for vector search
			});
			memories = results.map((r) => r.memory);
		} catch (err) {
			message = { type: 'error', text: t('memory_search_failed').replace('{error}', String(err)) };
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
	 * Deletes a memory
	 */
	async function handleDelete(memory: Memory): Promise<void> {
		if (!confirm(t('memory_confirm_delete'))) {
			return;
		}

		actionLoading = true;
		try {
			await invoke('delete_memory', { memoryId: memory.id });
			memories = memories.filter((m) => m.id !== memory.id);
			message = { type: 'success', text: t('memory_deleted') };
			onchange?.();
		} catch (err) {
			message = { type: 'error', text: t('memory_failed_delete_memory').replace('{error}', String(err)) };
		} finally {
			actionLoading = false;
		}
	}

	/**
	 * Exports memories
	 */
	async function handleExport(format: 'json' | 'csv'): Promise<void> {
		actionLoading = true;
		try {
			const exportFormat: ExportFormat = format === 'json' ? 'json' : 'csv';
			const data = await invoke<string>('export_memories', {
				format: exportFormat,
				typeFilter: typeFilter || undefined
			});

			// Create download link
			const blob = new Blob([data], { type: format === 'json' ? 'application/json' : 'text/csv' });
			const url = URL.createObjectURL(blob);
			const a = document.createElement('a');
			a.href = url;
			a.download = `memories-${new Date().toISOString().slice(0, 10)}.${format}`;
			document.body.appendChild(a);
			a.click();
			document.body.removeChild(a);
			URL.revokeObjectURL(url);

			message = { type: 'success', text: t('memory_exported').replace('{count}', String(memories.length)) };
		} catch (err) {
			message = { type: 'error', text: t('memory_export_failed').replace('{error}', String(err)) };
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
				const result = await invoke<ImportResult>('import_memories', { data: text });

				if (result.imported > 0) {
					message = { type: 'success', text: t('memory_imported').replace('{count}', String(result.imported)) };
					await loadMemories();
					onchange?.();
				}

				if (result.failed > 0) {
					message = {
						type: 'error',
						text: t('memory_import_failed').replace('{count}', String(result.failed)).replace('{errors}', result.errors.slice(0, 3).join(', '))
					};
				}
			} catch (err) {
				message = { type: 'error', text: t('memory_import_failed_generic').replace('{error}', String(err)) };
			} finally {
				actionLoading = false;
			}
		};

		input.click();
	}

	/**
	 * Regenerates embeddings for all memories
	 */
	async function handleRegenerateEmbeddings(): Promise<void> {
		if (!confirm(t('memory_confirm_regenerate'))) {
			return;
		}

		actionLoading = true;
		try {
			const result = await invoke<RegenerateResult>('regenerate_embeddings', {
				typeFilter: typeFilter || undefined
			});
			message = {
				type: 'success',
				text: t('memory_regenerate_result')
					.replace('{processed}', String(result.processed))
					.replace('{success}', String(result.success))
					.replace('{failed}', String(result.failed))
			};
			onchange?.();
		} catch (err) {
			message = { type: 'error', text: t('memory_regenerate_failed').replace('{error}', String(err)) };
		} finally {
			actionLoading = false;
		}
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
	let searchTimeout: ReturnType<typeof setTimeout>;
	function handleSearchInput(event: Event & { currentTarget: HTMLInputElement }): void {
		searchQuery = event.currentTarget.value;
		clearTimeout(searchTimeout);
		searchTimeout = setTimeout(() => {
			handleSearch();
		}, 300);
	}

	// Load memories on mount
	$effect(() => {
		loadMemories();
	});
</script>

<div class="memory-list">
	<!-- Header Actions -->
	<div class="header-actions">
		<div class="filters">
			<Select
				label=""
				options={typeOptions}
				value={typeFilter}
				onchange={handleTypeChange}
			/>
			<Input
				type="search"
				placeholder={$i18n('memory_search_placeholder')}
				value={searchQuery}
				oninput={handleSearchInput}
			/>
		</div>

		<div class="actions">
			<Button variant="secondary" size="sm" onclick={() => handleExport('json')} disabled={actionLoading}>
				<Download size={16} />
				<span>{$i18n('memory_export_json')}</span>
			</Button>
			<Button variant="secondary" size="sm" onclick={() => handleExport('csv')} disabled={actionLoading}>
				<Download size={16} />
				<span>{$i18n('memory_export_csv')}</span>
			</Button>
			<Button variant="secondary" size="sm" onclick={handleImport} disabled={actionLoading}>
				<Upload size={16} />
				<span>{$i18n('memory_import')}</span>
			</Button>
			<Button variant="secondary" size="sm" onclick={handleRegenerateEmbeddings} disabled={actionLoading}>
				<RefreshCw size={16} />
				<span>{$i18n('memory_regenerate')}</span>
			</Button>
			<Button variant="primary" size="sm" onclick={openAddModal}>
				{$i18n('memory_add')}
			</Button>
		</div>
	</div>

	{#if message}
		<div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
			{message.text}
		</div>
	{/if}

	<!-- Memory Table -->
	{#if loading || searching}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>{searching ? $i18n('memory_searching') : $i18n('memory_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if memories.length === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<h3>{$i18n('memory_no_memories')}</h3>
					<p>
						{searchQuery
							? $i18n('memory_no_match')
							: $i18n('memory_no_created')}
					</p>
					{#if !searchQuery}
						<Button variant="primary" onclick={openAddModal}>
							{$i18n('memory_add_first')}
						</Button>
					{/if}
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="table-container">
			<table class="memory-table">
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
									{memory.type}
								</Badge>
							</td>
							<td class="scope-cell" title={memory.workflow_id || $i18n('memory_scope_general')}>
								<span class="scope-badge" class:workflow={memory.workflow_id}>
									{formatScope(memory.workflow_id)}
								</span>
							</td>
							<td class="content-cell">
								{truncate(memory.content, 100)}
							</td>
							<td class="date-cell">
								{formatDate(memory.created_at)}
							</td>
							<td class="actions-cell">
								<button
									type="button"
									class="action-btn"
									onclick={() => openViewModal(memory)}
									title={$i18n('memory_modal_view')}
								>
									<Eye size={16} />
								</button>
								<button
									type="button"
									class="action-btn"
									onclick={() => openEditModal(memory)}
									title={$i18n('common_edit')}
								>
									<Edit size={16} />
								</button>
								<button
									type="button"
									class="action-btn"
									onclick={() => handleDelete(memory)}
									title={$i18n('common_delete')}
								>
									<Trash2 size={16} />
								</button>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
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
<Modal
	open={showViewModal}
	title={$i18n('memory_modal_view')}
	onclose={closeViewModal}
>
	{#snippet body()}
		{#if viewingMemory}
			<div class="view-content">
				<div class="view-field">
					<span class="field-label">{$i18n('memory_field_type')}</span>
					<Badge variant={getTypeVariant(viewingMemory.type as MemoryType)}>
						{viewingMemory.type}
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

<style>
	.memory-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.header-actions {
		display: flex;
		justify-content: space-between;
		align-items: flex-end;
		gap: var(--spacing-lg);
		flex-wrap: wrap;
	}

	.filters {
		display: flex;
		gap: var(--spacing-md);
		flex: 1;
		min-width: 300px;
	}

	.actions {
		display: flex;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.message {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.message.success {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.message.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}

	.loading-state,
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-2xl);
		text-align: center;
	}

	.empty-state h3 {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.empty-state p {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.table-container {
		overflow-x: auto;
	}

	.memory-table {
		width: 100%;
		border-collapse: collapse;
		font-size: var(--font-size-sm);
	}

	.memory-table th,
	.memory-table td {
		padding: var(--spacing-md);
		text-align: left;
		border-bottom: 1px solid var(--color-border);
	}

	.memory-table th {
		font-weight: var(--font-weight-semibold);
		background: var(--color-bg-secondary);
		color: var(--color-text-secondary);
	}

	.memory-table tbody tr:hover {
		background: var(--color-bg-hover);
	}

	.content-cell {
		max-width: 400px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.date-cell {
		white-space: nowrap;
		color: var(--color-text-secondary);
	}

	.scope-cell {
		white-space: nowrap;
	}

	.scope-badge {
		display: inline-block;
		padding: var(--spacing-2xs) var(--spacing-xs);
		border-radius: var(--border-radius-sm);
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-medium);
		background: var(--color-bg-tertiary);
		color: var(--color-text-secondary);
	}

	.scope-badge.workflow {
		background: var(--color-accent-light);
		color: var(--color-accent);
	}

	.actions-cell {
		display: flex;
		gap: var(--spacing-xs);
	}

	.action-btn {
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

	.action-btn:hover {
		color: var(--color-text-primary);
		background: var(--color-bg-hover);
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
		font-family: var(--font-family-mono);
		font-size: var(--font-size-sm);
		max-height: 300px;
		overflow-y: auto;
		margin: 0;
	}

	@media (max-width: 768px) {
		.header-actions {
			flex-direction: column;
			align-items: stretch;
		}

		.filters {
			flex-direction: column;
		}

		.actions {
			justify-content: center;
		}
	}
</style>
