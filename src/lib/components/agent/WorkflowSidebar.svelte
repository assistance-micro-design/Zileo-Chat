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

WorkflowSidebar Component
Left sidebar for workflow management with search and CRUD operations.
-->

<script lang="ts">
	import { Plus, Search, CheckSquare, FolderPlus } from '@lucide/svelte';
	import { Button, HelpButton } from '$lib/components/ui';
	import DeleteConfirmModal from '$lib/components/ui/DeleteConfirmModal.svelte';
	import Sidebar from '$lib/components/layout/Sidebar.svelte';
	import WorkflowList from '$lib/components/workflow/WorkflowList.svelte';
	import StatusFilters from '$lib/components/workflow/StatusFilters.svelte';
	import { SvelteSet } from 'svelte/reactivity';
	import { i18n } from '$lib/i18n';
	import { debounce } from '$lib/utils/debounce';
	import { getErrorMessage } from '$lib/utils/error';
	import type { Workflow, WorkflowFolder } from '$types/workflow';
	import type { StatusFilter } from '$types/sidebar';

	interface Props {
		collapsed?: boolean;
		workflows: Workflow[];
		selectedWorkflowId: string | null;
		searchFilter?: string;
		/** Error message from loadWorkflows failure */
		error?: string | null;
		/** Whether workflows are currently loading */
		loading?: boolean;
		/** Active status filter */
		activeStatusFilter?: StatusFilter;
		/** Workflow count per status */
		statusCounts?: Record<StatusFilter, number>;
		/** Available folders */
		folders?: WorkflowFolder[];
		/** Set of expanded folder IDs */
		expandedFolderIds?: Set<string>;
		onsearchchange?: (value: string) => void;
		onselect: (workflow: Workflow) => void;
		oncreate: () => void;
		ondelete: (workflow: Workflow) => void;
		onrename?: (workflow: Workflow, newName: string) => void;
		/** Retry handler for failed loads */
		onretry?: () => void;
		/** Handler for status filter changes */
		onstatusfilterchange?: (filter: StatusFilter) => void;
		/** Batch delete handler */
		onbatchdelete?: (ids: string[]) => Promise<{ deleted: number; skipped_running: string[] }>;
		/** Folder toggle handler */
		onfoldertoggle?: (folderId: string) => void;
		/** Folder create handler */
		onfoldercreate?: () => void;
		/** Folder rename handler */
		onfolderrename?: (folder: WorkflowFolder, name: string) => void;
		/** Folder delete handler */
		onfolderdelete?: (folder: WorkflowFolder) => void;
		/** Pin toggle handler */
		ontogglepin?: (workflow: Workflow) => void;
		/** Move to folder handler */
		onmoveto?: (workflow: Workflow, folderId: string | null) => void;
		/** Batch move workflows to folder (drag & drop) */
		onworkflowmove?: (workflowIds: string[], folderId: string | null) => void;
		/** Set of workflow IDs currently running in the background */
		runningWorkflowIds?: Set<string>;
		/** Set of workflow IDs that recently completed */
		recentlyCompletedIds?: Set<string>;
		/** Set of workflow IDs with a pending user question */
		questionPendingIds?: Set<string>;
	}

	const defaultCounts: Record<StatusFilter, number> = { all: 0, idle: 0, running: 0, completed: 0, error: 0 };

	let {
		collapsed = $bindable(false),
		workflows,
		selectedWorkflowId,
		searchFilter = $bindable(''),
		error = null,
		loading = false,
		activeStatusFilter = 'all',
		statusCounts = defaultCounts,
		folders = [],
		expandedFolderIds = new Set<string>(),
		onsearchchange,
		onselect,
		oncreate,
		ondelete,
		onrename,
		onretry,
		onstatusfilterchange,
		onbatchdelete,
		onfoldertoggle,
		onfoldercreate,
		onfolderrename,
		onfolderdelete,
		ontogglepin,
		onmoveto,
		onworkflowmove,
		runningWorkflowIds = new Set<string>(),
		recentlyCompletedIds = new Set<string>(),
		questionPendingIds = new Set<string>()
	}: Props = $props();

	/** Multi-selection state */
	let selectionMode = $state(false);
	let selectedIds = new SvelteSet<string>();
	let showBatchDeleteConfirm = $state(false);
	let batchDeleting = $state(false);

	/** Last clicked index for Shift+Click range selection */
	let lastClickedId = $state<string | null>(null);

	/**
	 * Toggle selection mode on/off
	 */
	function toggleSelectionMode(): void {
		selectionMode = !selectionMode;
		if (!selectionMode) {
			selectedIds.clear();
			lastClickedId = null;
		}
	}

	/**
	 * Handle selection toggle for a workflow (Ctrl+Click, Shift+Click, checkbox)
	 */
	function handleSelectionToggle(workflowId: string, event: MouseEvent | KeyboardEvent): void {
		if (!selectionMode) {
			selectionMode = true;
		}

		if ('shiftKey' in event && event.shiftKey && lastClickedId) {
			const ids = workflows.map((w) => w.id);
			const startIdx = ids.indexOf(lastClickedId);
			const endIdx = ids.indexOf(workflowId);
			if (startIdx !== -1 && endIdx !== -1) {
				const [from, to] = startIdx < endIdx ? [startIdx, endIdx] : [endIdx, startIdx];
				for (let i = from; i <= to; i++) {
					selectedIds.add(ids[i]);
				}
			}
		} else {
			if (selectedIds.has(workflowId)) {
				selectedIds.delete(workflowId);
			} else {
				selectedIds.add(workflowId);
			}
		}

		lastClickedId = workflowId;
	}

	/**
	 * Confirm and execute batch delete
	 */
	/** Error message from last failed batch delete */
	let batchDeleteError = $state<string | null>(null);

	async function handleBatchDelete(): Promise<void> {
		if (!onbatchdelete || selectedIds.size === 0) return;
		batchDeleting = true;
		batchDeleteError = null;
		try {
			await onbatchdelete([...selectedIds]);
			selectedIds.clear();
			selectionMode = false;
			lastClickedId = null;
		} catch (e) {
			batchDeleteError = getErrorMessage(e);
		} finally {
			batchDeleting = false;
			showBatchDeleteConfirm = false;
		}
	}

	function handleSearchInput(e: Event) {
		const target = e.target as HTMLInputElement;
		searchFilter = target.value;
		debouncedSearchChange(target.value);
	}

	const debouncedSearchChange = debounce((value: string) => {
		onsearchchange?.(value);
	}, 300);

