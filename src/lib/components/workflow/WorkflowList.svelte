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
  WorkflowList Component
  A list of workflow items with selection, folder grouping, and pinned support.
  Supports collapsed mode for compact sidebar display.
-->
<script lang="ts">
	import type { Workflow, WorkflowFolder } from '$types/workflow';
	import WorkflowItem from './WorkflowItem.svelte';
	import WorkflowItemCompact from './WorkflowItemCompact.svelte';
	import FolderItem from './FolderItem.svelte';
	import { AlertTriangle, RefreshCw } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { getWorkflowIdsFromDrag, hasWorkflowDragData } from '$lib/utils/dragDrop';
	import { groupByDate } from '$lib/utils/dateGrouping';

	const DATE_GROUP_I18N: Record<string, string> = {
		today: 'workflow_group_today',
		yesterday: 'workflow_group_yesterday',
		last_7_days: 'workflow_group_last_7_days',
		older: 'workflow_group_older'
	};

	interface Props {
		/** Array of workflows to display */
		workflows: Workflow[];
		/** ID of the currently selected workflow */
		selectedId?: string;
		/** Whether to show compact view (collapsed sidebar) */
		collapsed?: boolean;
		/** Error message from loadWorkflows failure */
		error?: string | null;
		/** Whether workflows are currently loading */
		loading?: boolean;
		/** Whether multi-selection mode is active */
		selectionMode?: boolean;
		/** Set of selected workflow IDs (multi-select) */
		selectedIds?: Set<string>;
		/** Available folders */
		folders?: WorkflowFolder[];
		/** Set of expanded folder IDs */
		expandedFolderIds?: Set<string>;
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
		/** Delete handler */
		ondelete?: (workflow: Workflow) => void;
		/** Rename handler */
		onrename?: (workflow: Workflow, newName: string) => void;
		/** Retry handler for failed loads */
		onretry?: () => void;
		/** Multi-selection toggle handler */
		onselectiontoggle?: (workflowId: string, event: MouseEvent | KeyboardEvent) => void;
		/** Folder toggle handler */
		onfoldertoggle?: (folderId: string) => void;
		/** Folder rename handler */
		onfolderrename?: (folder: WorkflowFolder, name: string) => void;
		/** Folder delete handler */
		onfolderdelete?: (folder: WorkflowFolder) => void;
		/** Pin toggle handler */
		ontogglepin?: (workflow: Workflow) => void;
		/** Move to folder handler */
		onmoveto?: (workflow: Workflow, folderId: string | null) => void;
		/** Set of workflow IDs currently running in the background */
		runningWorkflowIds?: Set<string>;
		/** Set of workflow IDs that recently completed */
		recentlyCompletedIds?: Set<string>;
		/** Set of workflow IDs with a pending user question */
		questionPendingIds?: Set<string>;
		/** Handler for workflows dropped into a folder (or null for uncategorized) */
		onworkflowmove?: (workflowIds: string[], folderId: string | null) => void;
	}

	let {
		workflows,
		selectedId,
		collapsed = false,
		error = null,
		loading = false,
		selectionMode = false,
		selectedIds = new Set<string>(),
		folders = [],
		expandedFolderIds = new Set<string>(),
		onselect,
		ondelete,
		onrename,
		onretry,
		onselectiontoggle,
		onfoldertoggle,
		onfolderrename,
		onfolderdelete,
		ontogglepin,
		onmoveto,
		runningWorkflowIds = new Set<string>(),
		recentlyCompletedIds = new Set<string>(),
		questionPendingIds = new Set<string>(),
		onworkflowmove
	}: Props = $props();

	/** Pinned workflows (shown at top) */
	const pinnedWorkflows = $derived(
		workflows.filter((w) => w.pinned)
	);

	/** Running workflows (not pinned, shown after pinned) */
	const runningWorkflows = $derived(
		workflows.filter((w) => runningWorkflowIds.has(w.id) && !w.pinned)
	);

	/** Recently completed workflows (not pinned) */
	const completedWorkflows = $derived(
		workflows.filter((w) => recentlyCompletedIds.has(w.id) && !runningWorkflowIds.has(w.id) && !w.pinned)
	);

	/** Precomputed map of folder ID -> workflows in that folder (not pinned, not running, not recently completed) */
	const folderWorkflowsMap = $derived(
		folders.reduce<Record<string, Workflow[]>>((acc, folder) => {
			acc[folder.id] = workflows.filter(
				(w) =>
					w.folder_id === folder.id &&
					!w.pinned &&
					!runningWorkflowIds.has(w.id) &&
					!recentlyCompletedIds.has(w.id)
			);
			return acc;
		}, {})
	);

	/** Uncategorized workflows (no folder, not pinned, not running, not recently completed) */
	const uncategorizedWorkflows = $derived(
		workflows.filter(
			(w) =>
				!w.folder_id &&
				!w.pinned &&
				!runningWorkflowIds.has(w.id) &&
				!recentlyCompletedIds.has(w.id)
		)
	);

	/** Uncategorized workflows grouped by date */
	const dateGroups = $derived(groupByDate(uncategorizedWorkflows, 'updated_at'));

	/** Whether a drag is hovering the uncategorized drop zone */
	let uncategorizedDragOver = $state(false);

	function handleUncategorizedDragOver(event: DragEvent): void {
		if (!onworkflowmove || !event.dataTransfer || !hasWorkflowDragData(event)) return;
		event.preventDefault();
		event.dataTransfer.dropEffect = 'move';
		uncategorizedDragOver = true;
	}

	function handleUncategorizedDragEnter(event: DragEvent): void {
		if (!onworkflowmove || !hasWorkflowDragData(event)) return;
		event.preventDefault();
		uncategorizedDragOver = true;
	}

	function handleUncategorizedDragLeave(event: DragEvent): void {
		const related = event.relatedTarget as Node | null;
		const container = event.currentTarget as HTMLElement;
		if (related && container.contains(related)) return;
		uncategorizedDragOver = false;
	}

	function handleUncategorizedDrop(event: DragEvent): void {
		event.preventDefault();
		uncategorizedDragOver = false;
		const ids = getWorkflowIdsFromDrag(event);
		if (ids && ids.length > 0) {
			onworkflowmove?.(ids, null);
		}
	}

	/**
	 * Handle folder drop by delegating to onworkflowmove
	 */
	function handleFolderDrop(workflowIds: string[], folderId: string): void {
		onworkflowmove?.(workflowIds, folderId);
	}

	/** Whether there are any sections above the date groups */
	const hasSectionsAbove = $derived(
		pinnedWorkflows.length > 0 ||
		runningWorkflows.length > 0 ||
		completedWorkflows.length > 0 ||
		folders.length > 0
	);
