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
  A list of workflow items with selection support.
  Supports collapsed mode for compact sidebar display.

  @example
  <WorkflowList workflows={workflows} selectedId={currentWorkflowId} onselect={handleSelect} ondelete={handleDelete} />
  <WorkflowList workflows={workflows} collapsed={true} />
-->
<script lang="ts">
	import type { Workflow } from '$types/workflow';
	import WorkflowItem from './WorkflowItem.svelte';
	import WorkflowItemCompact from './WorkflowItemCompact.svelte';
	import { AlertTriangle, RefreshCw } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { groupByDate } from '$lib/utils/dateGrouping';

	const DATE_GROUP_I18N: Record<string, string> = {
		today: 'workflow_group_today',
		yesterday: 'workflow_group_yesterday',
		last_7_days: 'workflow_group_last_7_days',
		older: 'workflow_group_older'
	};

	/**
	 * WorkflowList props
	 */
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
		/** Selection handler */
		onselect?: (workflow: Workflow) => void;
		/** Delete handler */
		ondelete?: (workflow: Workflow) => void;
		/** Rename handler */
		onrename?: (workflow: Workflow, newName: string) => void;
		/** Retry handler for failed loads */
		onretry?: () => void;
		/** Set of workflow IDs currently running in the background */
		runningWorkflowIds?: Set<string>;
		/** Set of workflow IDs that recently completed */
		recentlyCompletedIds?: Set<string>;
		/** Set of workflow IDs with a pending user question */
		questionPendingIds?: Set<string>;
	}

	let {
		workflows,
		selectedId,
		collapsed = false,
		error = null,
		loading = false,
		onselect,
		ondelete,
		onrename,
		onretry,
		runningWorkflowIds = new Set<string>(),
		recentlyCompletedIds = new Set<string>(),
		questionPendingIds = new Set<string>()
	}: Props = $props();

	/** Workflows that are currently running in the background */
	const runningWorkflows = $derived(
		workflows.filter((w) => runningWorkflowIds.has(w.id))
	);

	/** Workflows that recently completed in the background */
	const completedWorkflows = $derived(
		workflows.filter((w) => recentlyCompletedIds.has(w.id) && !runningWorkflowIds.has(w.id))
	);

	/** Remaining workflows (not running, not recently completed) */
	const remainingWorkflows = $derived(
		workflows.filter((w) => !runningWorkflowIds.has(w.id) && !recentlyCompletedIds.has(w.id))
	);

	/** Remaining workflows grouped by date (for expanded mode) */
	const dateGroups = $derived(groupByDate(remainingWorkflows, 'updated_at'));
</script>

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
		{#each remainingWorkflows as workflow (workflow.id)}
			<WorkflowItemCompact
				{workflow}
				active={workflow.id === selectedId}
				{onselect}
			/>
		{/each}
	{:else}
		{#if runningWorkflows.length > 0}
			<h3 class="section-header running">{$i18n('workflow_section_running')}</h3>
			{#each runningWorkflows as workflow (workflow.id)}
				<WorkflowItem
					{workflow}
					active={workflow.id === selectedId}
					running={true}
					hasQuestion={questionPendingIds.has(workflow.id)}
					{onselect}
					{ondelete}
					{onrename}
				/>
			{/each}
		{/if}
		{#if completedWorkflows.length > 0}
			{#if runningWorkflows.length > 0}
				<div class="section-divider"></div>
			{/if}
			<h3 class="section-header completed">{$i18n('workflow_section_recently_completed')}</h3>
			{#each completedWorkflows as workflow (workflow.id)}
				<WorkflowItem
					{workflow}
					active={workflow.id === selectedId}
					hasQuestion={questionPendingIds.has(workflow.id)}
					{onselect}
					{ondelete}
					{onrename}
				/>
			{/each}
		{/if}
		{#each dateGroups as group (group.label)}
			{#if runningWorkflows.length > 0 || completedWorkflows.length > 0 || dateGroups.indexOf(group) > 0}
				<div class="section-divider"></div>
			{/if}
			<h3 class="section-header">{$i18n(DATE_GROUP_I18N[group.label])}</h3>
			{#each group.items as workflow (workflow.id)}
				<WorkflowItem
					{workflow}
					active={workflow.id === selectedId}
					{onselect}
					{ondelete}
					{onrename}
				/>
			{/each}
		{/each}
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
</style>