</script>

<Sidebar bind:collapsed={collapsed}>
	{#snippet header(isCollapsed)}
		<div class="sidebar-header-content" class:collapsed={isCollapsed}>
			{#if isCollapsed}
				<Button
					variant="primary"
					size="icon"
					onclick={oncreate}
					ariaLabel={$i18n('workflow_new')}
					title={$i18n('workflow_new')}
				>
					<Plus size={16} />
				</Button>
			{:else}
				<div class="title-row">
					<h2 class="sidebar-title">{$i18n('workflow_title')}</h2>
					<Button variant="primary" size="icon" onclick={oncreate} ariaLabel={$i18n('workflow_new')}>
						<Plus size={14} />
					</Button>
				</div>
				<div class="secondary-actions">
					<HelpButton
						titleKey="help_workflow_sidebar_title"
						descriptionKey="help_workflow_sidebar_description"
						tutorialKey="help_workflow_sidebar_tutorial"
					/>
					{#if onfoldercreate}
						<button
							type="button"
							class="action-btn"
							onclick={onfoldercreate}
							title={$i18n('sidebar_folder_create')}
							aria-label={$i18n('sidebar_folder_create')}
						>
							<FolderPlus size={14} />
						</button>
					{/if}
					{#if onbatchdelete}
						<button
							type="button"
							class={['action-btn', selectionMode && 'active']}
							onclick={toggleSelectionMode}
							title={$i18n('sidebar_selection_toggle')}
							aria-label={$i18n('sidebar_selection_toggle')}
							aria-pressed={selectionMode}
						>
							<CheckSquare size={14} />
						</button>
					{/if}
				</div>
				<div class="search-input-wrapper">
					<span class="search-icon-container">
						<Search size={16} />
					</span>
					<input
						type="search"
						class="search-input"
						placeholder={$i18n('workflow_filter_placeholder')}
						value={searchFilter}
						oninput={handleSearchInput}
					/>
				</div>
				{#if onstatusfilterchange}
					<StatusFilters
						activeFilter={activeStatusFilter}
						counts={statusCounts}
						onfilterchange={onstatusfilterchange}
					/>
				{/if}
			{/if}
		</div>
	{/snippet}

	{#snippet nav(isCollapsed)}
		<WorkflowList
			{workflows}
			selectedId={selectedWorkflowId ?? undefined}
			collapsed={isCollapsed}
			{error}
			{loading}
			selectionMode={!isCollapsed && selectionMode}
			{selectedIds}
			{folders}
			{expandedFolderIds}
			{onselect}
			{ondelete}
			{onrename}
			{onretry}
			onselectiontoggle={handleSelectionToggle}
			{onfoldertoggle}
			{onfolderrename}
			{onfolderdelete}
			{ontogglepin}
			{onmoveto}
			onworkflowmove={!isCollapsed ? onworkflowmove : undefined}
			{runningWorkflowIds}
			{recentlyCompletedIds}
			{questionPendingIds}
		/>
		{#if selectionMode && selectedIds.size > 0 && !isCollapsed}
			<div class="batch-action-bar">
				{#if batchDeleteError}
					<p class="batch-error" role="alert">{batchDeleteError}</p>
				{/if}
				<Button
					variant="danger"
					size="sm"
					onclick={() => (showBatchDeleteConfirm = true)}
					disabled={batchDeleting}
				>
					{$i18n('sidebar_selection_delete', { count: selectedIds.size })}
				</Button>
				<Button
					variant="ghost"
					size="sm"
					onclick={toggleSelectionMode}
				>
					{$i18n('sidebar_selection_cancel')}
				</Button>
			</div>
		{/if}
	{/snippet}
</Sidebar>

{#if showBatchDeleteConfirm}
	<DeleteConfirmModal
		open={showBatchDeleteConfirm}
		titleKey="sidebar_selection_delete_title"
		confirmMessageKey="sidebar_selection_delete_confirm"
		warningMessageKey="sidebar_selection_delete_warning"
		deleting={batchDeleting}
		onConfirm={handleBatchDelete}
		onCancel={() => (showBatchDeleteConfirm = false)}
	/>
{/if}

<style>
	.sidebar-header-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		transition: all var(--transition-fast);
	}

	.sidebar-header-content.collapsed {
		align-items: center;
		justify-content: center;
		gap: 0;
	}

	.title-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-sm);
	}

	.sidebar-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.secondary-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.search-input-wrapper {
		position: relative;
		display: flex;
		align-items: center;
	}

	.search-icon-container {
		position: absolute;
		left: var(--spacing-sm);
		top: 50%;
		transform: translateY(-50%);
		color: var(--color-text-tertiary);
		pointer-events: none;
		z-index: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.search-input {
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		padding-left: calc(var(--spacing-sm) + 16px + var(--spacing-sm));
		font-size: var(--font-size-sm);
		font-family: var(--font-family);
		color: var(--color-text-primary);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		transition:
			border-color var(--transition-fast),
			box-shadow var(--transition-fast);
	}

	.search-input:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 3px var(--color-accent-light);
	}

	.search-input::placeholder {
		color: var(--color-text-tertiary);
	}

	/* Remove default search input styling */
	.search-input::-webkit-search-cancel-button {
		-webkit-appearance: none;
		appearance: none;
		height: 14px;
		width: 14px;
		background: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='%236c757d' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cline x1='18' y1='6' x2='6' y2='18'%3E%3C/line%3E%3Cline x1='6' y1='6' x2='18' y2='18'%3E%3C/line%3E%3C/svg%3E")
			center/contain no-repeat;
		cursor: pointer;
	}

	.action-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		background: transparent;
		border: 1px solid transparent;
		border-radius: var(--border-radius-md);
		color: var(--color-text-tertiary);
		cursor: pointer;
		transition: all var(--transition-fast);
	}

	.action-btn:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.action-btn.active {
		background: var(--color-accent-light);
		border-color: var(--color-accent);
		color: var(--color-accent);
	}

	.batch-action-bar {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm) var(--spacing-md);
		border-top: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
	}

	.batch-error {
		width: 100%;
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-error);
		text-align: center;
	}

</style>