</script>

{#snippet workflowItemSnippet(workflow: Workflow, isRunning?: boolean)}
	<WorkflowItem
		{workflow}
		active={workflow.id === selectedId}
		running={isRunning ?? false}
		hasQuestion={questionPendingIds.has(workflow.id)}
		{onselect}
		{ondelete}
		{onrename}
		{selectionMode}
		selected={selectedIds.has(workflow.id)}
		{onselectiontoggle}
		{ontogglepin}
		{onmoveto}
		{folders}
		{selectedIds}
	/>
{/snippet}

<div class="workflow-list" class:collapsed role="listbox" aria-label={$i18n('workflow_list_arialabel')}>
	{#if error && workflows.length === 0}
		<div class="workflow-list-error" role="alert">
			{#if collapsed}
				<span class="error-icon" title={$i18n('workflow_load_error_short')}>
					<AlertTriangle size={16} />
				</span>
			{:else}
				<div class="error-icon-wrapper">
					<AlertTriangle size={20} />
				</div>
				<p class="error-message">{$i18n('workflow_load_error')}</p>
				<p class="error-detail">{error}</p>
				{#if onretry}
					<button
						type="button"
						class="retry-button"
						onclick={onretry}
						disabled={loading}
						aria-busy={loading}
					>
						<RefreshCw size={14} class={loading ? 'spinning' : ''} />
						{loading ? $i18n('workflow_load_retrying') : $i18n('workflow_load_retry')}
					</button>
				{/if}
			{/if}
		</div>
	{:else if workflows.length === 0}
		<div class="workflow-list-empty">
			{#if collapsed}
				<span class="empty-icon" title={$i18n('workflow_no_workflows_short')}>-</span>
			{:else}
				<p>{$i18n('workflow_no_workflows')}</p>
				<p class="hint">{$i18n('workflow_no_workflows_hint')}</p>
			{/if}
		</div>
	{:else if collapsed}
		{#each pinnedWorkflows as workflow (workflow.id)}
			<WorkflowItemCompact
				{workflow}
				active={workflow.id === selectedId}
				{onselect}
			/>
		{/each}
		{#each runningWorkflows as workflow (workflow.id)}
			<WorkflowItemCompact
				{workflow}
				active={workflow.id === selectedId}
				running={true}
				hasQuestion={questionPendingIds.has(workflow.id)}
				{onselect}
			/>
		{/each}
		{#each completedWorkflows as workflow (workflow.id)}
			<WorkflowItemCompact
				{workflow}
				active={workflow.id === selectedId}
				hasQuestion={questionPendingIds.has(workflow.id)}
				{onselect}
			/>
		{/each}
		{#each uncategorizedWorkflows as workflow (workflow.id)}
			<WorkflowItemCompact
				{workflow}
				active={workflow.id === selectedId}
				{onselect}
			/>
		{/each}
	{:else}
		<!-- Pinned section -->
		{#if pinnedWorkflows.length > 0}
			<h3 class="section-header pinned">{$i18n('sidebar_pin_section_title')}</h3>
			{#each pinnedWorkflows as workflow (workflow.id)}
				{@render workflowItemSnippet(workflow)}
			{/each}
		{/if}

		<!-- Running section -->
		{#if runningWorkflows.length > 0}
			{#if pinnedWorkflows.length > 0}
				<div class="section-divider"></div>
			{/if}
			<h3 class="section-header running">{$i18n('workflow_section_running')}</h3>
			{#each runningWorkflows as workflow (workflow.id)}
				{@render workflowItemSnippet(workflow, true)}
			{/each}
		{/if}

		<!-- Recently completed section -->
		{#if completedWorkflows.length > 0}
			{#if pinnedWorkflows.length > 0 || runningWorkflows.length > 0}
				<div class="section-divider"></div>
			{/if}
			<h3 class="section-header completed">{$i18n('workflow_section_recently_completed')}</h3>
			{#each completedWorkflows as workflow (workflow.id)}
				{@render workflowItemSnippet(workflow)}
			{/each}
		{/if}

		<!-- Folder sections -->
		{#each folders as folder, folderIdx (folder.id)}
			{#if pinnedWorkflows.length > 0 || runningWorkflows.length > 0 || completedWorkflows.length > 0 || folderIdx > 0}
				<div class="section-divider"></div>
			{/if}
			<FolderItem
				{folder}
				expanded={expandedFolderIds.has(folder.id)}
				workflowCount={(folderWorkflowsMap[folder.id] ?? []).length}
				ontoggle={() => onfoldertoggle?.(folder.id)}
				onrename={onfolderrename}
				ondelete={onfolderdelete}
				onworkflowdrop={onworkflowmove ? handleFolderDrop : undefined}
			>
				{#if (folderWorkflowsMap[folder.id] ?? []).length === 0}
					<p class="folder-empty-hint">{$i18n('sidebar_folder_empty')}</p>
				{:else}
					{#each folderWorkflowsMap[folder.id] ?? [] as workflow (workflow.id)}
						{@render workflowItemSnippet(workflow)}
					{/each}
				{/if}
			</FolderItem>
		{/each}

		<!-- Uncategorized (by date) - acts as drop zone to remove from folder -->
		{#if dateGroups.length > 0}
			<div
				class="uncategorized-drop-zone"
				class:drag-over={uncategorizedDragOver}
				role="group"
				aria-label={$i18n('sidebar_folder_uncategorized')}
				ondragover={handleUncategorizedDragOver}
				ondragenter={handleUncategorizedDragEnter}
				ondragleave={handleUncategorizedDragLeave}
				ondrop={handleUncategorizedDrop}
			>
				{#each dateGroups as group, groupIdx (group.label)}
					{#if hasSectionsAbove || groupIdx > 0}
						<div class="section-divider"></div>
					{/if}
					<h3 class="section-header">{$i18n(DATE_GROUP_I18N[group.label] ?? group.label)}</h3>
					{#each group.items as workflow (workflow.id)}
						{@render workflowItemSnippet(workflow)}
					{/each}
				{/each}
			</div>
		{/if}
	{/if}
</div>

<style>
	.workflow-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.workflow-list.collapsed {
		gap: var(--spacing-sm);
		align-items: center;
	}

	.workflow-list-error {
		text-align: center;
		padding: var(--spacing-xl) var(--spacing-md);
		color: var(--color-text-secondary);
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.error-icon-wrapper {
		color: var(--color-warning);
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.error-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border-radius: var(--border-radius-md);
		background: var(--color-bg-tertiary);
		color: var(--color-warning);
	}

	.error-message {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		margin: 0;
	}

	.error-detail {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin: 0;
		word-break: break-word;
	}

	.retry-button {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-md);
		font-size: var(--font-size-sm);
		font-family: var(--font-family);
		color: var(--color-accent);
		background: transparent;
		border: 1px solid var(--color-accent);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		transition:
			background var(--transition-fast),
			opacity var(--transition-fast);
		margin-top: var(--spacing-xs);
	}

	.retry-button:hover:not(:disabled) {
		background: var(--color-accent-light);
	}

	.retry-button:disabled {
		opacity: 0.6;
		cursor: not-allowed;
	}

	.retry-button :global(.spinning) {
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.workflow-list-empty {
		text-align: center;
		padding: var(--spacing-xl);
		color: var(--color-text-tertiary);
	}

	.workflow-list.collapsed .workflow-list-empty {
		padding: var(--spacing-md);
	}

	.workflow-list-empty p {
		font-size: var(--font-size-sm);
		margin: 0;
	}

	.workflow-list-empty .hint {
		font-size: var(--font-size-xs);
		margin-top: var(--spacing-sm);
	}

	.empty-icon {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 32px;
		height: 32px;
		border-radius: var(--border-radius-md);
		background: var(--color-bg-tertiary);
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}

	.section-header {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
		margin: var(--spacing-xs) 0 0 0;
		padding: 0 var(--spacing-md);
		font-weight: var(--font-weight-medium);
	}

	.section-header.pinned {
		color: var(--color-accent);
	}

	.section-header.running {
		color: var(--color-success);
	}

	.section-header.completed {
		color: var(--color-text-secondary);
	}

	.section-divider {
		height: 1px;
		background: var(--color-border);
		margin: var(--spacing-sm) var(--spacing-md);
	}

	.uncategorized-drop-zone {
		border-radius: var(--border-radius-md);
		transition: outline var(--transition-fast), background var(--transition-fast);
	}

	.uncategorized-drop-zone.drag-over {
		outline: 2px dashed var(--color-accent);
		outline-offset: -2px;
		background: var(--color-accent-light);
	}

	.folder-empty-hint {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		padding: var(--spacing-sm) var(--spacing-md);
		margin: 0;
		font-style: italic;
	}
</style>
